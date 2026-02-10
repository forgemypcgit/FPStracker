//! Configuration management for fps-tracker
//!
//! Config file location:
//! - Linux: ~/.config/fps-tracker/config.toml
//! - macOS: ~/Library/Application Support/fps-tracker/config.toml
//! - Windows: %APPDATA%/fps-tracker/config.toml
//!
//! You can override the config location by setting `FPS_TRACKER_CONFIG_PATH`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// API endpoint configuration
    #[serde(default)]
    pub api: ApiConfig,

    /// User preferences
    #[serde(default)]
    pub user: UserConfig,

    /// Consent required before any benchmark submission
    #[serde(default)]
    pub consent: ConsentConfig,

    /// Benchmark settings
    #[serde(default)]
    pub benchmark: BenchmarkConfig,

    /// Build check defaults
    #[serde(default)]
    pub build_check: BuildCheckConfig,

    /// Live capture behavior defaults
    #[serde(default)]
    pub capture: CaptureConfig,

    /// Distribution/update metadata
    #[serde(default)]
    pub distribution: DistributionConfig,
}

impl Config {
    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

            let config: Config = toml::from_str(&content).with_context(|| {
                format!("Failed to parse config from {}", config_path.display())
            })?;

            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let toml = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        fs::write(&config_path, toml)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        if let Ok(path) = std::env::var("FPS_TRACKER_CONFIG_PATH") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }

        let proj_dirs = ProjectDirs::from("com", "pcbuilder", "fps-tracker")
            .context("Could not determine project directories")?;

        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    /// Create default config file if it doesn't exist
    pub fn init() -> Result<Self> {
        let config = Self::load()?;

        // Save default config if file doesn't exist
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            config.save()?;
        }

        Ok(config)
    }
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API base URL
    #[serde(default = "default_api_url")]
    pub base_url: String,

    /// API timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Whether to verify SSL certificates
    #[serde(default = "default_true")]
    pub verify_ssl: bool,

    /// Number of retry attempts for transient network errors
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: default_api_url(),
            timeout_seconds: default_timeout(),
            verify_ssl: default_true(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_api_url() -> String {
    "https://aipcbuilderbackend-ikpg65blaq-uc.a.run.app".to_string()
}

fn default_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u32 {
    2
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// User ID for tracking contributions (optional)
    pub contributor_id: Option<String>,

    /// Default region for pricing data
    pub region: Option<String>,

    /// Preferred currency
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Custom key-value pairs for user preferences
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            contributor_id: None,
            region: None,
            currency: default_currency(),
            custom: HashMap::new(),
        }
    }
}

fn default_currency() -> String {
    "USD".to_string()
}

/// User consent for benchmark submission.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsentConfig {
    /// Accept the Terms of Service.
    #[serde(default)]
    pub tos_accepted: bool,

    /// Allow anonymized submission data to be used publicly (including aggregate stats).
    #[serde(default)]
    pub consent_public_use: bool,

    /// Acknowledge retention policy.
    #[serde(default)]
    pub retention_acknowledged: bool,

    /// Timestamp when consent was accepted (UTC).
    #[serde(default)]
    pub accepted_at_utc: Option<DateTime<Utc>>,
}

impl ConsentConfig {
    pub fn is_complete(&self) -> bool {
        self.tos_accepted && self.consent_public_use && self.retention_acknowledged
    }
}

/// Benchmark settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Auto-detect CapFrameX captures
    #[serde(default = "default_true")]
    pub auto_detect_capframex: bool,

    /// Auto-detect MangoHud logs
    #[serde(default = "default_true")]
    pub auto_detect_mangohud: bool,

    /// Default CapFrameX capture directory
    pub capframex_dir: Option<String>,

    /// Default MangoHud log directory
    pub mangohud_dir: Option<String>,

    /// Minimum FPS for validation
    #[serde(default = "default_min_fps")]
    pub min_fps: f64,

    /// Maximum FPS for validation (sanity check)
    #[serde(default = "default_max_fps")]
    pub max_fps: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            auto_detect_capframex: true,
            auto_detect_mangohud: true,
            capframex_dir: None,
            mangohud_dir: None,
            min_fps: default_min_fps(),
            max_fps: default_max_fps(),
        }
    }
}

fn default_min_fps() -> f64 {
    1.0
}

fn default_max_fps() -> f64 {
    1000.0
}

