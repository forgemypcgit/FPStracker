//! CPU detection module
//!
//! Detects CPU information using:
//! - Cross-platform: sysinfo crate
//! - Linux: /proc/cpuinfo, cpufreq sysfs
//! - Windows: WMI, registry

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use sysinfo::System;

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU name (e.g., "AMD Ryzen 7 5800X")
    pub name: String,
    /// Number of physical cores
    pub cores: usize,
    /// Number of logical threads
    pub threads: usize,
    /// Base frequency in MHz (if available)
    pub frequency_mhz: Option<u64>,
    /// CPU vendor
    pub vendor: String,
    /// CPU architecture
    pub architecture: Option<String>,
    /// Max turbo/boost frequency in MHz (if available)
    pub max_frequency_mhz: Option<u64>,
}

impl CpuInfo {
    /// Detect CPU information (platform-specific)
    pub fn detect() -> Result<Self> {
        let mut sys = System::new();
        sys.refresh_cpu_all();

        let cpus = sys.cpus();
        if cpus.is_empty() {
            anyhow::bail!("No CPU detected");
        }

        let first_cpu = &cpus[0];
        let name = first_cpu.brand().to_string();
        let vendor = first_cpu.vendor_id().to_string();
        let frequency_mhz = Some(first_cpu.frequency());

        // Count physical cores
        let threads = cpus.len();
        let cores = sys.physical_core_count().unwrap_or(threads / 2);

        // Get platform-specific additional info
        #[cfg(target_os = "linux")]
        let (architecture, max_frequency_mhz) = Self::get_linux_cpu_info();

        #[cfg(target_os = "windows")]
        let (architecture, max_frequency_mhz) = Self::get_windows_cpu_info();

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let (architecture, max_frequency_mhz) = (None, None);

        Ok(CpuInfo {
            name,
            cores,
            threads,
            frequency_mhz,
            vendor,
            architecture,
            max_frequency_mhz,
        })
    }

    /// Get additional CPU info from Linux /proc/cpuinfo
    #[cfg(target_os = "linux")]
    fn get_linux_cpu_info() -> (Option<String>, Option<u64>) {
        let mut architecture = None;
        let mut max_frequency = None;

        if Path::new("/proc/cpuinfo").exists() {
            if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
                for line in content.lines() {
                    if line.starts_with("model name") && architecture.is_none() {
                        // Extract architecture hints from model name
                        if line.contains("x86-64") || line.contains("Intel") || line.contains("AMD")
                        {
                            architecture = Some("x86_64".to_string());
                        }
                    }
                }
            }
        }

        // Try to get max frequency from cpufreq
        if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq").exists() {
            if let Ok(freq) =
                fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq")
            {
                if let Ok(freq_khz) = freq.trim().parse::<u64>() {
                    max_frequency = Some(freq_khz / 1000); // Convert kHz to MHz
                }
            }
        }

        (architecture, max_frequency)
    }

    /// Get additional CPU info from Windows WMI
    #[cfg(target_os = "windows")]
    fn get_windows_cpu_info() -> (Option<String>, Option<u64>) {
        let mut architecture = Some("x86_64".to_string()); // Most Windows PCs are x86_64
        let mut max_frequency = None;

        // Use wmic to get CPU info
        if let Ok(output) = Command::new("wmic")
            .args(["cpu", "get", "MaxClockSpeed,Architecture", "/format:csv"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);

                // Parse CSV output (skip header)
                for line in stdout.lines().skip(1) {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 3 {
                        // MaxClockSpeed is in MHz
                        if let Ok(freq) = parts[1].trim().parse::<u64>() {
                            max_frequency = Some(freq);
                        }

                        // Architecture codes: 0=x86, 1=MIPS, 2=Alpha, 3=PowerPC,
                        // 5=ARM, 6=ia64, 9=x64
                        if let Ok(arch_code) = parts[2].trim().parse::<u32>() {
                            architecture = match arch_code {
                                0 => Some("x86".to_string()),
                                9 => Some("x86_64".to_string()),
                                5 => Some("ARM".to_string()),
                                6 => Some("IA64".to_string()),
                                12 => Some("ARM64".to_string()),
                                _ => Some("x86_64".to_string()),
                            };
                        }
                        break;
                    }
                }
            }
        }

        // Alternative: Try to read from registry
        if max_frequency.is_none() {
            if let Ok(output) = Command::new("reg")
                .args([
                    "query",
                    "HKEY_LOCAL_MACHINE\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
                    "/v",
                    "~MHz",
                ])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Parse: "    ~MHz    REG_DWORD    0x1e61"
                    for line in stdout.lines() {
                        if line.contains("~MHz") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if let Some(hex_str) = parts.last() {
                                if let Ok(freq) =
                                    u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
                                {
                                    max_frequency = Some(freq);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        (architecture, max_frequency)
    }
}
