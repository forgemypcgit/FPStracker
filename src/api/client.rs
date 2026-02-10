//! API client for PC Builder backend
//!
//! Handles all HTTP communication with the backend API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

use crate::benchmark::{BenchmarkSubmission, SubmissionResponse};
use crate::config::Config;

/// API base URL (can be overridden via environment variable)
const DEFAULT_API_URL: &str = "http://localhost:8000";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_RETRIES: u32 = 2;

#[derive(Debug, Serialize)]
struct TrackerSubmissionPayload {
    hardware: TrackerHardwarePayload,
    sessions: Vec<TrackerSessionPayload>,
    tracker_version: Option<String>,
    source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    benchmark_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capture_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scene_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_url: Option<String>,
    consent_public_use: bool,
    legal_attestation: bool,
}

#[derive(Debug, Serialize)]
struct TrackerHardwarePayload {
    gpu: String,
    cpu: String,
    ram_gb: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    ram_speed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    driver_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrackerSessionPayload {
    game: String,
    resolution: String,
    quality_preset: String,
    ray_tracing: String,
    upscaling: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    upscaling_mode: Option<String>,
    fps_avg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    fps_1_low: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fps_0_1_low: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_duration_minutes: Option<u32>,
}

/// API errors
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Consent required: {0}")]
    ConsentRequired(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Server unreachable")]
    #[allow(dead_code)]
    Unreachable,
}

/// API client for PC Builder
pub struct ApiClient {
    client: Client,
    base_url: String,
    max_retries: u32,
    consent_public_use: bool,
    legal_attestation: bool,
}

impl ApiClient {
    /// Create a new API client
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();

