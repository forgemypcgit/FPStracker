//! RAM detection module
//!
//! Detects RAM information using:
//! - Cross-platform: sysinfo for usable RAM
//! - Linux: /proc/meminfo, sysfs, dmidecode
//! - Windows: WMI, registry

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "linux")]
type DmidecodeInfo = (Option<u64>, Option<String>, Option<u32>, Option<String>);

/// RAM module information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamInfo {
    /// Total installed RAM in MB (if detectable)
    pub installed_mb: Option<u64>,
    /// Total usable RAM in MB
    pub usable_mb: u64,
    /// RAM speed in MHz (if detectable)
    pub speed_mhz: Option<u64>,
    /// RAM type (DDR4, DDR5, etc.)
    pub ram_type: Option<String>,
    /// Number of RAM sticks
    pub stick_count: Option<u32>,
    /// RAM manufacturer/model (if detectable)
    pub model: Option<String>,
}

impl RamInfo {
    /// Detect RAM information (platform-specific)
    pub fn detect(usable_mb: u64) -> Result<Self> {
        let mut info = RamInfo {
            installed_mb: None,
            usable_mb,
            speed_mhz: None,
            ram_type: None,
            stick_count: None,
            model: None,
        };

        #[cfg(target_os = "linux")]
        {
            // Get usable RAM from /proc/meminfo (always available, no root needed)
            if let Some(mem_total) = Self::get_meminfo_total() {
                info.usable_mb = mem_total;
            }

            // Try to get installed RAM from various sources (no root required)
            info.installed_mb = Self::get_installed_ram_linux()
                .or_else(|| Some(Self::estimate_installed_from_usable(info.usable_mb)));

            // Try to get detailed info from dmidecode (requires root, but we try anyway)
            if let Ok((speed, ram_type, sticks, model)) = Self::get_dmidecode_info() {
                info.speed_mhz = speed;
                info.ram_type = ram_type;
                info.stick_count = sticks;
                info.model = model;
            }

            // Fallback: try to detect RAM type and speed from other sources
            if info.ram_type.is_none() {
                info.ram_type = Self::detect_ram_type_from_sys();
            }

            if info.speed_mhz.is_none() {
                info.speed_mhz = Self::get_speed_from_sys();
            }

            // Try to count sticks from sysfs
            if info.stick_count.is_none() {
                info.stick_count = Self::count_sticks_from_sys();
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Get installed RAM from WMI
            if let Some((installed, sticks)) = Self::get_windows_ram_info() {
                info.installed_mb = Some(installed);
                info.stick_count = sticks;
            } else {
                info.installed_mb = Some(Self::estimate_installed_from_usable(info.usable_mb));
            }

            // Get RAM speed and type from WMI
            if let Ok((speed, ram_type, model)) = Self::get_windows_ram_details() {
                info.speed_mhz = speed;
                info.ram_type = ram_type;
                info.model = model;
            }
        }

        Ok(info)
    }

    /// Get total memory from /proc/meminfo (Linux only)
    #[cfg(target_os = "linux")]
    fn get_meminfo_total() -> Option<u64> {
        let content = fs::read_to_string("/proc/meminfo").ok()?;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                // Format: "MemTotal:       16384000 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return Some(kb / 1024); // Convert KB to MB
                    }
                }
            }
        }

        None
    }

    /// Try to detect installed RAM from various sources on Linux
    #[cfg(target_os = "linux")]
    fn get_installed_ram_linux() -> Option<u64> {
        // Try to parse from dmesg logs
        if let Some(mem) = Self::get_installed_from_dmesg() {
            return Some(mem);
        }

        // Try /proc/kmsg
        if let Some(mem) = Self::get_installed_from_kmsg() {
            return Some(mem);
        }

        None
    }

    /// Estimate installed RAM from usable RAM
    /// Common RAM sizes: 8, 16, 32, 64 GB
    pub fn estimate_installed_from_usable(usable_mb: u64) -> u64 {
        let usable_gb = usable_mb as f64 / 1024.0;

        // Common RAM sizes in GB
        let common_sizes = [4, 8, 16, 32, 64, 128];

        for &size in &common_sizes {
            let size_mb = size * 1024;
            // If usable is within 1GB of a common size, assume that's the installed size
            if usable_gb >= (size as f64 - 1.0) && usable_gb <= size as f64 {
                return size_mb as u64;
            }
        }

        // If no match, round up to nearest GB
        (usable_gb.ceil() as u64) * 1024
    }

    /// Parse installed RAM from dmesg output (Linux only)
    #[cfg(target_os = "linux")]
    fn get_installed_from_dmesg() -> Option<u64> {
        // Try to read from dmesg command
        if let Ok(output) = Command::new("dmesg").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Look for patterns like:
            // "Memory: 16384000K/16777216K available"
            // "RAM: 16384 MB"
            for line in stdout.lines() {
                if line.contains("Memory:") && line.contains("available") {
                    // Extract total memory (after the /)
                    if let Some(idx) = line.find('/') {
                        let after_slash = &line[idx + 1..];
                        if let Some(end_idx) = after_slash
                            .find(|c: char| !c.is_ascii_digit() && c != 'K' && c != 'M' && c != 'G')
                        {
                            let mem_str = &after_slash[..end_idx];
                            if let Some(stripped) = mem_str.strip_suffix('K') {
                                if let Ok(kb) = stripped.parse::<u64>() {
                                    return Some(kb / 1024);
                                }
                            } else if let Some(stripped) = mem_str.strip_suffix('M') {
                                if let Ok(mb) = stripped.parse::<u64>() {
                                    return Some(mb);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Try to read from /var/log/dmesg if available
        if let Ok(content) = fs::read_to_string("/var/log/dmesg") {
            for line in content.lines() {
                if line.contains("Memory:") && line.contains("available") {
                    if let Some(idx) = line.find('/') {
                        let after_slash = &line[idx + 1..];
                        if let Some(end_idx) = after_slash
                            .find(|c: char| !c.is_ascii_digit() && c != 'K' && c != 'M' && c != 'G')
                        {
                            let mem_str = &after_slash[..end_idx];
                            if let Some(stripped) = mem_str.strip_suffix('K') {
                                if let Ok(kb) = stripped.parse::<u64>() {
                                    return Some(kb / 1024);
                                }
                            } else if let Some(stripped) = mem_str.strip_suffix('M') {
                                if let Ok(mb) = stripped.parse::<u64>() {
                                    return Some(mb);
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Try to get installed RAM from /dev/kmsg or /proc/kmsg (Linux only)
    #[cfg(target_os = "linux")]
    fn get_installed_from_kmsg() -> Option<u64> {
        // Try /proc/kmsg (might need root, but we try anyway)
        if let Ok(content) = fs::read_to_string("/proc/kmsg") {
            let lines: Vec<&str> = content.lines().take(100).collect();
            for line in lines {
                if line.contains("Memory:") {
                    // Parse similar to dmesg
                    if let Some(idx) = line.find("Memory:") {
                        let mem_part = &line[idx + 7..];
                        if let Some(slash_idx) = mem_part.find('/') {
                            let after_slash = &mem_part[slash_idx + 1..];
                            let digits: String = after_slash
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            if let Ok(kb) = digits.parse::<u64>() {
                                return Some(kb / 1024);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Detect RAM type from sysfs (DDR4, DDR5, etc.) - Linux only
    #[cfg(target_os = "linux")]
    fn detect_ram_type_from_sys() -> Option<String> {
        // Try to read from EDAC (Error Detection and Correction) if available
        let edac_paths = [
            "/sys/devices/system/edac/mc/mc0/",
            "/sys/devices/system/edac/mc/mc1/",
        ];

        for path in &edac_paths {
            let mc_path = Path::new(path);
            if mc_path.exists() {
                // Check for any files that might indicate memory type
                if let Ok(entries) = fs::read_dir(mc_path) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.contains("ddr4") || name.contains("DDR4") {
                            return Some("DDR4".to_string());
                        } else if name.contains("ddr5") || name.contains("DDR5") {
                            return Some("DDR5".to_string());
                        } else if name.contains("ddr3") || name.contains("DDR3") {
                            return Some("DDR3".to_string());
                        }
                    }
                }
            }
        }

        None
    }

    /// Get RAM speed from various sys sources - Linux only
    #[cfg(target_os = "linux")]
    fn get_speed_from_sys() -> Option<u64> {
        // Try memory controller clock speed
        let paths = [
            "/sys/devices/system/edac/mc/mc0/clock_speed",
            "/sys/devices/system/edac/mc/mc1/clock_speed",
            "/sys/class/dmi/id/dimm_freq",
        ];

        for path in &paths {
            if Path::new(path).exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(freq) = content.trim().parse::<u64>() {
                        return Some(freq);
                    }
                }
            }
        }

        None
    }

    /// Count RAM sticks from sysfs - Linux only
    #[cfg(target_os = "linux")]
    fn count_sticks_from_sys() -> Option<u32> {
        // Try to count from memory controller info if available
        let mut count = 0;

        // Check if we can read EDAC (Error Detection and Correction) info
        // This requires kernel support and may not be available
        let edac_path = Path::new("/sys/devices/system/edac/mc/");
        if edac_path.exists() {
            if let Ok(entries) = fs::read_dir(edac_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("mc") && name.len() > 2 {
                        // Count csrow entries (chip select rows = memory slots)
                        let mc_path = entry.path();
                        if let Ok(csrows) = fs::read_dir(&mc_path) {
                            let csrow_count = csrows
                                .filter(|e| {
                                    e.as_ref()
                                        .map(|e| {
                                            e.file_name().to_string_lossy().starts_with("csrow")
                                        })
                                        .unwrap_or(false)
                                })
                                .count();
                            if csrow_count > 0 {
                                count += csrow_count as u32;
                            }
                        }
                    }
                }
            }
        }

        // Only return if we got a reasonable number (EDAC gives slots, not blocks)
        if count > 0 && count < 16 {
            Some(count)
        } else {
            None
        }
    }

    /// Get RAM info from dmidecode (Linux only, requires root)
    #[cfg(target_os = "linux")]
    fn get_dmidecode_info() -> Result<DmidecodeInfo> {
        let output = Command::new("dmidecode")
            .args(["-t", "17"]) // Type 17 = Memory Device
            .output()?;

        if !output.status.success() {
            anyhow::bail!("dmidecode failed or not available");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut speed = None;
        let mut ram_type = None;
        let mut sticks = 0u32;
        let mut model = None;

        for line in stdout.lines() {
            if line.contains("Speed:") && !line.contains("Unknown") {
                if let Some(s) = line.split(':').nth(1) {
                    if let Ok(speed_val) = s.trim().replace(" MHz", "").parse::<u64>() {
                        if speed_val > 0 {
                            speed = Some(speed_val);
                        }
                    }
                }
            }
            if line.contains("Type:") && !line.contains("Unknown") {
                if let Some(t) = line.split(':').nth(1) {
                    let type_str = t.trim();
                    if type_str.starts_with("DDR") {
                        ram_type = Some(type_str.to_string());
                    }
                }
            }
            if line.contains("Part Number:") && !line.contains("Unknown") {
                if let Some(p) = line.split(':').nth(1) {
                    let part = p.trim().to_string();
                    if !part.is_empty() && part != "Not Specified" {
                        model = Some(part);
                    }
                }
            }
            if line.contains("Size:") && !line.contains("No Module Installed") {
                sticks += 1;
            }
        }

        if sticks == 0 {
            sticks = 1; // Assume at least 1 stick if we got any info
        }

        Ok((speed, ram_type, Some(sticks), model))
    }

    /// Get Windows RAM info using WMI
    #[cfg(target_os = "windows")]
    fn get_windows_ram_info() -> Option<(u64, Option<u32>)> {
        // Get total physical memory
        let output = Command::new("wmic")
            .args([
                "ComputerSystem",
                "get",
                "TotalPhysicalMemory",
                "/format:csv",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total_mb = 0u64;

        // Parse CSV output (skip header)
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                // TotalPhysicalMemory is in bytes
                if let Ok(bytes) = parts[1].trim().parse::<u64>() {
                    total_mb = bytes / 1024 / 1024;
                    break;
                }
            }
        }

        // Get number of memory devices
        let output = Command::new("wmic")
            .args(["MemoryChip", "get", "Capacity", "/format:csv"])
            .output()
            .ok()?;

        let mut stick_count = 0u32;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Count non-empty lines (skip header)
            stick_count = stdout
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .count() as u32;
        }

        if total_mb > 0 {
            Some((
                total_mb,
                if stick_count > 0 {
                    Some(stick_count)
                } else {
                    None
                },
            ))
        } else {
            None
        }
    }

    /// Get Windows RAM details (speed, type, model) using WMI
    #[cfg(target_os = "windows")]
    fn get_windows_ram_details() -> Result<(Option<u64>, Option<String>, Option<String>)> {
        let output = Command::new("wmic")
            .args([
                "MemoryChip",
                "get",
                "Speed,SMBIOSMemoryType,PartNumber",
                "/format:csv",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("wmic failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut speed = None;
        let mut ram_type = None;
        let mut model = None;

        // Parse CSV output (skip header)
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 4 {
                // Speed is in MHz
                if speed.is_none() {
                    if let Ok(s) = parts[1].trim().parse::<u64>() {
                        if s > 0 {
                            speed = Some(s);
                        }
                    }
                }

                // SMBIOSMemoryType: 24=DDR3, 26=DDR4, 34=DDR5
                if ram_type.is_none() {
                    if let Ok(mem_type) = parts[2].trim().parse::<u32>() {
                        ram_type = match mem_type {
                            24 => Some("DDR3".to_string()),
                            26 => Some("DDR4".to_string()),
                            34 => Some("DDR5".to_string()),
                            _ => None,
                        };
                    }
                }

                // Part number (model)
                if model.is_none() {
                    let part = parts[3].trim();
                    if !part.is_empty() && part != "PartNumber" {
                        model = Some(part.to_string());
                    }
                }

                // Break if we have all info
                if speed.is_some() && ram_type.is_some() && model.is_some() {
                    break;
                }
            }
        }

        Ok((speed, ram_type, model))
    }

    /// Format RAM info for display
    #[allow(dead_code)]
    pub fn format(&self) -> String {
        let usable_gb = self.usable_mb as f64 / 1024.0;

        let mut parts = Vec::new();

        // Show both installed and usable when we have both
        if let Some(installed_mb) = self.installed_mb {
            let installed_gb = installed_mb as f64 / 1024.0;
            if (installed_gb - usable_gb).abs() > 0.5 {
                // Significant difference, show both
                parts.push(format!(
                    "{:.0} GB installed ({:.1} GB usable)",
                    installed_gb, usable_gb
                ));
            } else {
                // Similar, just show one
                parts.push(format!("{:.0} GB", installed_gb));
            }
        } else {
            // Only have usable
            parts.push(format!("{:.1} GB", usable_gb));
        }

        if let Some(ref ram_type) = self.ram_type {
            parts.push(ram_type.clone());
        }

        if let Some(speed) = self.speed_mhz {
            parts.push(format!("{} MHz", speed));
        }

        if let Some(sticks) = self.stick_count {
            parts.push(format!("{} stick(s)", sticks));
        }

        if let Some(ref model) = self.model {
            parts.push(model.clone());
        }

        parts.join(" | ")
    }
}
