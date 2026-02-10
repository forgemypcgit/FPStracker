//! Benchmark submission
//!
//! Data structure for submitting benchmarks to the PC Builder API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::hardware::SystemInfo;

/// A benchmark submission to the PC Builder API
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

        if let Some(fps_1_low) = self.fps_1_low {
            output.push_str(&format!("  1% Low: {:.1}\n", fps_1_low));
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
                    errors.push("0.1% low FPS should not be higher than 1% low FPS".to_string());
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
    /// One-time delete token for contribution removal (privacy)
    #[serde(default)]
    pub delete_token: Option<String>,
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
}
