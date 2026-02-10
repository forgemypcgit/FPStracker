//! GPU detection module
//!
//! Detects GPU information using:
//! - Linux: Parse /sys/class/drm and lspci output
//! - Windows: WMI, DirectX, or dxdiag
//! - NVIDIA: Can use nvidia-smi if available (cross-platform)
//! - AMD: Parse amdgpu sysfs (Linux) or WMI (Windows)
//! - Intel: Parse i915 sysfs (Linux) or WMI (Windows)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

/// GPU vendor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

impl std::fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuVendor::Nvidia => write!(f, "NVIDIA"),
            GpuVendor::Amd => write!(f, "AMD"),
            GpuVendor::Intel => write!(f, "Intel"),
            GpuVendor::Unknown => write!(f, "Unknown"),
        }
    }
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name (e.g., "NVIDIA GeForce RTX 4070 Super")
    pub name: String,
    /// Vendor
    pub vendor: GpuVendor,
    /// VRAM in MB (if detectable)
    pub vram_mb: Option<u64>,
    /// Driver version (if detectable)
    pub driver_version: Option<String>,
    /// PCI device ID
    pub pci_id: Option<String>,
    /// GPU clock speed in MHz (if detectable)
    pub gpu_clock_mhz: Option<u64>,
    /// Memory clock speed in MHz (if detectable)
    pub memory_clock_mhz: Option<u64>,
    /// GPU temperature in Celsius (if detectable)
    pub temperature_c: Option<u64>,
    /// GPU utilization percentage (if detectable)
    pub utilization_percent: Option<u64>,
}

impl GpuInfo {
    /// Detect primary GPU (platform-specific)
    pub fn detect() -> Result<Self> {
        // Try multiple detection methods in order of reliability

        // Method 1: Try nvidia-smi for NVIDIA GPUs (cross-platform)
        if let Ok(gpu) = Self::detect_nvidia_smi() {
            return Ok(gpu);
        }

        #[cfg(target_os = "linux")]
        {
            // Method 2: Parse lspci output (Linux)
            if let Ok(gpu) = Self::detect_lspci() {
                return Ok(gpu);
            }

            // Method 3: Parse /sys/class/drm (Linux)
            if let Ok(gpu) = Self::detect_sysfs() {
                return Ok(gpu);
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Method 2: Use dxdiag on Windows
            if let Ok(gpu) = Self::detect_dxdiag() {
                return Ok(gpu);
            }

            // Method 3: Use WMI on Windows
            if let Ok(gpu) = Self::detect_wmi() {
                return Ok(gpu);
            }
        }

        // Fallback: Unknown GPU
        Ok(GpuInfo {
            name: "Unknown GPU".to_string(),
            vendor: GpuVendor::Unknown,
            vram_mb: None,
            driver_version: None,
            pci_id: None,
            gpu_clock_mhz: None,
            memory_clock_mhz: None,
            temperature_c: None,
            utilization_percent: None,
        })
    }

    /// Detect NVIDIA GPU using nvidia-smi (cross-platform)
    fn detect_nvidia_smi() -> Result<Self> {
        // Query basic info
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .context("nvidia-smi not found")?;

        if !output.status.success() {
            anyhow::bail!("nvidia-smi failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next().context("No GPU found")?;
        let parts: Vec<&str> = line.split(", ").collect();

        if parts.len() < 3 {
            anyhow::bail!("Invalid nvidia-smi output");
        }

        let raw_name = parts[0].trim();
        let name = if raw_name.starts_with("NVIDIA") {
            raw_name.to_string()
        } else {
            format!("NVIDIA {}", raw_name)
        };
        let vram_mb: u64 = parts[1].trim().parse().unwrap_or(0);
        let driver_version = parts[2].trim().to_string();

        // Try to get additional info (clocks, temp, utilization)
        let (gpu_clock, memory_clock, temp, utilization) = Self::get_nvidia_smi_extra();

        Ok(GpuInfo {
            name,
            vendor: GpuVendor::Nvidia,
            vram_mb: Some(vram_mb),
            driver_version: Some(driver_version),
            pci_id: None,
            gpu_clock_mhz: gpu_clock,
            memory_clock_mhz: memory_clock,
            temperature_c: temp,
            utilization_percent: utilization,
        })
    }

    /// Get additional NVIDIA GPU info (clocks, temp, utilization)
    fn get_nvidia_smi_extra() -> (Option<u64>, Option<u64>, Option<u64>, Option<u64>) {
        let mut gpu_clock = None;
        let mut memory_clock = None;
        let mut temp = None;
        let mut utilization = None;

        // Query clocks and temperature
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=clocks.gr,clocks.mem,temperature.gpu,utilization.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    let parts: Vec<&str> = line.split(", ").collect();
                    if parts.len() >= 4 {
                        // Parse clock speeds (remove " MHz" suffix if present)
                        gpu_clock = parts[0].trim().replace(" MHz", "").parse().ok();
                        memory_clock = parts[1].trim().replace(" MHz", "").parse().ok();
                        temp = parts[2].trim().replace(" C", "").parse().ok();
                        utilization = parts[3].trim().replace(" %", "").parse().ok();
                    }
                }
            }
        }

        (gpu_clock, memory_clock, temp, utilization)
    }

