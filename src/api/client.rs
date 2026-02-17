//! API client for the backend API
//!
//! Handles all HTTP communication with the backend API.

use crate::benchmark::{BenchmarkSubmission, SubmissionResponse};
use crate::config::Config;
use crate::feedback::{FeedbackBackendResponse, FeedbackSubmission};
use crate::idempotency;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

/// Default API base URL (may be overridden via config/env).
const DEFAULT_API_URL: &str = "https://fps-tracker-api-689034767510.us-central1.run.app";
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
    synthetic_cpu_score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_gpu_score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_ram_score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_disk_score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_suite_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    synthetic_extended: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_duration_minutes: Option<u32>,
}

#[derive(Debug, Serialize)]
struct TrackerFeedbackPayload {
    surface: String,
    category: String,
    issue_code: String,
    message: String,
    tracker_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<serde_json::Value>,
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

    #[error("Invalid API response: {0}")]
    InvalidResponse(String),

    #[error("Server unreachable")]
    #[allow(dead_code)]
    Unreachable,
}

/// API client for the backend service.
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

        let base_url = std::env::var("FPS_TRACKER_API_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| config.api.base_url.clone());

        let timeout_seconds = std::env::var("FPS_TRACKER_API_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| config.api.timeout_seconds.max(1));

        let verify_ssl =
            parse_bool_env("FPS_TRACKER_API_VERIFY_SSL").unwrap_or(config.api.verify_ssl);

        let max_retries = std::env::var("FPS_TRACKER_API_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(config.api.max_retries);

        let (consent_public_use, legal_attestation) = consent_flags(&config);

        Self::with_settings(
            &base_url,
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
        let (consent_public_use, legal_attestation) = consent_flags(&config);

        Self::with_settings(
            &base_url,
            DEFAULT_TIMEOUT_SECONDS,
            true,
            DEFAULT_MAX_RETRIES,
            consent_public_use,
            legal_attestation,
        )
    }

    fn with_settings(
        base_url: &str,
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

        Self {
            client,
            base_url: normalize_base_url(base_url),
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
        let idempotency_key = idempotency::new_submit_key();
        self.submit_benchmark_with_key(submission, &idempotency_key)
            .await
    }

    pub async fn submit_benchmark_with_key(
        &self,
        submission: &BenchmarkSubmission,
        idempotency_key: &str,
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
        let payload_full = to_tracker_submission_payload(
            submission,
            self.consent_public_use,
            self.legal_attestation,
        );
        let payload_legacy = to_tracker_submission_payload_with_extended(
            submission,
            self.consent_public_use,
            self.legal_attestation,
            false,
        );

        match self
            .submit_tracker_payload_with_key(&url, &payload_full, idempotency_key)
            .await
        {
            Ok(ok) => Ok(ok),
            Err(ApiError::Api { status, message }) => {
                let wants_extended = submission
                    .synthetic_profile
                    .as_deref()
                    .is_some_and(|v| !v.trim().is_empty())
                    || submission
                        .synthetic_suite_version
                        .as_deref()
                        .is_some_and(|v| !v.trim().is_empty())
                    || submission.synthetic_extended.is_some();

                if wants_extended && is_schema_rejection(status, &message) {
                    self.submit_tracker_payload_with_key(&url, &payload_legacy, idempotency_key)
                        .await
                } else {
                    Err(ApiError::Api { status, message })
                }
            }
            Err(err) => Err(err),
        }
    }

    async fn submit_tracker_payload_with_key(
        &self,
        url: &str,
        payload: &TrackerSubmissionPayload,
        idempotency_key: &str,
    ) -> Result<SubmissionResponse, ApiError> {
        let max_attempts = self.max_retries.saturating_add(1).max(1);

        for attempt in 1..=max_attempts {
            let response_result = self
                .client
                .post(url)
                .header("Idempotency-Key", idempotency_key)
                .header("X-Idempotency-Key", idempotency_key)
                .json(payload)
                .send()
                .await;

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
                let status_code = status.as_u16();
                let body = response.text().await.map_err(|err| ApiError::Api {
                    status: status_code,
                    message: format!(
                        "Failed to read successful response body (status {status_code}): {err}"
                    ),
                })?;

                let result: SubmissionResponse = serde_json::from_str(&body).map_err(|err| {
                    ApiError::InvalidResponse(format!(
                        "Invalid successful response payload (status {status_code}): {err}"
                    ))
                })?;
                if result.effective_id().is_none() && !result.is_rejected() {
                    return Err(ApiError::InvalidResponse(
                        "Successful response is missing submission_id".to_string(),
                    ));
                }
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

            if attempt < max_attempts && is_retryable_status(status.as_u16()) {
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

    /// Submit feedback (does not require consent flags).
    #[allow(dead_code)]
    pub async fn submit_feedback(
        &self,
        feedback: &FeedbackSubmission,
    ) -> Result<FeedbackBackendResponse, ApiError> {
        let idempotency_key = idempotency::new_feedback_key();
        self.submit_feedback_with_key(feedback, &idempotency_key)
            .await
    }

    pub async fn submit_feedback_with_key(
        &self,
        feedback: &FeedbackSubmission,
        idempotency_key: &str,
    ) -> Result<FeedbackBackendResponse, ApiError> {
        if let Err(errors) = feedback.validate() {
            return Err(ApiError::Validation(errors.join(", ")));
        }

        let url = format!("{}/api/v2/tracker/feedback", self.base_url);
        let max_attempts = self.max_retries.saturating_add(1).max(1);
        let payload = to_tracker_feedback_payload(feedback);

        for attempt in 1..=max_attempts {
            let response_result = self
                .client
                .post(&url)
                .header("Idempotency-Key", idempotency_key)
                .header("X-Idempotency-Key", idempotency_key)
                .json(&payload)
                .send()
                .await;

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
                let status_code = status.as_u16();
                let body = response.text().await.map_err(|err| ApiError::Api {
                    status: status_code,
                    message: format!(
                        "Failed to read successful response body (status {status_code}): {err}"
                    ),
                })?;

                let result: FeedbackBackendResponse =
                    serde_json::from_str(&body).map_err(|err| {
                        ApiError::InvalidResponse(format!(
                            "Invalid successful response payload (status {status_code}): {err}"
                        ))
                    })?;
                return Ok(result);
            }

            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if attempt < max_attempts && is_retryable_status(status.as_u16()) {
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
}

pub fn should_queue_offline(err: &ApiError) -> bool {
    matches!(
        err,
        ApiError::Network(_)
            | ApiError::Unreachable
            | ApiError::InvalidResponse(_)
            | ApiError::Api {
                status: 408 | 429 | 500..=u16::MAX,
                ..
            }
    )
}

pub fn should_queue_offline_feedback(err: &ApiError) -> bool {
    if should_queue_offline(err) {
        return true;
    }

    matches!(
        err,
        ApiError::Api {
            status: 404 | 501,
            ..
        }
    )
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

pub async fn submit_benchmark_with_idempotency_key(
    submission: &BenchmarkSubmission,
    idempotency_key: &str,
) -> Result<SubmissionResponse, ApiError> {
    let client = ApiClient::new();
    client
        .submit_benchmark_with_key(submission, idempotency_key)
        .await
}

pub async fn submit_feedback_with_idempotency_key(
    feedback: &FeedbackSubmission,
    idempotency_key: &str,
) -> Result<FeedbackBackendResponse, ApiError> {
    let client = ApiClient::new();
    client
        .submit_feedback_with_key(feedback, idempotency_key)
        .await
}

fn to_tracker_submission_payload(
    submission: &BenchmarkSubmission,
    consent_public_use: bool,
    legal_attestation: bool,
) -> TrackerSubmissionPayload {
    to_tracker_submission_payload_with_extended(
        submission,
        consent_public_use,
        legal_attestation,
        true,
    )
}

fn to_tracker_submission_payload_with_extended(
    submission: &BenchmarkSubmission,
    consent_public_use: bool,
    legal_attestation: bool,
    include_extended: bool,
) -> TrackerSubmissionPayload {
    let ram_mb = submission
        .system_info
        .ram
        .installed_mb
        .unwrap_or(submission.system_info.ram.usable_mb);
    let ram_gb = ((ram_mb as f64) / 1024.0).round() as u32;

    let ram_speed = submission
        .system_info
        .ram
        .speed_mhz
        .and_then(|speed| u32::try_from(speed).ok());

    let os = Some(submission.system_info.os_version.as_deref().map_or_else(
        || submission.system_info.os.clone(),
        |version| format!("{} {}", submission.system_info.os, version),
    ));

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

    let capture_method = submission
        .capture_method
        .as_deref()
        .and_then(normalize_capture_method)
        .or_else(|| {
            if submission.sample_count.is_some() || submission.duration_secs.is_some() {
                Some("captured".to_string())
            } else {
                Some("manual_entry".to_string())
            }
        });

    let capture_scene_tag = submission.capture_quality_score.map(|score| {
        let score = score.min(100);
        if submission.unstable_capture.unwrap_or(false) {
            format!("capture_q{score}_unstable")
        } else {
            format!("capture_q{score}")
        }
    });
    let synthetic_scene_tag = synthetic_scene_tag(submission);
    let scene_tag = match (capture_scene_tag, synthetic_scene_tag) {
        (Some(capture), Some(synthetic)) => Some(format!("{capture}_{synthetic}")),
        (Some(capture), None) => Some(capture),
        (None, Some(synthetic)) => Some(synthetic),
        (None, None) => None,
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
            synthetic_cpu_score: submission.synthetic_cpu_score,
            synthetic_gpu_score: submission.synthetic_gpu_score,
            synthetic_ram_score: submission.synthetic_ram_score,
            synthetic_disk_score: submission.synthetic_disk_score,
            synthetic_profile: if include_extended {
                submission
                    .synthetic_profile
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            },
            synthetic_suite_version: if include_extended {
                submission
                    .synthetic_suite_version
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            },
            synthetic_extended: if include_extended {
                submission.synthetic_extended.clone()
            } else {
                None
            },
            session_duration_minutes,
        }],
        tracker_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        source_type: "self_recorded".to_string(),
        benchmark_tool: submission.benchmark_tool.clone(),
        capture_method,
        scene_tag,
        evidence_url: None,
        consent_public_use,
        legal_attestation,
    }
}

fn to_tracker_feedback_payload(feedback: &FeedbackSubmission) -> TrackerFeedbackPayload {
    let surface = match feedback.surface {
        crate::feedback::FeedbackSurface::WebUi => "web_ui",
        crate::feedback::FeedbackSurface::TerminalUi => "terminal_ui",
    }
    .to_string();

    let category = serde_json::to_value(feedback.category)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "other".to_string());

    let diagnostics = feedback
        .diagnostics
        .as_ref()
        .and_then(|d| serde_json::to_value(d).ok());

    TrackerFeedbackPayload {
        surface,
        category,
        issue_code: feedback.issue_code.trim().to_string(),
        message: feedback.message.trim().to_string(),
        tracker_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        diagnostics,
    }
}

fn to_tracker_resolution(resolution: &str) -> String {
    let normalized = resolution.trim().to_ascii_lowercase().replace(' ', "");

    if normalized.is_empty() {
        return resolution.trim().to_string();
    }

    match normalized.as_str() {
        "720p" => return "1280x720".to_string(),
        "900p" => return "1600x900".to_string(),
        "1080p" | "fhd" => return "1920x1080".to_string(),
        "1200p" => return "1920x1200".to_string(),
        "1440p" | "qhd" | "2k" => return "2560x1440".to_string(),
        "1600p" => return "2560x1600".to_string(),
        "1800p" => return "3200x1800".to_string(),
        "5k" => return "5120x2880".to_string(),
        "4k" | "2160p" | "uhd" => return "3840x2160".to_string(),
        "8k" | "4320p" => return "7680x4320".to_string(),
        _ => {}
    }

    if let Some((width, height)) = parse_resolution_dimensions(&normalized) {
        return format!("{width}x{height}");
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
            2880 => "5120x2880".to_string(),
            2160 => "3840x2160".to_string(),
            4320 => "7680x4320".to_string(),
            _ => resolution.trim().to_string(),
        };
    }

    resolution.trim().to_string()
}

fn normalize_capture_method(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let mapped = match normalized.as_str() {
        "in_game_counter" => "in_game_counter",
        "built_in_benchmark" => "built_in_benchmark",
        "external_tool" => "external_tool",
        "manual_entry" => "manual_entry",
        "captured" => "captured",
        _ => return None,
    };
    Some(mapped.to_string())
}

fn synthetic_scene_tag(submission: &BenchmarkSubmission) -> Option<String> {
    let mut tags: Vec<String> = Vec::new();
    if let Some(profile) = submission
        .synthetic_profile
        .as_deref()
        .and_then(scene_tag_token)
    {
        tags.push(format!("syn_profile_{profile}"));
    }
    if let Some(cpu) = submission.synthetic_cpu_score {
        tags.push(format!("syn_cpu{cpu}"));
    }
    if let Some(source) = submission
        .synthetic_cpu_source
        .as_deref()
        .and_then(scene_tag_token)
    {
        tags.push(format!("syn_cpu_src_{source}"));
    }
    if let Some(gpu) = submission.synthetic_gpu_score {
        tags.push(format!("syn_gpu{gpu}"));
    }
    if let Some(source) = submission
        .synthetic_gpu_source
        .as_deref()
        .and_then(scene_tag_token)
    {
        tags.push(format!("syn_gpu_src_{source}"));
    }
    if let Some(ram) = submission.synthetic_ram_score {
        tags.push(format!("syn_ram{ram}"));
    }
    if let Some(source) = submission
        .synthetic_ram_source
        .as_deref()
        .and_then(scene_tag_token)
    {
        tags.push(format!("syn_ram_src_{source}"));
    }
    if let Some(disk) = submission.synthetic_disk_score {
        tags.push(format!("syn_disk{disk}"));
    }
    if let Some(source) = submission
        .synthetic_disk_source
        .as_deref()
        .and_then(scene_tag_token)
    {
        tags.push(format!("syn_disk_src_{source}"));
    }

    if tags.is_empty() {
        None
    } else {
        Some(tags.join("_"))
    }
}

fn scene_tag_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let token: String = trimmed
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let token = token.trim_matches('_').to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
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

const fn consent_flags(config: &Config) -> (bool, bool) {
    (
        config.consent.consent_public_use,
        config.consent.tos_accepted && config.consent.retention_acknowledged,
    )
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

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429) || (500..=599).contains(&status)
}

fn is_schema_rejection(status: u16, message: &str) -> bool {
    // Backends vary in how they report schema/unknown-field failures.
    // We treat common patterns as an instruction to retry with extended metrics stripped.
    if !(status == 400 || status == 422) {
        return false;
    }
    let m = message.to_ascii_lowercase();
    m.contains("unknown field")
        || m.contains("unrecognized field")
        || m.contains("unexpected field")
        || m.contains("additional properties")
        || m.contains("schema")
        || m.contains("invalid json")
        || m.contains("invalid payload")
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
    use crate::hardware::cpu::CpuInfo;
    use crate::hardware::gpu::{GpuInfo, GpuVendor};
    use crate::hardware::ram::RamInfo;
    use crate::hardware::SystemInfo;
    use chrono::Utc;
    use uuid::Uuid;

    fn submission_with_scores(
        synthetic_cpu_score: Option<u64>,
        synthetic_gpu_score: Option<u64>,
        synthetic_ram_score: Option<u64>,
        synthetic_disk_score: Option<u64>,
    ) -> BenchmarkSubmission {
        BenchmarkSubmission {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            system_info: SystemInfo {
                gpu: GpuInfo {
                    name: "NVIDIA RTX 4070 SUPER".to_string(),
                    vendor: GpuVendor::Nvidia,
                    vram_mb: Some(12_288),
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
                    installed_mb: Some(32_768),
                    usable_mb: 31_990,
                    speed_mhz: Some(6000),
                    ram_type: Some("DDR5".to_string()),
                    stick_count: Some(2),
                    model: Some("Test RAM".to_string()),
                },
                os: "Windows".to_string(),
                os_version: Some("11".to_string()),
            },
            game: "Cyberpunk 2077".to_string(),
            resolution: "1440p".to_string(),
            preset: "Ultra".to_string(),
            avg_fps: 120.0,
            fps_1_low: Some(95.0),
            fps_01_low: Some(82.0),
            ray_tracing: false,
            upscaling: Some("DLSS Quality".to_string()),
            frame_gen: None,
            sample_count: None,
            duration_secs: None,
            benchmark_tool: None,
            capture_quality_score: None,
            unstable_capture: None,
            capture_method: Some("manual_entry".to_string()),
            anti_cheat_acknowledged: None,
            anti_cheat_strict_acknowledged: None,
            synthetic_cpu_score,
            synthetic_cpu_source: None,
            synthetic_gpu_score,
            synthetic_gpu_source: None,
            synthetic_ram_score,
            synthetic_ram_source: None,
            synthetic_disk_score,
            synthetic_disk_source: None,
            synthetic_profile: None,
            synthetic_suite_version: None,
            synthetic_extended: None,
            notes: None,
        }
    }

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
        assert_eq!(to_tracker_resolution(""), "");
        assert_eq!(to_tracker_resolution("weird-res"), "weird-res");
    }

    #[test]
    fn test_capture_method_normalization() {
        assert_eq!(
            normalize_capture_method(" in_game_counter "),
            Some("in_game_counter".to_string())
        );
        assert_eq!(
            normalize_capture_method("built_in_benchmark"),
            Some("built_in_benchmark".to_string())
        );
        assert_eq!(normalize_capture_method(""), None);
        assert_eq!(normalize_capture_method("something-else"), None);
    }

    #[test]
    fn test_synthetic_scene_tag_generation() {
        assert_eq!(
            synthetic_scene_tag(&submission_with_scores(
                Some(12_345),
                Some(6_789),
                None,
                None
            )),
            Some("syn_cpu12345_syn_gpu6789".to_string())
        );
        assert_eq!(
            synthetic_scene_tag(&submission_with_scores(Some(12_345), None, None, None)),
            Some("syn_cpu12345".to_string())
        );
        assert_eq!(
            synthetic_scene_tag(&submission_with_scores(None, Some(6_789), None, None)),
            Some("syn_gpu6789".to_string())
        );
        assert_eq!(
            synthetic_scene_tag(&submission_with_scores(None, None, None, None)),
            None
        );
        assert_eq!(
            synthetic_scene_tag(&submission_with_scores(None, None, Some(6_000), Some(800))),
            Some("syn_ram6000_syn_disk800".to_string())
        );
    }

    #[test]
    fn test_payload_includes_synthetic_scene_tag() {
        let submission = submission_with_scores(Some(12_345), Some(6_789), Some(5_555), Some(777));
        let payload = to_tracker_submission_payload(&submission, true, true);
        assert_eq!(
            payload.scene_tag.as_deref(),
            Some("syn_cpu12345_syn_gpu6789_syn_ram5555_syn_disk777")
        );
        assert_eq!(payload.sessions[0].synthetic_cpu_score, Some(12_345));
        assert_eq!(payload.sessions[0].synthetic_gpu_score, Some(6_789));
        assert_eq!(payload.sessions[0].synthetic_ram_score, Some(5_555));
        assert_eq!(payload.sessions[0].synthetic_disk_score, Some(777));
    }

    #[test]
    fn test_payload_scene_tag_includes_synthetic_profile() {
        let mut submission = submission_with_scores(Some(5000), None, None, None);
        submission.synthetic_profile = Some("quick".to_string());
        let payload = to_tracker_submission_payload(&submission, true, true);
        assert_eq!(
            payload.scene_tag.as_deref(),
            Some("syn_profile_quick_syn_cpu5000")
        );
    }

    #[test]
    fn test_offline_queue_status_policy() {
        assert!(should_queue_offline(&ApiError::Api {
            status: 408,
            message: "timeout".to_string()
        }));
        assert!(should_queue_offline(&ApiError::Api {
            status: 429,
            message: "rate limited".to_string()
        }));
        assert!(should_queue_offline(&ApiError::Api {
            status: 500,
            message: "server".to_string()
        }));
        assert!(!should_queue_offline(&ApiError::Api {
            status: 400,
            message: "bad request".to_string()
        }));
        assert!(!should_queue_offline(&ApiError::Api {
            status: 200,
            message: "invalid success payload".to_string()
        }));
        assert!(should_queue_offline(&ApiError::InvalidResponse(
            "invalid successful payload".to_string()
        )));
        assert!(!should_queue_offline(&ApiError::ConsentRequired(
            "consent missing".to_string()
        )));
    }

    #[test]
    fn test_retryable_status_policy() {
        assert!(is_retryable_status(408));
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(422));
    }
}
