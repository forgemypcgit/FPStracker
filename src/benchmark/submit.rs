//! Benchmark submission
//!
//! Data structure for submitting benchmarks to the backend API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::hardware::SystemInfo;

/// A benchmark submission to the backend API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSubmission {
    /// Submission ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// System information
    pub system_info: SystemInfo,
    /// Game name
    pub game: String,
    /// Resolution (e.g., "1080p", "1440p", "4K")
    pub resolution: String,
    /// Graphics preset (e.g., "Ultra", "High", "Medium", "Low")
    pub preset: String,
    /// Average FPS
    pub avg_fps: f64,
    /// 1% low FPS (optional)
    pub fps_1_low: Option<f64>,
    /// 0.1% low FPS (optional)
    pub fps_01_low: Option<f64>,
    /// Ray tracing enabled
    pub ray_tracing: bool,
    /// Upscaling mode (e.g., "DLSS Quality", "FSR Balanced")
    pub upscaling: Option<String>,
    /// Frame generation enabled
    pub frame_gen: Option<bool>,
    /// Sample count (for automated sessions)
    pub sample_count: Option<u32>,
    /// Session duration in seconds (for automated sessions)
    pub duration_secs: Option<f64>,
    /// Benchmark tool used (e.g., "CapFrameX", "PresentMon", "MangoHud")
    ///
    /// This is used to qualify data quality downstream (manual vs captured/imported),
    /// without collecting any personal information.
    #[serde(default)]
    pub benchmark_tool: Option<String>,
    /// Capture quality score (0-100) when available (live capture preview only).
    #[serde(default)]
    pub capture_quality_score: Option<u8>,
    /// Whether capture was flagged as unstable by heuristics (live capture preview only).
    #[serde(default)]
    pub unstable_capture: Option<bool>,
    /// Capture method reported by the UI/client when available.
    #[serde(default)]
    pub capture_method: Option<String>,
    /// User acknowledgment for anti-cheat-safe capture flows.
    #[serde(default)]
    pub anti_cheat_acknowledged: Option<bool>,
    /// Extra strict anti-cheat acknowledgment for high-risk titles.
    #[serde(default)]
    pub anti_cheat_strict_acknowledged: Option<bool>,
    /// Optional synthetic CPU score recorded in this session (e.g., WinSAT/7z/sysbench).
    #[serde(default)]
    pub synthetic_cpu_score: Option<u64>,
    /// Source identifier for `synthetic_cpu_score` (e.g., winsat, 7z_mips, internal).
    #[serde(default)]
    pub synthetic_cpu_source: Option<String>,
    /// Optional synthetic GPU score recorded in this session (e.g., WinSAT/glmark2).
    #[serde(default)]
    pub synthetic_gpu_score: Option<u64>,
    /// Source identifier for `synthetic_gpu_score` (e.g., winsat, glmark2).
    #[serde(default)]
    pub synthetic_gpu_source: Option<String>,
    /// Optional synthetic RAM score recorded in this session (e.g., WinSAT memory score).
    #[serde(default)]
    pub synthetic_ram_score: Option<u64>,
    /// Source identifier for `synthetic_ram_score` (e.g., winsat, internal).
    #[serde(default)]
    pub synthetic_ram_source: Option<String>,
    /// Optional synthetic storage score recorded in this session (e.g., WinSAT disk score).
    #[serde(default)]
    pub synthetic_disk_score: Option<u64>,
    /// Source identifier for `synthetic_disk_score` (e.g., winsat, diskspd_read_mib_s, internal).
    #[serde(default)]
    pub synthetic_disk_source: Option<String>,
    /// Synthetic run profile when a baseline run was attempted ("quick", "standard", "extended").
    ///
    /// This allows backend analytics to compare submissions by benchmark precision mode
    /// without introducing any user identifiers.
    #[serde(default)]
    pub synthetic_profile: Option<String>,
    /// Synthetic suite semantics version (internal schema version for score meanings).
    ///
    /// This is set automatically when FPS Tracker runs the synthetic baseline, and helps keep
    /// analytics consistent across future scoring changes.
    #[serde(default)]
    pub synthetic_suite_version: Option<String>,
    /// Optional extended synthetic metrics for audit/debug (tool-specific raw readings).
    #[serde(default)]
    pub synthetic_extended: Option<Value>,
    /// User notes (optional)
    pub notes: Option<String>,
}