    /// Detect GPU using lspci (Linux only)
    #[cfg(target_os = "linux")]
    fn detect_lspci() -> Result<Self> {
        let _output = Command::new("lspci")
            .args(["-v", "-s", "0000:00:02.0"]) // Try integrated first
            .output();

        // Try to find discrete GPU
        let output = Command::new("lspci").output().context("lspci not found")?;

        if !output.status.success() {
            anyhow::bail!("lspci failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Look for VGA or 3D controller
        for line in stdout.lines() {
            if line.contains("VGA") || line.contains("3D controller") {
                let (vendor, name) = Self::parse_lspci_line(line);
                return Ok(GpuInfo {
                    name,
                    vendor,
                    vram_mb: None,
                    driver_version: None,
                    pci_id: Some(line.split_whitespace().next().unwrap_or("").to_string()),
                    gpu_clock_mhz: None,
                    memory_clock_mhz: None,
                    temperature_c: None,
                    utilization_percent: None,
                });
            }
        }

        anyhow::bail!("No GPU found in lspci output")
    }

    /// Parse a single lspci line
    #[cfg(target_os = "linux")]
    fn parse_lspci_line(line: &str) -> (GpuVendor, String) {
        let vendor = if line.contains("NVIDIA") {
            GpuVendor::Nvidia
        } else if line.contains("AMD") || line.contains("ATI") || line.contains("Radeon") {
            GpuVendor::Amd
        } else if line.contains("Intel") {
            GpuVendor::Intel
        } else {
            GpuVendor::Unknown
        };

        // Extract GPU name from the line
        // Format: "01:00.0 VGA compatible controller: NVIDIA Corporation GA104 [GeForce RTX 3070] (rev a1)"
        let name = if let Some(idx) = line.find(": ") {
            let after_colon = &line[idx + 2..];
            // Remove revision info
            if let Some(rev_idx) = after_colon.rfind(" (rev") {
                after_colon[..rev_idx].to_string()
            } else {
                after_colon.to_string()
            }
        } else {
            line.to_string()
        };

        (vendor, name)
    }

    /// Detect GPU using sysfs (Linux only)
    #[cfg(target_os = "linux")]
    fn detect_sysfs() -> Result<Self> {
        let drm_path = Path::new("/sys/class/drm");
        if !drm_path.exists() {
            anyhow::bail!("/sys/class/drm not found");
        }

        // Look for card0, card1, etc.
        for entry in fs::read_dir(drm_path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("card") && !name_str.contains('-') {
                let device_path = entry.path().join("device");

                // Try to read vendor
                let vendor_path = device_path.join("vendor");
                if let Ok(vendor_id) = fs::read_to_string(&vendor_path) {
                    let vendor_id = vendor_id.trim();
                    let vendor = match vendor_id {
                        "0x10de" => GpuVendor::Nvidia,
                        "0x1002" => GpuVendor::Amd,
                        "0x8086" => GpuVendor::Intel,
                        _ => GpuVendor::Unknown,
                    };

                    // Try to get device name
                    let uevent_path = device_path.join("uevent");
                    let gpu_name = if let Ok(uevent) = fs::read_to_string(&uevent_path) {
                        uevent
                            .lines()
                            .find(|l| l.starts_with("PCI_ID="))
                            .map(|l| l.replace("PCI_ID=", ""))
                            .unwrap_or_else(|| format!("{} GPU", vendor))
                    } else {
                        format!("{} GPU", vendor)
                    };

                    return Ok(GpuInfo {
                        name: gpu_name,
                        vendor,
                        vram_mb: None,
                        driver_version: None,
                        pci_id: None,
                        gpu_clock_mhz: None,
                        memory_clock_mhz: None,
                        temperature_c: None,
                        utilization_percent: None,
                    });
                }
            }
        }

        anyhow::bail!("No GPU found in sysfs")
    }

    /// Detect GPU using dxdiag on Windows
    #[cfg(target_os = "windows")]
    fn detect_dxdiag() -> Result<Self> {
        // Run dxdiag and save output to a randomized temp directory to avoid predictable filenames.
        let temp_dir = tempfile::Builder::new()
            .prefix("fps-tracker-dxdiag-")
            .tempdir()
            .context("Failed to create temp directory for dxdiag output")?;
        let temp_file = temp_dir.path().join("dxdiag_output.txt");

        let output = Command::new("dxdiag")
            .arg("/t")
            .arg(&temp_file)
            .output()
            .context("dxdiag not found")?;

        if !output.status.success() {
            anyhow::bail!("dxdiag failed");
        }

        // Read the output file
        let content = fs::read_to_string(&temp_file).context("Failed to read dxdiag output")?;

        // Parse the display device info
        let mut name = String::new();
        let mut vendor = GpuVendor::Unknown;
        let mut vram_mb = None;
        let mut driver_version = None;

        for line in content.lines() {
            let line = line.trim();

            // Look for device name
            if line.starts_with("Device:") || line.starts_with("Card name:") {
                if let Some(idx) = line.find(':') {
                    name = line[idx + 1..].trim().to_string();

                    // Detect vendor from name
                    if name.contains("NVIDIA")
                        || name.contains("GeForce")
                        || name.contains("RTX")
                        || name.contains("GTX")
                    {
                        vendor = GpuVendor::Nvidia;
                    } else if name.contains("AMD") || name.contains("Radeon") {
                        vendor = GpuVendor::Amd;
                    } else if name.contains("Intel")
                        || name.contains("Arc")
                        || name.contains("Iris")
                        || name.contains("UHD")
                    {
                        vendor = GpuVendor::Intel;
                    }
                }
            }

            // Look for display memory
            if line.contains("Display Memory") || line.contains("Dedicated Memory") {
                if let Some(idx) = line.find(':') {
                    let mem_str = line[idx + 1..].trim();
                    // Parse memory value (e.g., "8192 MB" or "8 GB")
                    let mem_mb = Self::parse_memory_string(mem_str);
                    vram_mb = Some(mem_mb);
                }
            }

            // Look for driver version
            if line.starts_with("Driver Version:") || line.starts_with("Version:") {
                if let Some(idx) = line.find(':') {
                    driver_version = Some(line[idx + 1..].trim().to_string());
                }
            }
        }

        if name.is_empty() {
            anyhow::bail!("No GPU found in dxdiag output");
        }

        Ok(GpuInfo {
            name,
            vendor,
            vram_mb,
            driver_version,
            pci_id: None,
            gpu_clock_mhz: None,
            memory_clock_mhz: None,
            temperature_c: None,
            utilization_percent: None,
        })
    }

    /// Parse memory string (e.g., "8192 MB" or "8 GB") to MB
    #[cfg(target_os = "windows")]
    fn parse_memory_string(s: &str) -> u64 {
        let s = s.to_lowercase();
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() >= 2 {
            if let Ok(value) = parts[0].parse::<f64>() {
                match parts[1] {
                    "gb" | "gib" => return (value * 1024.0) as u64,
                    "mb" | "mib" => return value as u64,
                    _ => {}
                }
            }
        }

        // Try to extract just numbers
        s.chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    }

    /// Detect GPU using WMI on Windows
    #[cfg(target_os = "windows")]
    fn detect_wmi() -> Result<Self> {
        // Use wmic to query video controller
        let output = Command::new("wmic")
            .args([
                "path",
                "win32_VideoController",
                "get",
                "Name,AdapterRAM,DriverVersion",
                "/format:csv",
            ])
            .output()
            .context("wmic not found")?;

        if !output.status.success() {
            anyhow::bail!("wmic failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse CSV output
        for line in stdout.lines().skip(1) {
            // Skip header
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 4 {
                let name = parts[1].trim().to_string();
                let ram_str = parts[2].trim();
                let driver = parts[3].trim().to_string();

                if !name.is_empty() && name != "Name" {
                    // Detect vendor
                    let vendor = if name.contains("NVIDIA") || name.contains("GeForce") {
                        GpuVendor::Nvidia
                    } else if name.contains("AMD") || name.contains("Radeon") {
                        GpuVendor::Amd
                    } else if name.contains("Intel") {
                        GpuVendor::Intel
                    } else {
                        GpuVendor::Unknown
                    };

                    // Parse VRAM (WMI returns bytes)
                    let vram_mb = ram_str.parse::<u64>().ok().map(|bytes| bytes / 1024 / 1024);

                    return Ok(GpuInfo {
                        name,
                        vendor,
                        vram_mb,
                        driver_version: if driver.is_empty() {
                            None
                        } else {
                            Some(driver)
                        },
                        pci_id: None,
                        gpu_clock_mhz: None,
                        memory_clock_mhz: None,
                        temperature_c: None,
                        utilization_percent: None,
                    });
                }
            }
        }

        anyhow::bail!("No GPU found in WMI output")
    }
}