/// Build check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCheckConfig {
    /// Warn if PSU wattage is below this multiplier of TDP
    #[serde(default = "default_psu_headroom")]
    pub psu_headroom_multiplier: f64,

    /// Warn if cooler TDP rating is below this multiplier of CPU TDP
    #[serde(default = "default_cooler_headroom")]
    pub cooler_headroom_multiplier: f64,

    /// Minimum GPU clearance margin in mm
    #[serde(default = "default_gpu_margin")]
    pub gpu_clearance_margin_mm: u32,

    /// Minimum CPU cooler height margin in mm
    #[serde(default = "default_cooler_margin")]
    pub cooler_height_margin_mm: u32,

    /// Enable strict compatibility checking
    #[serde(default)]
    pub strict_mode: bool,
}

impl Default for BuildCheckConfig {
    fn default() -> Self {
        Self {
            psu_headroom_multiplier: default_psu_headroom(),
            cooler_headroom_multiplier: default_cooler_headroom(),
            gpu_clearance_margin_mm: default_gpu_margin(),
            cooler_height_margin_mm: default_cooler_margin(),
            strict_mode: false,
        }
    }
}

fn default_psu_headroom() -> f64 {
    1.2 // 20% headroom
}

fn default_cooler_headroom() -> f64 {
    1.2 // 20% headroom
}

fn default_gpu_margin() -> u32 {
    20 // 20mm clearance
}

fn default_cooler_margin() -> u32 {
    10 // 10mm clearance
}

/// Live capture focus policy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusPolicy {
    Strict,
    Lenient,
}

fn default_focus_policy() -> FocusPolicy {
    FocusPolicy::Strict
}

fn default_poll_ms() -> u64 {
    100
}

fn default_process_validation() -> bool {
    true
}

fn default_max_frame_time_ms() -> f64 {
    250.0
}

fn default_strict_unfocus_grace_ms() -> u64 {
    1000
}

/// Live capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Focus policy used while live capture runs.
    #[serde(default = "default_focus_policy")]
    pub focus_policy: FocusPolicy,

    /// Pause and drop samples while target process/window is unfocused.
    #[serde(default = "default_true")]
    pub pause_on_unfocus: bool,

    /// Default tail polling interval in milliseconds.
    #[serde(default = "default_poll_ms")]
    pub default_poll_ms: u64,

    /// Enable strict process validation.
    #[serde(default = "default_process_validation")]
    pub process_validation: bool,

    /// Reject frame times above this threshold as likely invalid capture noise.
    #[serde(default = "default_max_frame_time_ms")]
    pub max_frame_time_ms: f64,

    /// Grace period for strict focus mode before capture is considered invalid.
    #[serde(default = "default_strict_unfocus_grace_ms")]
    pub strict_unfocus_grace_ms: u64,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            focus_policy: default_focus_policy(),
            pause_on_unfocus: true,
            default_poll_ms: default_poll_ms(),
            process_validation: default_process_validation(),
            max_frame_time_ms: default_max_frame_time_ms(),
            strict_unfocus_grace_ms: default_strict_unfocus_grace_ms(),
        }
    }
}

fn default_channel() -> String {
    "stable".to_string()
}

/// Distribution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionConfig {
    /// Update channel metadata for release automation.
    #[serde(default = "default_channel")]
    pub channel: String,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            channel: default_channel(),
        }
    }
}

/// Get configuration file path for display purposes
pub fn get_config_path() -> Result<String> {
    let path = Config::config_path()?;
    Ok(path.display().to_string())
}

/// Initialize configuration (load or create default)
pub fn init_config() -> Result<Config> {
    Config::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(
            config.api.base_url,
            "https://aipcbuilderbackend-ikpg65blaq-uc.a.run.app"
        );
        assert_eq!(config.api.timeout_seconds, 30);
        assert_eq!(config.user.currency, "USD");
        assert!(!config.consent.is_complete());
        assert!(config.benchmark.auto_detect_capframex);
        assert_eq!(config.capture.focus_policy, FocusPolicy::Strict);
        assert!(config.capture.pause_on_unfocus);
        assert!(config.capture.process_validation);
        assert_eq!(config.capture.default_poll_ms, 100);
        assert_eq!(config.capture.max_frame_time_ms, 250.0);
        assert_eq!(config.capture.strict_unfocus_grace_ms, 1000);
        assert_eq!(config.distribution.channel, "stable");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml = toml::to_string(&config).unwrap();

        assert!(toml.contains("base_url"));
        assert!(toml.contains("timeout_seconds"));
        assert!(toml.contains("currency"));
        assert!(toml.contains("[consent]"));
        assert!(toml.contains("focus_policy"));
        assert!(toml.contains("channel"));
    }
}