impl BenchmarkSubmission {
    /// Create a new submission from manual input
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        system_info: SystemInfo,
        game: String,
        resolution: String,
        preset: String,
        avg_fps: f64,
        fps_1_low: Option<f64>,
        ray_tracing: bool,
        upscaling: Option<String>,
    ) -> Self {
        let normalized_resolution =
            normalize_resolution(&resolution).unwrap_or_else(|| resolution.trim().to_string());

        BenchmarkSubmission {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            system_info,
            game,
            resolution: normalized_resolution,
            preset,
            avg_fps,
            fps_1_low,
            fps_01_low: None,
            ray_tracing,
            upscaling,
            frame_gen: None,
            sample_count: None,
            duration_secs: None,
            benchmark_tool: None,
            capture_quality_score: None,
            unstable_capture: None,
            capture_method: None,
            anti_cheat_acknowledged: None,
            anti_cheat_strict_acknowledged: None,
            synthetic_cpu_score: None,
            synthetic_cpu_source: None,
            synthetic_gpu_score: None,
            synthetic_gpu_source: None,
            synthetic_ram_score: None,
            synthetic_ram_source: None,
            synthetic_disk_score: None,
            synthetic_disk_source: None,
            synthetic_profile: None,
            synthetic_suite_version: None,
            synthetic_extended: None,
            notes: None,
        }
    }

    /// Create from a benchmark session
    #[allow(dead_code)]
    pub fn from_session(
        system_info: SystemInfo,
        session: &super::session::BenchmarkSession,
    ) -> Option<Self> {
        let avg_fps = session.average_fps()?;
        let normalized_resolution = normalize_resolution(&session.resolution)
            .unwrap_or_else(|| session.resolution.trim().to_string());

        Some(BenchmarkSubmission {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            system_info,
            game: session.game.clone(),
            resolution: normalized_resolution,
            preset: session.preset.clone(),
            avg_fps,
            fps_1_low: session.fps_1_low(),
            fps_01_low: session.fps_01_low(),
            ray_tracing: session.ray_tracing,
            upscaling: session.upscaling.clone(),
            frame_gen: None,
            sample_count: Some(session.fps_samples.len() as u32),
            duration_secs: Some(session.duration_secs()),
            benchmark_tool: None,
            capture_quality_score: None,
            unstable_capture: None,
            capture_method: None,
            anti_cheat_acknowledged: None,
            anti_cheat_strict_acknowledged: None,
            synthetic_cpu_score: None,
            synthetic_cpu_source: None,
            synthetic_gpu_score: None,
            synthetic_gpu_source: None,
            synthetic_ram_score: None,
            synthetic_ram_source: None,
            synthetic_disk_score: None,
            synthetic_disk_source: None,
            synthetic_profile: None,
            synthetic_suite_version: None,
            synthetic_extended: None,
            notes: None,
        })
    }

    /// Display submission details
    pub fn display(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("  Game: {}\n", self.game));
        output.push_str(&format!("  Resolution: {}\n", self.resolution));
        output.push_str(&format!("  Preset: {}\n", self.preset));
        output.push_str(&format!("  Average FPS: {:.1}\n", self.avg_fps));

        if let Some(tool) = self.benchmark_tool.as_deref() {
            output.push_str(&format!("  Capture Tool: {}\n", tool));
        }
        if let Some(score) = self.capture_quality_score {
            output.push_str(&format!("  Capture Quality: {}\n", score.min(100)));
        }
        if let Some(unstable) = self.unstable_capture {
            output.push_str(&format!(
                "  Capture Stability: {}\n",
                if unstable { "unstable" } else { "stable" }
            ));
        }

        if let Some(fps_1_low) = self.fps_1_low {
            output.push_str(&format!("  1% Low: {:.1}\n", fps_1_low));
        }
        if let Some(fps_01_low) = self.fps_01_low {
            output.push_str(&format!("  0.1% Low: {:.1}\n", fps_01_low));
        }

        if let Some(score) = self.synthetic_cpu_score {
            if let Some(src) = self
                .synthetic_cpu_source
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                output.push_str(&format!(
                    "  Synthetic CPU Score: {} ({})\n",
                    score,
                    src.trim()
                ));
            } else {
                output.push_str(&format!("  Synthetic CPU Score: {}\n", score));
            }
        }
        if let Some(score) = self.synthetic_gpu_score {
            if let Some(src) = self
                .synthetic_gpu_source
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                output.push_str(&format!(
                    "  Synthetic GPU Score: {} ({})\n",
                    score,
                    src.trim()
                ));
            } else {
                output.push_str(&format!("  Synthetic GPU Score: {}\n", score));
            }
        }
        if let Some(score) = self.synthetic_ram_score {
            if let Some(src) = self
                .synthetic_ram_source
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                output.push_str(&format!(
                    "  Synthetic RAM Score: {} ({})\n",
                    score,
                    src.trim()
                ));
            } else {
                output.push_str(&format!("  Synthetic RAM Score: {}\n", score));
            }
        }
        if let Some(score) = self.synthetic_disk_score {
            if let Some(src) = self
                .synthetic_disk_source
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                output.push_str(&format!(
                    "  Synthetic SSD Score: {} ({})\n",
                    score,
                    src.trim()
                ));
            } else {
                output.push_str(&format!("  Synthetic SSD Score: {}\n", score));
            }
        }
        if let Some(profile) = self
            .synthetic_profile
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            output.push_str(&format!("  Synthetic Profile: {}\n", profile));
        }

        if self.ray_tracing {
            output.push_str("  Ray Tracing: ON\n");
        }

        if let Some(ref upscaling) = self.upscaling {
            output.push_str(&format!("  Upscaling: {}\n", upscaling));
        }

        output
    }

    /// Validate the submission
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // FPS sanity checks
        if !self.avg_fps.is_finite() {
            errors.push("Average FPS must be a finite number".to_string());
        }
        if self.avg_fps < 1.0 {
            errors.push(
                "Average FPS must be at least 1. Did you enter the correct value?".to_string(),
            );
        }
        if self.avg_fps > 500.0 {
            errors.push(format!(
                "Average FPS of {:.0} is above current tracker API limits (max 500). Please verify your value.",
                self.avg_fps
            ));
        }

        // 1% low should be lower than average
        if let Some(fps_1_low) = self.fps_1_low {
            if fps_1_low > self.avg_fps {
                errors.push("1% low FPS should be lower than average FPS (it measures worst-case performance)".to_string());
            }
            if fps_1_low < 1.0 {
                errors.push("1% low FPS must be at least 1".to_string());
            }
            if fps_1_low > 500.0 {
                errors.push("1% low FPS must be 500 or below".to_string());
            }
            if !fps_1_low.is_finite() {
                errors.push("1% low FPS must be a finite number".to_string());
            }
        }

        if let Some(fps_01_low) = self.fps_01_low {
            if !fps_01_low.is_finite() {
                errors.push("0.1% low FPS must be a finite number".to_string());
            } else {
                if fps_01_low < 1.0 {
                    errors.push("0.1% low FPS must be at least 1".to_string());
                }
                if fps_01_low > 500.0 {
                    errors.push("0.1% low FPS must be 500 or below".to_string());
                }
                if fps_01_low > self.avg_fps {
                    errors.push("0.1% low FPS should be lower than average FPS".to_string());
                }
                if let Some(fps_1_low) = self.fps_1_low {
                    if fps_01_low > fps_1_low {
                        errors
                            .push("0.1% low FPS should not be higher than 1% low FPS".to_string());
                    }
                }
            }
        }

        // Resolution check
        if normalize_resolution(&self.resolution).is_none() {
            errors.push(format!(
                "Resolution '{}' not recognized. Use 1080p, 1440p, 4K, or dimensions like 1920x1080",
                self.resolution
            ));
        }

        if let Some(score) = self.capture_quality_score {
            if score > 100 {
                errors.push("Capture quality score must be between 0 and 100".to_string());
            }
        }

        if let Some(tool) = self.benchmark_tool.as_deref() {
            let trimmed = tool.trim();
            if trimmed.is_empty() {
                errors.push("Benchmark tool cannot be empty when provided".to_string());
            }
            if trimmed.len() > 64 {
                errors.push("Benchmark tool is too long (max 64 characters)".to_string());
            }
        }

        if let Some(method) = self.capture_method.as_deref() {
            let normalized = method.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                errors.push("Capture method cannot be empty when provided".to_string());
            } else if !matches!(
                normalized.as_str(),
                "in_game_counter"
                    | "built_in_benchmark"
                    | "external_tool"
                    | "captured"
                    | "manual_entry"
            ) {
                errors.push(format!(
                    "Capture method '{}' is not recognized",
                    method.trim()
                ));
            }
        }

        // Game name sanity
        if self.game.trim().is_empty() {
            errors.push("Game name cannot be empty".to_string());
        }
        if self.game.trim().len() > 120 {
            errors.push("Game name is too long (max 120 characters)".to_string());
        }

        // Preset sanity
        if self.preset.trim().is_empty() {
            errors.push("Graphics preset cannot be empty".to_string());
        }

        if is_placeholder_hardware_name(&self.system_info.gpu.name) {
            errors.push(
                "GPU model is missing or placeholder. Please provide your real GPU model."
                    .to_string(),
            );
        }

        if is_placeholder_hardware_name(&self.system_info.cpu.name) {
            errors.push(
                "CPU model is missing or placeholder. Please provide your real CPU model."
                    .to_string(),
            );
        }

        if self.system_info.cpu.cores == 0 {
            errors.push("CPU core count must be greater than 0".to_string());
        }
        if self.system_info.cpu.threads == 0 {
            errors.push("CPU thread count must be greater than 0".to_string());
        }
        if self.system_info.cpu.threads < self.system_info.cpu.cores {
            errors.push("CPU thread count cannot be lower than core count".to_string());
        }

        let ram_mb = self
            .system_info
            .ram
            .installed_mb
            .unwrap_or(self.system_info.ram.usable_mb);
        if ram_mb == 0 {
            errors.push(
                "RAM amount is missing. Please provide detected or manual RAM size.".to_string(),
            );
        }

        if let Some(samples) = self.sample_count {
            if samples == 0 {
                errors.push("Sample count must be greater than 0 when provided".to_string());
            }
        }

        if let Some(duration_secs) = self.duration_secs {
            if !duration_secs.is_finite() || duration_secs <= 0.0 {
                errors.push(
                    "Session duration must be a positive finite number when provided".to_string(),
                );
            }
        }

        if self.anti_cheat_strict_acknowledged == Some(true)
            && self.anti_cheat_acknowledged == Some(false)
        {
            errors.push(
                "Strict anti-cheat acknowledgment requires anti-cheat acknowledgment as well."
                    .to_string(),
            );
        }

        if let Some(score) = self.synthetic_cpu_score {
            if score == 0 {
                errors.push("Synthetic CPU score must be greater than 0 when provided".to_string());
            }
        }
        if self.synthetic_cpu_score.is_none()
            && self
                .synthetic_cpu_source
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
        {
            errors.push("Synthetic CPU source cannot be set without a CPU score".to_string());
        }
        if let Some(source) = self.synthetic_cpu_source.as_deref() {
            if !source.trim().is_empty() && !is_valid_synthetic_source(source) {
                errors.push("Synthetic CPU source contains unsupported characters".to_string());
            }
        }
        if let Some(score) = self.synthetic_gpu_score {
            if score == 0 {
                errors.push("Synthetic GPU score must be greater than 0 when provided".to_string());
            }
        }
        if self.synthetic_gpu_score.is_none()
            && self
                .synthetic_gpu_source
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
        {
            errors.push("Synthetic GPU source cannot be set without a GPU score".to_string());
        }
        if let Some(source) = self.synthetic_gpu_source.as_deref() {
            if !source.trim().is_empty() && !is_valid_synthetic_source(source) {
                errors.push("Synthetic GPU source contains unsupported characters".to_string());
            }
        }
        if let Some(score) = self.synthetic_ram_score {
            if score == 0 {
                errors.push("Synthetic RAM score must be greater than 0 when provided".to_string());
            }
        }
        if self.synthetic_ram_score.is_none()
            && self
                .synthetic_ram_source
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
        {
            errors.push("Synthetic RAM source cannot be set without a RAM score".to_string());
        }
        if let Some(source) = self.synthetic_ram_source.as_deref() {
            if !source.trim().is_empty() && !is_valid_synthetic_source(source) {
                errors.push("Synthetic RAM source contains unsupported characters".to_string());
            }
        }
        if let Some(score) = self.synthetic_disk_score {
            if score == 0 {
                errors
                    .push("Synthetic disk score must be greater than 0 when provided".to_string());
            }
        }
        if self.synthetic_disk_score.is_none()
            && self
                .synthetic_disk_source
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
        {
            errors.push("Synthetic disk source cannot be set without a disk score".to_string());
        }
        if let Some(source) = self.synthetic_disk_source.as_deref() {
            if !source.trim().is_empty() && !is_valid_synthetic_source(source) {
                errors.push("Synthetic disk source contains unsupported characters".to_string());
            }
        }
        if let Some(profile) = self.synthetic_profile.as_deref() {
            let normalized = profile.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                errors.push("Synthetic profile cannot be empty when provided".to_string());
            } else if !matches!(normalized.as_str(), "quick" | "standard" | "extended") {
                errors.push("Synthetic profile must be quick, standard, or extended".to_string());
            }
        }

        if let Some(ref mode) = self.upscaling {
            if mode.trim().len() > 64 {
                errors.push("Upscaling mode is too long (max 64 characters)".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn is_valid_synthetic_source(source: &str) -> bool {
    let trimmed = source.trim();
    if trimmed.is_empty() || trimmed.len() > 32 {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
}

fn normalize_resolution(resolution: &str) -> Option<String> {
    let normalized = resolution.trim().to_ascii_lowercase().replace(' ', "");
    if normalized.is_empty() {
        return None;
    }

    if let Some(mapped) = match normalized.as_str() {
        "720p" | "1280x720" => Some("720p"),
        "900p" | "1600x900" => Some("900p"),
        "1080p" | "1920x1080" | "fullhd" | "fhd" => Some("1080p"),
        "1440p" | "2560x1440" | "qhd" | "2k" | "wqhd" => Some("1440p"),
        "2160p" | "4k" | "3840x2160" | "uhd" => Some("4K"),
        "3440x1440" | "uwqhd" => Some("3440x1440"),
        "5k" | "5120x2880" => Some("5K"),
        "8k" | "7680x4320" => Some("8K"),
        _ => None,
    } {
        return Some(mapped.to_string());
    }

    if normalized.ends_with('p') {
        let number = normalized.trim_end_matches('p');
        if let Ok(value) = number.parse::<u16>() {
            return match value {
                720 | 900 | 1080 | 1200 | 1440 | 1600 | 1800 | 2160 | 4320 => {
                    Some(format!("{value}p"))
                }
                _ => None,
            };
        }
    }

    // Accept plain shorthand values: 1080, 1440, 2160...
    if let Ok(value) = normalized.parse::<u16>() {
        return match value {
            720 | 900 | 1080 | 1200 | 1440 | 1600 | 1800 => Some(format!("{value}p")),
            2160 => Some("4K".to_string()),
            4320 => Some("8K".to_string()),
            _ => None,
        };
    }

    None
}

fn is_placeholder_hardware_name(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "unknown"
        || normalized == "unknown gpu"
        || normalized == "unknown cpu"
        || normalized.contains("browser fallback")
        || normalized == "generic processor"
        || normalized.ends_with("-core processor")
}

/// Response from the API after successful submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResponse {
    /// Assigned submission ID
    #[serde(default, alias = "contribution_id")]
    pub id: String,
    /// Server message
    #[serde(default)]
    pub message: String,
    /// Points earned (gamification)
    #[serde(default)]
    pub points: Option<u32>,
    /// New contribution total
    #[serde(default)]
    pub total_contributions: Option<u32>,
    /// Tracker API status (accepted/rejected)
    #[serde(default)]
    pub status: Option<String>,
    /// Number of sessions accepted by tracker API
    #[serde(default)]
    pub sessions_accepted: Option<u32>,
    /// Number of sessions rejected by tracker API
    #[serde(default)]
    pub sessions_rejected: Option<u32>,
    /// Human-readable rejection reasons from tracker API
    #[serde(default)]
    pub rejection_reasons: Vec<String>,
}

impl SubmissionResponse {
    pub fn effective_id(&self) -> Option<&str> {
        let id = self.id.trim();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }

    pub fn is_rejected(&self) -> bool {
        self.status
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("rejected"))
            .unwrap_or(false)
    }

    pub fn rejection_summary(&self) -> String {
        if !self.rejection_reasons.is_empty() {
            return self.rejection_reasons.join(", ");
        }
        if self.message.trim().is_empty() {
            "Submission rejected by server".to_string()
        } else {
            self.message.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_resolution;
    use crate::hardware::cpu::CpuInfo;
    use crate::hardware::gpu::{GpuInfo, GpuVendor};
    use crate::hardware::ram::RamInfo;
    use crate::hardware::SystemInfo;

    use super::BenchmarkSubmission;

    fn mock_system_info() -> SystemInfo {
        SystemInfo {
            gpu: GpuInfo {
                name: "NVIDIA RTX 4070 SUPER".to_string(),
                vendor: GpuVendor::Nvidia,
                vram_mb: Some(12288),
                driver_version: Some("551.23".to_string()),
                pci_id: None,
                gpu_clock_mhz: None,
                memory_clock_mhz: None,
                temperature_c: None,
                utilization_percent: None,
            },
            cpu: CpuInfo {
                name: "AMD Ryzen 7 7800X3D".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: Some(4200),
                vendor: "AMD".to_string(),
                architecture: Some("x86_64".to_string()),
                max_frequency_mhz: Some(5000),
            },
            ram: RamInfo {
                installed_mb: Some(32768),
                usable_mb: 31990,
                speed_mhz: Some(6000),
                ram_type: Some("DDR5".to_string()),
                stick_count: Some(2),
                model: Some("Test RAM".to_string()),
            },
            os: "Linux".to_string(),
            os_version: Some("6.8".to_string()),
        }
    }

    #[test]
    fn accepts_dimension_based_resolution() {
        let submission = BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1920x1080".to_string(),
            "Ultra".to_string(),
            95.0,
            Some(72.0),
            false,
            Some("DLSS Quality".to_string()),
        );
        assert!(submission.validate().is_ok());
    }

    #[test]
    fn rejects_empty_preset() {
        let submission = BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "   ".to_string(),
            95.0,
            Some(72.0),
            false,
            None,
        );
        assert!(submission.validate().is_err());
    }

    #[test]
    fn normalize_resolution_maps_common_values() {
        assert_eq!(normalize_resolution("2560x1440").as_deref(), Some("1440p"));
        assert_eq!(normalize_resolution("3840x2160").as_deref(), Some("4K"));
        assert_eq!(normalize_resolution("  1080p ").as_deref(), Some("1080p"));
        assert_eq!(normalize_resolution("1080").as_deref(), Some("1080p"));
        assert_eq!(normalize_resolution("1440").as_deref(), Some("1440p"));
        assert_eq!(normalize_resolution("2160").as_deref(), Some("4K"));
        assert_eq!(normalize_resolution("2k").as_deref(), Some("1440p"));
        assert_eq!(normalize_resolution("fhd").as_deref(), Some("1080p"));
        assert_eq!(
            normalize_resolution("3440x1440").as_deref(),
            Some("3440x1440")
        );
    }

    #[test]
    fn rejects_placeholder_hardware_values() {
        let mut info = mock_system_info();
        info.gpu.name = "Unknown GPU (browser fallback)".to_string();
        info.cpu.name = "Generic 8-Core Processor".to_string();
        info.ram.installed_mb = Some(0);
        info.ram.usable_mb = 0;

        let submission = BenchmarkSubmission::new(
            info,
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            120.0,
            Some(90.0),
            false,
            None,
        );

        assert!(submission.validate().is_err());
    }

    #[test]
    fn rejects_unknown_capture_method() {
        let mut submission = BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            120.0,
            Some(90.0),
            false,
            None,
        );
        submission.capture_method = Some("mystery".to_string());

        assert!(submission.validate().is_err());
    }

    #[test]
    fn rejects_zero_synthetic_scores() {
        let mut submission = BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            120.0,
            Some(90.0),
            false,
            None,
        );
        submission.synthetic_cpu_score = Some(0);
        submission.synthetic_gpu_score = Some(0);
        submission.synthetic_ram_score = Some(0);
        submission.synthetic_disk_score = Some(0);

        assert!(submission.validate().is_err());
    }

    #[test]
    fn validates_synthetic_profile_values() {
        let mut submission = BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            120.0,
            Some(90.0),
            false,
            None,
        );

        submission.synthetic_profile = Some("standard".to_string());
        assert!(submission.validate().is_ok());

        submission.synthetic_profile = Some("turbo".to_string());
        assert!(submission.validate().is_err());
    }
}