        let base_url = std::env::var("PCBUILDER_API_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| config.api.base_url.clone());

        let timeout_seconds = std::env::var("PCBUILDER_API_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| config.api.timeout_seconds.max(1));

        let verify_ssl =
            parse_bool_env("PCBUILDER_API_VERIFY_SSL").unwrap_or(config.api.verify_ssl);

        let max_retries = std::env::var("PCBUILDER_API_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(config.api.max_retries);

        let consent_public_use = config.consent.consent_public_use;
        let legal_attestation =
            config.consent.tos_accepted && config.consent.retention_acknowledged;

        Self::with_settings(
            base_url,
            timeout_seconds,
            verify_ssl,
            max_retries,
            consent_public_use,
            legal_attestation,
        )
    }

    /// Create with custom base URL
    #[allow(dead_code)]
    pub fn with_url(base_url: String) -> Self {
        let config = Config::load().unwrap_or_default();
        let consent_public_use = config.consent.consent_public_use;
        let legal_attestation =
            config.consent.tos_accepted && config.consent.retention_acknowledged;

        Self::with_settings(
            base_url,
            DEFAULT_TIMEOUT_SECONDS,
            true,
            DEFAULT_MAX_RETRIES,
            consent_public_use,
            legal_attestation,
        )
    }

    fn with_settings(
        base_url: String,
        timeout_seconds: u64,
        verify_ssl: bool,
        max_retries: u32,
        consent_public_use: bool,
        legal_attestation: bool,
    ) -> Self {
        let timeout = Duration::from_secs(timeout_seconds.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(!verify_ssl)
            .build()
            .unwrap_or_else(|_| Client::new());

        ApiClient {
            client,
            base_url: normalize_base_url(&base_url),
            max_retries,
            consent_public_use,
            legal_attestation,
        }
    }

    /// Check if API is reachable
    #[allow(dead_code)]
    pub async fn health_check(&self) -> Result<bool, ApiError> {
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Submit a benchmark
    pub async fn submit_benchmark(
        &self,
        submission: &BenchmarkSubmission,
    ) -> Result<SubmissionResponse, ApiError> {
        // Validate before submitting
        if let Err(errors) = submission.validate() {
            return Err(ApiError::Validation(errors.join(", ")));
        }

        if !self.consent_public_use || !self.legal_attestation {
            let hint = "Run `fps-tracker start` (terminal) or `fps-tracker ui` (browser) and accept the consent terms before submitting.";
            return Err(ApiError::ConsentRequired(hint.to_string()));
        }

        let url = format!("{}/api/v2/tracker/submit", self.base_url);
        let max_attempts = self.max_retries.saturating_add(1).max(1);
        let payload = to_tracker_submission_payload(
            submission,
            self.consent_public_use,
            self.legal_attestation,
        );

        for attempt in 1..=max_attempts {
            let response_result = self.client.post(&url).json(&payload).send().await;

            let response = match response_result {
                Ok(response) => response,
                Err(err) => {
                    if attempt < max_attempts && is_retryable_network_error(&err) {
                        sleep(backoff_for_attempt(attempt)).await;
                        continue;
                    }
                    return Err(ApiError::Network(err));
                }
            };

            let status = response.status();

            if status.is_success() {
                let result: SubmissionResponse = response.json().await?;
                if result.is_rejected() {
                    return Err(ApiError::Api {
                        status: 422,
                        message: result.rejection_summary(),
                    });
                }
                return Ok(result);
            }

            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if attempt < max_attempts && status.is_server_error() {
                sleep(backoff_for_attempt(attempt)).await;
                continue;
            }

            return Err(ApiError::Api {
                status: status.as_u16(),
                message: error_body,
            });
        }

        Err(ApiError::Unreachable)
    }

    /// Get leaderboard (placeholder)
    #[allow(dead_code)]
    pub async fn get_leaderboard(&self) -> Result<Vec<LeaderboardEntry>, ApiError> {
        let url = format!("{}/api/v2/tracker/leaderboard", self.base_url);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let entries: Vec<LeaderboardEntry> = response.json().await?;
            Ok(entries)
        } else {
            Err(ApiError::Api {
                status: response.status().as_u16(),
                message: "Failed to fetch leaderboard".to_string(),
            })
        }
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub username: String,
    pub contributions: u32,
    pub points: u32,
}

/// Convenience function to submit a benchmark
#[allow(dead_code)]
pub async fn submit_benchmark(
    submission: &BenchmarkSubmission,
) -> Result<SubmissionResponse, ApiError> {
    let client = ApiClient::new();
    client.submit_benchmark(submission).await
}

fn to_tracker_submission_payload(
    submission: &BenchmarkSubmission,
    consent_public_use: bool,
    legal_attestation: bool,
) -> TrackerSubmissionPayload {
    let ram_mb = submission
        .system_info
        .ram
        .installed_mb
        .unwrap_or(submission.system_info.ram.usable_mb);
    let ram_gb = ((ram_mb as f64) / 1024.0).round() as u32;
    let ram_gb = ram_gb.clamp(4, 512);

    let ram_speed = submission
        .system_info
        .ram
        .speed_mhz
        .and_then(|speed| u32::try_from(speed).ok())
        .map(|speed| speed.clamp(1600, 10_000));

    let os = if let Some(version) = submission.system_info.os_version.as_deref() {
        Some(format!("{} {}", submission.system_info.os, version))
    } else {
        Some(submission.system_info.os.clone())
    };

    let upscaling_mode = submission
        .upscaling
        .as_ref()
        .map(|mode| mode.trim())
        .filter(|mode| !mode.is_empty())
        .map(str::to_string);
    let upscaling = upscaling_mode.clone().unwrap_or_else(|| "Off".to_string());

    let session_duration_minutes = submission.duration_secs.map(|secs| {
        let rounded = (secs / 60.0).ceil();
        rounded.max(1.0) as u32
    });

    let capture_method = if submission.sample_count.is_some() || submission.duration_secs.is_some()
    {
        Some("captured".to_string())
    } else {
        Some("manual_entry".to_string())
    };

    TrackerSubmissionPayload {
        hardware: TrackerHardwarePayload {
            gpu: submission.system_info.gpu.name.clone(),
            cpu: submission.system_info.cpu.name.clone(),
            ram_gb,
            ram_speed,
            storage_type: None,
            os,
            driver_version: submission.system_info.gpu.driver_version.clone(),
        },
        sessions: vec![TrackerSessionPayload {
            game: submission.game.clone(),
            resolution: to_tracker_resolution(&submission.resolution),
            quality_preset: submission.preset.clone(),
            ray_tracing: if submission.ray_tracing {
                "On".to_string()
            } else {
                "Off".to_string()
            },
            upscaling,
            upscaling_mode,
            fps_avg: submission.avg_fps,
            fps_1_low: submission.fps_1_low,
            fps_0_1_low: submission.fps_01_low,
            session_duration_minutes,
        }],
        tracker_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        source_type: "self_recorded".to_string(),
        benchmark_tool: None,
        capture_method,
        scene_tag: None,
        evidence_url: None,
        consent_public_use,
        legal_attestation,
    }
}

fn to_tracker_resolution(resolution: &str) -> String {
    let normalized = resolution.trim().to_ascii_lowercase().replace(' ', "");

    if normalized.is_empty() {
        return "1920x1080".to_string();
    }

    match normalized.as_str() {
        "720p" => return "1280x720".to_string(),
        "900p" => return "1600x900".to_string(),
        "1080p" | "fhd" => return "1920x1080".to_string(),
        "1200p" => return "1920x1200".to_string(),
        "1440p" | "qhd" | "2k" => return "2560x1440".to_string(),
        "1600p" => return "2560x1600".to_string(),
        "1800p" => return "3200x1800".to_string(),
        "4k" | "2160p" | "uhd" => return "3840x2160".to_string(),
        "8k" | "4320p" => return "7680x4320".to_string(),
        _ => {}
    }

    if let Some((width, height)) = parse_resolution_dimensions(&normalized) {
        return format!("{}x{}", width, height);
    }

    if let Ok(height) = normalized.trim_end_matches('p').parse::<u32>() {
        return match height {
            720 => "1280x720".to_string(),
            900 => "1600x900".to_string(),
            1080 => "1920x1080".to_string(),
            1200 => "1920x1200".to_string(),
            1440 => "2560x1440".to_string(),
            1600 => "2560x1600".to_string(),
            1800 => "3200x1800".to_string(),
            2160 => "3840x2160".to_string(),
            4320 => "7680x4320".to_string(),
            _ => "1920x1080".to_string(),
        };
    }

    "1920x1080".to_string()
}

fn parse_resolution_dimensions(value: &str) -> Option<(u32, u32)> {
    let (width_text, height_text) = value.split_once('x')?;
    let width = width_text.parse::<u32>().ok()?;
    let height = height_text.parse::<u32>().ok()?;
    if width < 320 || height < 200 {
        return None;
    }
    Some((width, height))
}

fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return DEFAULT_API_URL.to_string();
    }
    trimmed.trim_end_matches('/').to_string()
}

fn parse_bool_env(key: &str) -> Option<bool> {
    let value = std::env::var(key).ok()?;
    parse_bool_value(&value)
}

fn parse_bool_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn is_retryable_network_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

fn backoff_for_attempt(attempt: u32) -> Duration {
    // 200ms, 400ms, 800ms ... capped at 2s
    let exponent = attempt.saturating_sub(1).min(4);
    let factor = 2u64.saturating_pow(exponent);
    let ms = 200u64.saturating_mul(factor).min(2_000);
    Duration::from_millis(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new();
        assert!(client.base_url.starts_with("http"));
    }

    #[test]
    fn test_custom_url() {
        let client = ApiClient::with_url("https://api.example.com".to_string());
        assert_eq!(client.base_url, "https://api.example.com");
    }

    #[test]
    fn test_normalize_base_url() {
        assert_eq!(
            normalize_base_url("https://api.example.com/"),
            "https://api.example.com"
        );
        assert_eq!(normalize_base_url(""), DEFAULT_API_URL);
    }

    #[test]
    fn test_parse_bool_variants() {
        assert_eq!(parse_bool_value("true"), Some(true));
        assert_eq!(parse_bool_value("1"), Some(true));
        assert_eq!(parse_bool_value("no"), Some(false));
        assert_eq!(parse_bool_value("0"), Some(false));
        assert_eq!(parse_bool_value("maybe"), None);
    }

    #[test]
    fn test_resolution_conversion() {
        assert_eq!(to_tracker_resolution("1080p"), "1920x1080");
        assert_eq!(to_tracker_resolution("1440p"), "2560x1440");
        assert_eq!(to_tracker_resolution("4K"), "3840x2160");
        assert_eq!(to_tracker_resolution("2560x1440"), "2560x1440");
    }
}
