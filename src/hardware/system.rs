//! System information aggregator

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sysinfo::System;

use super::cpu::CpuInfo;
use super::gpu::GpuInfo;
use super::ram::RamInfo;

/// Complete system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// GPU info
    pub gpu: GpuInfo,
    /// CPU info
    pub cpu: CpuInfo,
    /// RAM info
    pub ram: RamInfo,
    /// Operating system
    pub os: String,
    /// OS version
    pub os_version: Option<String>,
}

impl SystemInfo {
    /// Detect all system information
    pub fn detect() -> Result<Self> {
        let gpu = GpuInfo::detect()?;
        let cpu = CpuInfo::detect()?;

        let mut sys = System::new();
        sys.refresh_memory();

        let ram_mb = sys.total_memory() / 1024 / 1024; // bytes to MB
        let ram = RamInfo::detect(ram_mb)?;

        let os = System::name().unwrap_or_else(|| "Unknown".to_string());
        let os_version = System::os_version();

        Ok(SystemInfo {
            gpu,
            cpu,
            ram,
            os,
            os_version,
        })
    }

    /// Display system info as formatted string
    pub fn display(&self) -> String {
        const WIDTH: usize = 62;
        let mut output = String::new();

        output.push_str(&format!("╔{}╗\n", "═".repeat(WIDTH)));
        output.push_str(&format!("║{:^WIDTH$}║\n", "SYSTEM INFORMATION"));
        output.push_str(&format!("╠{}╣\n", "═".repeat(WIDTH)));

        // Helper to format lines with proper padding
        let format_line = |label: &str, content: &str| -> String {
            let content_width = WIDTH.saturating_sub(2); // Space for "║" and "║"
            let label_len = label.len();
            if label_len < content_width {
                format!(
                    "║ {}{:<content_width$}║\n",
                    label,
                    content,
                    content_width = content_width - label_len
                )
            } else {
                format!(
                    "║ {}{}║\n",
                    label,
                    &content[..content_width.saturating_sub(label_len)]
                )
            }
        };

        // GPU Section
        output.push_str(&format_line("GPU: ", &self.gpu.name));
        if let Some(vram) = self.gpu.vram_mb {
            output.push_str(&format_line("      ", &format!("VRAM: {} MB", vram)));
        }
        if let Some(clock) = self.gpu.gpu_clock_mhz {
            output.push_str(&format_line("      ", &format!("GPU Clock: {} MHz", clock)));
        }
        if let Some(mem_clock) = self.gpu.memory_clock_mhz {
            output.push_str(&format_line(
                "      ",
                &format!("Memory Clock: {} MHz", mem_clock),
            ));
        }
        if let Some(temp) = self.gpu.temperature_c {
            output.push_str(&format_line("      ", &format!("Temperature: {}°C", temp)));
        }
        if let Some(util) = self.gpu.utilization_percent {
            output.push_str(&format_line("      ", &format!("Utilization: {}%", util)));
        }
        if let Some(ref driver) = self.gpu.driver_version {
            output.push_str(&format_line("      ", &format!("Driver: {}", driver)));
        }

        output.push_str(&format!("╠{}╣\n", "═".repeat(WIDTH)));

        // CPU Section
        output.push_str(&format_line("CPU: ", &self.cpu.name));
        output.push_str(&format_line(
            "      ",
            &format!("{} cores / {} threads", self.cpu.cores, self.cpu.threads),
        ));
        if let Some(freq) = self.cpu.frequency_mhz {
            output.push_str(&format_line("      ", &format!("Base Clock: {} MHz", freq)));
        }
        if let Some(max_freq) = self.cpu.max_frequency_mhz {
            output.push_str(&format_line(
                "      ",
                &format!("Max Clock: {} MHz", max_freq),
            ));
        }
        if let Some(ref arch) = self.cpu.architecture {
            output.push_str(&format_line("      ", &format!("Architecture: {}", arch)));
        }

        output.push_str(&format!("╠{}╣\n", "═".repeat(WIDTH)));

        // RAM Section
        let ram_gb = self.ram.usable_mb as f64 / 1024.0;
        let mut ram_info = if let Some(installed_mb) = self.ram.installed_mb {
            let installed_gb = installed_mb as f64 / 1024.0;
            let diff_gb = installed_gb - ram_gb;
            // Show both if there's a meaningful difference (usually 0.2-1GB reserved for system)
            // The reserved memory is used by kernel, BIOS, and integrated graphics
            if diff_gb >= 0.2 {
                // Show if 200MB+ is reserved
                format!("{:.0} GB installed ({:.1} GB usable)", installed_gb, ram_gb)
            } else {
                format!("{:.0} GB", installed_gb)
            }
        } else {
            format!("{:.1} GB usable", ram_gb)
        };
        if let Some(ref ram_type) = self.ram.ram_type {
            ram_info.push_str(&format!(" | {}", ram_type));
        }
        if let Some(speed) = self.ram.speed_mhz {
            ram_info.push_str(&format!(" | {} MHz", speed));
        }
        if let Some(sticks) = self.ram.stick_count {
            ram_info.push_str(&format!(" | {} stick(s)", sticks));
        }
        output.push_str(&format_line("RAM: ", &ram_info));
        if let Some(ref model) = self.ram.model {
            output.push_str(&format_line("      ", &format!("Model: {}", model)));
        }

        output.push_str(&format!("╠{}╣\n", "═".repeat(WIDTH)));

        // OS Section
        let os_str = match &self.os_version {
            Some(ver) => format!("{} {}", self.os, ver),
            None => self.os.clone(),
        };
        output.push_str(&format_line("OS:  ", &os_str));

        output.push_str(&format!("╚{}╝", "═".repeat(WIDTH)));

        output
    }
}
