use crate::benchmark::BenchmarkSubmission;
use crate::config::Config;
use crate::deps;
use crate::feedback::{
    self, FeedbackCategory, FeedbackSchema, FeedbackSubmission, FeedbackSurface,
};
use crate::games::KNOWN_GAMES;
use crate::hardware::SystemInfo;
use axum::{
    extract::{Json, Query},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::UnboundedReceiverStream;

// --- Models ---

#[derive(Debug, Serialize)]
struct DepsStatusItem {
    name: String,
    required: bool,
    available: bool,
    details: String,
}

#[derive(Debug, Serialize)]
struct WindowsRuntimeStatus {
    winget_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    presentmon_path: Option<String>,
    presentmon_help_ok: bool,
    presentmon_help_summary: String,
}

#[derive(Debug, Serialize)]
struct DepsStatusResponse {
    platform: String,
    dependencies: Vec<DepsStatusItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    windows_runtime: Option<WindowsRuntimeStatus>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
struct InstallPresentmonRequest {
    confirm: bool,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Deserialize)]
struct InstallPresentmonRequest {}

#[derive(Debug, Serialize)]
struct InstallPresentmonResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    presentmon_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyntheticInstallCommandResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    tools_missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct HardwareResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    os_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os_version: Option<String>,
    gpu: ComponentSpec,
    cpu: ComponentSpec,
    ram: ComponentSpec,
    confidence: f64,
}

#[derive(Serialize)]
struct ComponentSpec {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    vram_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cores: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    threads: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_mhz: Option<u64>,
}

#[derive(Serialize)]
struct GameResponse {
    id: String,
    name: String,
    has_benchmark: bool,
    difficulty: String,
    supports_rt: bool,
    supports_dlss: bool,
    supports_fsr: bool,
    anti_cheat_risk: String,
    benchmark_notes: String,
}

#[derive(Deserialize)]
struct GameFilter {
    difficulty: Option<String>,
    has_benchmark: Option<bool>,
}

#[derive(Serialize)]
struct SubmissionResponse {
    status: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct SyntheticRunQuery {
    profile: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyntheticRunError {
    error: String,
    requires_admin: bool,
}

#[derive(Debug, Deserialize)]
struct ConsentAcceptRequest {
    tos_accepted: bool,
    consent_public_use: bool,
    retention_acknowledged: bool,
}

#[derive(Debug, Serialize)]
struct ConsentStatusResponse {
    tos_accepted: bool,
    consent_public_use: bool,
    retention_acknowledged: bool,
    accepted_at_utc: Option<chrono::DateTime<chrono::Utc>>,
    complete: bool,
}

#[derive(Debug, Deserialize)]
struct FeedbackSchemaQuery {
    surface: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeedbackSubmitRequest {
    category: FeedbackCategory,
    issue_code: String,
    message: String,
    #[serde(default)]
    include_diagnostics: bool,
}

#[derive(Debug, Serialize)]
struct FeedbackSubmitResponse {
    status: String,
    message: String,
}

// --- Handlers ---

async fn detect_hardware() -> impl IntoResponse {
    let info = match SystemInfo::detect() {
        Ok(info) => info,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Hardware detection failed: {err}"),
            )
                .into_response();
        }
    };

    let has_gpu =
        !info.gpu.name.trim().is_empty() && !info.gpu.name.eq_ignore_ascii_case("unknown gpu");
    let has_cpu = !info.cpu.name.trim().is_empty()
        && !info.cpu.name.eq_ignore_ascii_case("unknown cpu")
        && info.cpu.cores > 0
        && info.cpu.threads > 0;
    let has_ram = info.ram.installed_mb.unwrap_or(info.ram.usable_mb) > 0;
    let confidence = match (has_gpu, has_cpu, has_ram) {
        (true, true, true) => 0.95,
        (true, true, false) | (true, false, true) | (false, true, true) => 0.75,
        (true, false, false) | (false, true, false) | (false, false, true) => 0.55,
        (false, false, false) => 0.25,
    };

    let os_family = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    };

    (
        StatusCode::OK,
        Json(HardwareResponse {
            os_family: Some(os_family.to_string()),
            os: Some(info.os),
            os_version: info.os_version,
            gpu: ComponentSpec {
                name: info.gpu.name,
                vram_mb: info.gpu.vram_mb,
                cores: None,
                threads: None,
                total_mb: None,
                speed_mhz: None,
            },
            cpu: ComponentSpec {
                name: info.cpu.name,
                vram_mb: None,
                cores: Some(info.cpu.cores),
                threads: Some(info.cpu.threads),
                total_mb: None,
                speed_mhz: None,
            },
            ram: ComponentSpec {
                name: "System Memory".to_string(),
                vram_mb: None,
                cores: None,
                threads: None,
                // Use installed_mb if available, otherwise fallback to usable_mb (converted to Option)
                total_mb: info.ram.installed_mb.or(Some(info.ram.usable_mb)),
                speed_mhz: info.ram.speed_mhz,
            },
            confidence,
        }),
    )
        .into_response()
}

async fn list_games(Query(filter): Query<GameFilter>) -> Json<Vec<GameResponse>> {
    let games = KNOWN_GAMES
        .iter()
        .filter(|g| {
            if let Some(diff) = &filter.difficulty {
                if g.difficulty.to_string().to_lowercase() != diff.to_lowercase() {
                    return false;
                }
            }
            if let Some(has_bench) = filter.has_benchmark {
                if g.has_benchmark != has_bench {
                    return false;
                }
            }
            true
        })
        .map(|g| GameResponse {
            id: g.name.to_lowercase().replace(" ", "-"),
            name: g.name.to_string(),
            has_benchmark: g.has_benchmark,
            difficulty: g.difficulty.to_string().to_lowercase(),
            supports_rt: g.supports_rt,
            supports_dlss: g.supports_dlss,
            supports_fsr: g.supports_fsr,
            anti_cheat_risk: game_anti_cheat_risk(g.name).to_string(),
            benchmark_notes: g.benchmark_notes.to_string(),
        })
        .collect();

    Json(games)
}

fn game_anti_cheat_risk(game_name: &str) -> &'static str {
    match game_name {
        "Valorant" | "League of Legends" => "high",
        "Counter-Strike 2"
        | "Fortnite"
        | "Apex Legends"
        | "Call of Duty: Warzone"
        | "PUBG: BATTLEGROUNDS"
        | "Tom Clancy's Rainbow Six Siege"
        | "Destiny 2"
        | "The Finals"
        | "Delta Force"
        | "Marvel Rivals" => "medium",
        _ => "low",
    }
}

async fn consent_status() -> impl IntoResponse {
    let config = Config::load().unwrap_or_default();
    let consent = &config.consent;
    (
        StatusCode::OK,
        Json(ConsentStatusResponse {
            tos_accepted: consent.tos_accepted,
            consent_public_use: consent.consent_public_use,
            retention_acknowledged: consent.retention_acknowledged,
            accepted_at_utc: consent.accepted_at_utc,
            complete: consent.is_complete(),
        }),
    )
}

async fn accept_consent(Json(request): Json<ConsentAcceptRequest>) -> impl IntoResponse {
    if !request.tos_accepted || !request.consent_public_use || !request.retention_acknowledged {
        return (
            StatusCode::BAD_REQUEST,
            "All consent checkboxes must be accepted.",
        )
            .into_response();
    }

    let mut config = Config::load().unwrap_or_default();
    config.consent.tos_accepted = true;
    config.consent.consent_public_use = true;
    config.consent.retention_acknowledged = true;
    config.consent.accepted_at_utc = Some(Utc::now());

    if let Err(err) = config.save() {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }

    (
        StatusCode::OK,
        Json(ConsentStatusResponse {
            tos_accepted: config.consent.tos_accepted,
            consent_public_use: config.consent.consent_public_use,
            retention_acknowledged: config.consent.retention_acknowledged,
            accepted_at_utc: config.consent.accepted_at_utc,
            complete: config.consent.is_complete(),
        }),
    )
        .into_response()
}

// Reuse existing BenchmarkSubmission but wrapped for API
async fn submit_benchmark(
    headers: HeaderMap,
    Json(mut submission): Json<BenchmarkSubmission>,
) -> impl IntoResponse {
    // Canonicalize server-side identifiers/timestamp so client clocks/placeholders
    // never become authoritative in downstream storage.
    submission.id = uuid::Uuid::new_v4();
    submission.timestamp = Utc::now();

    let idempotency_key = headers
        .get("Idempotency-Key")
        .or_else(|| headers.get("X-Idempotency-Key"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(crate::idempotency::new_submit_key);
    match crate::api::submit_benchmark_with_idempotency_key(&submission, &idempotency_key).await {
        Ok(api_response) => {
            let status = api_response
                .status
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "accepted".to_string());

            let message = if api_response.message.trim().is_empty() {
                "Thank you for contributing!".to_string()
            } else {
                api_response.message.clone()
            };

            let response = SubmissionResponse { status, message };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            if crate::api::should_queue_offline(&err) {
                match crate::storage::init_storage().and_then(|storage| {
                    storage
                        .save_pending_benchmark_with_idempotency_key(&submission, &idempotency_key)
                }) {
                    Ok(_pending_id) => {
                        let response = SubmissionResponse {
                            status: "queued".to_string(),
                            message: format!(
                                "Submission queued locally ({}). We'll retry when you're back online.",
                                err
                            ),
                        };

                        (StatusCode::ACCEPTED, Json(response)).into_response()
                    }
                    Err(storage_err) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, storage_err.to_string()).into_response()
                    }
                }
            } else {
                match err {
                    crate::api::ApiError::Validation(message) => {
                        (StatusCode::BAD_REQUEST, message).into_response()
                    }
                    crate::api::ApiError::ConsentRequired(message) => {
                        (StatusCode::FORBIDDEN, message).into_response()
                    }
                    crate::api::ApiError::Api { status, message } => {
                        let status_code =
                            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
                        (status_code, message).into_response()
                    }
                    crate::api::ApiError::Network(message) => {
                        (StatusCode::BAD_GATEWAY, message.to_string()).into_response()
                    }
                    crate::api::ApiError::InvalidResponse(message) => {
                        (StatusCode::BAD_GATEWAY, message).into_response()
                    }
                    crate::api::ApiError::Unreachable => {
                        (StatusCode::BAD_GATEWAY, err.to_string()).into_response()
                    }
                }
            }
        }
    }
}

async fn run_synthetic_benchmarks(Query(query): Query<SyntheticRunQuery>) -> impl IntoResponse {
    let profile = query
        .profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    let bench_type = match profile.as_deref() {
        Some("quick") => crate::benchmark_runner::BenchmarkType::Quick,
        Some("extended") => crate::benchmark_runner::BenchmarkType::Extended,
        Some("standard") | None => crate::benchmark_runner::BenchmarkType::Standard,
        Some(_) => crate::benchmark_runner::BenchmarkType::Standard,
    };

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = bench_type;
        let err = SyntheticRunError {
            error:
                "Synthetic benchmarks are currently supported on Windows, Linux, and macOS only."
                    .to_string(),
            requires_admin: false,
        };
        (StatusCode::NOT_IMPLEMENTED, Json(err)).into_response()
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let options = crate::benchmark_runner::BenchmarkRunOptions {
            quiet: true,
            progress: None,
        };
        let result = tokio::task::spawn_blocking(move || {
            crate::benchmark_runner::run_benchmarks_with_options(bench_type, options)
        })
        .await;

        match result {
            Ok(Ok(results)) => (StatusCode::OK, Json(results)).into_response(),
            Ok(Err(err)) => {
                let message = err.to_string();
                let lower = message.to_ascii_lowercase();
                let requires_admin = lower.contains("administrator") || lower.contains("elevat");
                let status = if requires_admin {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::BAD_REQUEST
                };
                (
                    status,
                    Json(SyntheticRunError {
                        error: message,
                        requires_admin,
                    }),
                )
                    .into_response()
            }
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Synthetic benchmark failed: {err}"),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SyntheticStreamStart {
    profile: String,
}

#[derive(Debug, Serialize)]
struct SyntheticStreamError {
    error: String,
    requires_admin: bool,
}

async fn stream_synthetic_benchmarks(Query(query): Query<SyntheticRunQuery>) -> impl IntoResponse {
    let profile = query
        .profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    let bench_type = match profile.as_deref() {
        Some("quick") => crate::benchmark_runner::BenchmarkType::Quick,
        Some("extended") => crate::benchmark_runner::BenchmarkType::Extended,
        Some("standard") | None => crate::benchmark_runner::BenchmarkType::Standard,
        Some(_) => crate::benchmark_runner::BenchmarkType::Standard,
    };

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = bench_type;
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SyntheticRunError {
                error:
                    "Synthetic benchmarks are currently supported on Windows, Linux, and macOS only."
                        .to_string(),
                requires_admin: false,
            }),
        )
            .into_response();
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();

        let profile_key = bench_type.profile_key().to_string();
        let _ = tx.send(Ok(Event::default().event("start").data(
            serde_json::to_string(&SyntheticStreamStart {
                profile: profile_key.clone(),
            })
            .unwrap_or_else(|_| format!("{{\"profile\":\"{}\"}}", profile_key)),
        )));

        let progress_tx = tx.clone();
        let options = crate::benchmark_runner::BenchmarkRunOptions {
            quiet: true,
            progress: Some(Arc::new(move |update| {
                if let Ok(payload) = serde_json::to_string(&update) {
                    let _ = progress_tx.send(Ok(Event::default().event("progress").data(payload)));
                }
            })),
        };

        tokio::task::spawn_blocking(move || {
            let out = crate::benchmark_runner::run_benchmarks_with_options(bench_type, options);
            match out {
                Ok(results) => {
                    if let Ok(payload) = serde_json::to_string(&results) {
                        let _ = tx.send(Ok(Event::default().event("result").data(payload)));
                    } else {
                        let _ = tx.send(Ok(Event::default().event("bench_error").data(
                            "{\"error\":\"Failed to serialize benchmark results\",\"requires_admin\":false}",
                        )));
                    }
                }
                Err(err) => {
                    let message = err.to_string();
                    let lower = message.to_ascii_lowercase();
                    let requires_admin =
                        lower.contains("administrator") || lower.contains("elevat");
                    let payload = serde_json::to_string(&SyntheticStreamError {
                        error: message,
                        requires_admin,
                    })
                    .unwrap_or_else(|_| {
                        "{\"error\":\"Synthetic benchmark failed\",\"requires_admin\":false}"
                            .to_string()
                    });
                    let _ = tx.send(Ok(Event::default().event("bench_error").data(payload)));
                }
            }
        });

        Sse::new(UnboundedReceiverStream::new(rx))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
            .into_response()
    }
}

async fn feedback_schema(Query(query): Query<FeedbackSchemaQuery>) -> impl IntoResponse {
    let surface = match query
        .surface
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some("terminal_ui") | Some("terminal") => FeedbackSurface::TerminalUi,
        Some("web_ui") | Some("web") | None => FeedbackSurface::WebUi,
        Some(_) => FeedbackSurface::WebUi,
    };

    let schema: FeedbackSchema = feedback::schema_for(surface);
    (StatusCode::OK, Json(schema)).into_response()
}

async fn submit_feedback(
    headers: HeaderMap,
    Json(request): Json<FeedbackSubmitRequest>,
) -> impl IntoResponse {
    let surface = FeedbackSurface::WebUi;
    let diagnostics = if request.include_diagnostics {
        Some(feedback::collect_diagnostics(surface))
    } else {
        None
    };

    let feedback = FeedbackSubmission {
        surface,
        category: request.category,
        issue_code: request.issue_code,
        message: request.message,
        diagnostics,
    };

    let idempotency_key = headers
        .get("Idempotency-Key")
        .or_else(|| headers.get("X-Idempotency-Key"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(crate::idempotency::new_feedback_key);

    match crate::api::submit_feedback_with_idempotency_key(&feedback, &idempotency_key).await {
        Ok(api_response) => {
            let status = api_response
                .status
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "accepted".to_string());

            let message = api_response
                .message
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Thank you for the feedback.".to_string());

            (
                StatusCode::OK,
                Json(FeedbackSubmitResponse { status, message }),
            )
                .into_response()
        }
        Err(err) => {
            if crate::api::should_queue_offline_feedback(&err) {
                match crate::storage::init_storage().and_then(|storage| {
                    storage.save_pending_feedback_with_idempotency_key(&feedback, &idempotency_key)
                }) {
                    Ok(_pending_id) => {
                        let response = FeedbackSubmitResponse {
                            status: "queued".to_string(),
                            message: format!(
                                "Feedback queued locally ({}). We'll retry when you're back online.",
                                err
                            ),
                        };
                        (StatusCode::ACCEPTED, Json(response)).into_response()
                    }
                    Err(storage_err) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, storage_err.to_string()).into_response()
                    }
                }
            } else {
                match err {
                    crate::api::ApiError::Validation(message) => {
                        (StatusCode::BAD_REQUEST, message).into_response()
                    }
                    crate::api::ApiError::Api { status, message } => {
                        let status_code =
                            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
                        (status_code, message).into_response()
                    }
                    crate::api::ApiError::Network(message) => {
                        (StatusCode::BAD_GATEWAY, message.to_string()).into_response()
                    }
                    crate::api::ApiError::InvalidResponse(message) => {
                        (StatusCode::BAD_GATEWAY, message).into_response()
                    }
                    crate::api::ApiError::ConsentRequired(message) => {
                        (StatusCode::FORBIDDEN, message).into_response()
                    }
                    crate::api::ApiError::Unreachable => {
                        (StatusCode::BAD_GATEWAY, err.to_string()).into_response()
                    }
                }
            }
        }
    }
}

async fn deps_status() -> impl IntoResponse {
    let platform = std::env::consts::OS.to_string();
    let dependencies = deps::collect_dependency_statuses()
        .into_iter()
        .map(|dep| DepsStatusItem {
            name: dep.name.to_string(),
            required: dep.required,
            available: dep.available,
            details: dep.details,
        })
        .collect::<Vec<_>>();

    #[cfg(target_os = "windows")]
    let windows_runtime = Some({
        let probe = deps::probe_windows_runtime();
        WindowsRuntimeStatus {
            winget_available: probe.winget_available,
            presentmon_path: probe.presentmon_path.map(|p| p.display().to_string()),
            presentmon_help_ok: probe.presentmon_help_ok,
            presentmon_help_summary: probe.presentmon_help_summary,
        }
    });

    #[cfg(not(target_os = "windows"))]
    let windows_runtime = None;

    Json(DepsStatusResponse {
        platform,
        dependencies,
        windows_runtime,
    })
}

async fn install_presentmon(Json(request): Json<InstallPresentmonRequest>) -> impl IntoResponse {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = request;
        (
            StatusCode::BAD_REQUEST,
            Json(InstallPresentmonResponse {
                ok: false,
                presentmon_path: None,
                message: Some("PresentMon install is only supported on Windows.".to_string()),
            }),
        )
    }

    #[cfg(target_os = "windows")]
    {
        if !request.confirm {
            return (
                StatusCode::BAD_REQUEST,
                Json(InstallPresentmonResponse {
                    ok: false,
                    presentmon_path: None,
                    message: Some("Missing confirmation. Pass {\"confirm\": true}.".to_string()),
                }),
            );
        }

        let install = tokio::task::spawn_blocking(|| deps::ensure_presentmon_for_session(true));
        let outcome = install
            .await
            .map_err(|err| anyhow::anyhow!("Install task failed: {err}"))
            .and_then(|res| res);

        match outcome {
            Ok(Some(path)) => (
                StatusCode::OK,
                Json(InstallPresentmonResponse {
                    ok: true,
                    presentmon_path: Some(path.display().to_string()),
                    message: None,
                }),
            ),
            Ok(None) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(InstallPresentmonResponse {
                    ok: false,
                    presentmon_path: None,
                    message: Some("PresentMon was not installed.".to_string()),
                }),
            ),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(InstallPresentmonResponse {
                    ok: false,
                    presentmon_path: None,
                    message: Some(err.to_string()),
                }),
            ),
        }
    }
}

async fn synthetic_install_command() -> impl IntoResponse {
    #[cfg(target_os = "windows")]
    {
        (
            StatusCode::BAD_REQUEST,
            Json(SyntheticInstallCommandResponse {
                ok: false,
                command: None,
                tools_missing: Vec::new(),
                message: Some(
                    "Synthetic tool install command is only supported on Linux/macOS.".to_string(),
                ),
            }),
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        let statuses = deps::collect_dependency_statuses();
        let tools_missing = statuses
            .iter()
            .filter(|item| !item.available)
            .filter_map(|item| match item.name {
                "glmark2" | "sysbench" | "fio" | "stress-ng" => Some(item.name.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let command = deps::dependency_bulk_install_command(&statuses);
        (
            StatusCode::OK,
            Json(SyntheticInstallCommandResponse {
                ok: true,
                command,
                tools_missing,
                message: None,
            }),
        )
    }
}

pub fn api_routes() -> Router {
    Router::new()
        .route("/api/consent/status", get(consent_status))
        .route("/api/consent/accept", post(accept_consent))
        .route("/api/hardware/detect", post(detect_hardware))
        .route("/api/deps/status", get(deps_status))
        .route(
            "/api/deps/synthetic/install-command",
            get(synthetic_install_command),
        )
        .route("/api/deps/presentmon/install", post(install_presentmon))
        .route("/api/games/list", get(list_games))
        .route("/api/benchmark/submit", post(submit_benchmark))
        .route(
            "/api/benchmark/synthetic/run",
            post(run_synthetic_benchmarks),
        )
        .route(
            "/api/benchmark/synthetic/stream",
            get(stream_synthetic_benchmarks),
        )
        .route("/api/feedback/schema", get(feedback_schema))
        .route("/api/feedback/submit", post(submit_feedback))
}

#[cfg(test)]
mod tests {
    use super::api_routes;
    use crate::benchmark::BenchmarkSubmission;
    use crate::config::Config;
    use crate::hardware::cpu::CpuInfo;
    use crate::hardware::gpu::{GpuInfo, GpuVendor};
    use crate::hardware::ram::RamInfo;
    use crate::hardware::SystemInfo;
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::Value;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::OnceLock;
    use tempfile::TempDir;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<str>) -> Self {
            let prev = std::env::var(key).ok();
            std::env::set_var(key, value.as_ref());
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.prev {
                std::env::set_var(self.key, prev);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn mock_system_info() -> SystemInfo {
        SystemInfo {
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
            os: "Linux".to_string(),
            os_version: Some("6.8".to_string()),
        }
    }

    fn make_submission() -> BenchmarkSubmission {
        BenchmarkSubmission::new(
            mock_system_info(),
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "Ultra".to_string(),
            95.0,
            Some(72.0),
            false,
            Some("DLSS Quality".to_string()),
        )
    }

    async fn spawn_server(app: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn submit_benchmark_proxies_to_remote_backend() {
        let _guard = env_lock().lock().await;
        let temp_dir = TempDir::new().unwrap();
        let _data_dir = EnvVarGuard::set("XDG_DATA_HOME", temp_dir.path().to_string_lossy());
        let _config_path = EnvVarGuard::set(
            "FPS_TRACKER_CONFIG_PATH",
            temp_dir.path().join("config.toml").to_string_lossy(),
        );
        let _retries = EnvVarGuard::set("FPS_TRACKER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("FPS_TRACKER_API_TIMEOUT_SECONDS", "2");

        let mut config = Config::default();
        config.consent.tos_accepted = true;
        config.consent.consent_public_use = true;
        config.consent.retention_acknowledged = true;
        config.save().unwrap();

        let backend_app = Router::new().route(
            "/api/v2/tracker/submit",
            post(|| async {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "id": "server-123",
                        "message": "ok",
                        "status": "accepted",
                    })),
                )
            }),
        );
        let (backend_addr, backend_handle) = spawn_server(backend_app).await;
        let _api_url = EnvVarGuard::set("FPS_TRACKER_API_URL", format!("http://{backend_addr}"));

        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;
        let submission = make_submission();
        let response = reqwest::Client::new()
            .post(format!("http://{app_addr}/api/benchmark/submit"))
            .json(&submission)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: Value = response.json().await.unwrap();

        assert_eq!(json["status"], "accepted");
        assert_eq!(json["message"], "ok");
        assert!(json.get("submission_id").is_none());

        backend_handle.abort();
        app_handle.abort();
    }

    #[tokio::test]
    async fn submit_benchmark_saves_offline_on_5xx() {
        let _guard = env_lock().lock().await;
        let temp_dir = TempDir::new().unwrap();
        let _data_dir = EnvVarGuard::set("XDG_DATA_HOME", temp_dir.path().to_string_lossy());
        let _config_path = EnvVarGuard::set(
            "FPS_TRACKER_CONFIG_PATH",
            temp_dir.path().join("config.toml").to_string_lossy(),
        );
        let _retries = EnvVarGuard::set("FPS_TRACKER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("FPS_TRACKER_API_TIMEOUT_SECONDS", "2");

        let mut config = Config::default();
        config.consent.tos_accepted = true;
        config.consent.consent_public_use = true;
        config.consent.retention_acknowledged = true;
        config.save().unwrap();

        let backend_app = Router::new().route(
            "/api/v2/tracker/submit",
            post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        );
        let (backend_addr, backend_handle) = spawn_server(backend_app).await;
        let _api_url = EnvVarGuard::set("FPS_TRACKER_API_URL", format!("http://{backend_addr}"));

        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;
        let submission = make_submission();
        let response = reqwest::Client::new()
            .post(format!("http://{app_addr}/api/benchmark/submit"))
            .json(&submission)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let json: Value = response.json().await.unwrap();
        assert_eq!(json["status"], "queued");
        assert!(!json["message"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty());
        assert!(json.get("submission_id").is_none());

        let storage = crate::storage::init_storage().unwrap();
        let pending_dir = storage.data_dir().join("pending");
        let pending_files = std::fs::read_dir(&pending_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
            .count();
        assert!(
            pending_files >= 1,
            "Expected at least one pending submission file"
        );

        backend_handle.abort();
        app_handle.abort();
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    #[tokio::test]
    async fn synthetic_run_is_not_implemented_on_non_windows() {
        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;

        let response = reqwest::Client::new()
            .post(format!(
                "http://{app_addr}/api/benchmark/synthetic/run?profile=standard"
            ))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let json: Value = response.json().await.unwrap();
        assert!(json["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Windows"));

        app_handle.abort();
    }

    #[tokio::test]
    async fn submit_benchmark_forwards_fps_and_synthetic_metadata() {
        let _guard = env_lock().lock().await;
        let temp_dir = TempDir::new().unwrap();
        let _data_dir = EnvVarGuard::set("XDG_DATA_HOME", temp_dir.path().to_string_lossy());
        let _config_path = EnvVarGuard::set(
            "FPS_TRACKER_CONFIG_PATH",
            temp_dir.path().join("config.toml").to_string_lossy(),
        );
        let _retries = EnvVarGuard::set("FPS_TRACKER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("FPS_TRACKER_API_TIMEOUT_SECONDS", "2");

        let mut config = Config::default();
        config.consent.tos_accepted = true;
        config.consent.consent_public_use = true;
        config.consent.retention_acknowledged = true;
        config.save().unwrap();

        let captured_payload: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured_payload_for_route = Arc::clone(&captured_payload);
        let backend_app = Router::new().route(
            "/api/v2/tracker/submit",
            post(move |Json(payload): Json<Value>| {
                let captured_payload = Arc::clone(&captured_payload_for_route);
                async move {
                    *captured_payload.lock().await = Some(payload);
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "id": "server-xyz",
                            "message": "ok",
                            "status": "accepted",
                        })),
                    )
                }
            }),
        );
        let (backend_addr, backend_handle) = spawn_server(backend_app).await;
        let _api_url = EnvVarGuard::set("FPS_TRACKER_API_URL", format!("http://{backend_addr}"));

        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;

        let mut submission = make_submission();
        submission.fps_01_low = Some(61.0);
        submission.synthetic_cpu_score = Some(1234);
        submission.synthetic_gpu_score = Some(5678);
        submission.synthetic_ram_score = Some(4321);
        submission.synthetic_disk_score = Some(876);
        submission.synthetic_profile = Some("standard".to_string());
        submission.synthetic_suite_version = Some("1".to_string());
        submission.synthetic_extended = Some(serde_json::json!({
            "winsat_note": "WinSAT skipped: not elevated",
            "diskspd_read_mb_s": 1234,
        }));

        let response = reqwest::Client::new()
            .post(format!("http://{app_addr}/api/benchmark/submit"))
            .json(&submission)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let payload = captured_payload
            .lock()
            .await
            .clone()
            .expect("backend should receive a payload");

        assert_eq!(payload["sessions"][0]["fps_avg"], serde_json::json!(95.0));
        assert_eq!(payload["sessions"][0]["fps_1_low"], serde_json::json!(72.0));
        assert_eq!(
            payload["sessions"][0]["fps_0_1_low"],
            serde_json::json!(61.0)
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_cpu_score"],
            serde_json::json!(1234)
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_gpu_score"],
            serde_json::json!(5678)
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_ram_score"],
            serde_json::json!(4321)
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_disk_score"],
            serde_json::json!(876)
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_profile"],
            serde_json::json!("standard")
        );
        assert_eq!(
            payload["sessions"][0]["synthetic_suite_version"],
            serde_json::json!("1")
        );
        assert!(payload["sessions"][0]["synthetic_extended"].is_object());

        let scene_tag = payload["scene_tag"]
            .as_str()
            .expect("scene_tag must be present");
        assert!(scene_tag.contains("syn_cpu1234"));
        assert!(scene_tag.contains("syn_gpu5678"));
        assert!(scene_tag.contains("syn_ram4321"));
        assert!(scene_tag.contains("syn_disk876"));

        backend_handle.abort();
        app_handle.abort();
    }

    #[tokio::test]
    async fn feedback_schema_returns_categories() {
        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;

        let response = reqwest::Client::new()
            .get(format!(
                "http://{app_addr}/api/feedback/schema?surface=web_ui"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: Value = response.json().await.unwrap();
        assert_eq!(json["schema_version"], serde_json::json!(1));
        assert!(json["categories"].as_array().unwrap().len() >= 3);

        app_handle.abort();
    }

    #[tokio::test]
    async fn submit_feedback_proxies_to_remote_backend() {
        let _guard = env_lock().lock().await;
        let temp_dir = TempDir::new().unwrap();
        let _data_dir = EnvVarGuard::set("XDG_DATA_HOME", temp_dir.path().to_string_lossy());
        let _config_path = EnvVarGuard::set(
            "FPS_TRACKER_CONFIG_PATH",
            temp_dir.path().join("config.toml").to_string_lossy(),
        );
        let _retries = EnvVarGuard::set("FPS_TRACKER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("FPS_TRACKER_API_TIMEOUT_SECONDS", "2");

        let captured_payload: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured_payload_for_route = Arc::clone(&captured_payload);
        let backend_app = Router::new().route(
            "/api/v2/tracker/feedback",
            post(move |Json(payload): Json<Value>| {
                let captured_payload = Arc::clone(&captured_payload_for_route);
                async move {
                    *captured_payload.lock().await = Some(payload);
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "id": "fb-123",
                            "message": "ok",
                            "status": "accepted",
                        })),
                    )
                }
            }),
        );
        let (backend_addr, backend_handle) = spawn_server(backend_app).await;
        let _api_url = EnvVarGuard::set("FPS_TRACKER_API_URL", format!("http://{backend_addr}"));

        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;

        let response = reqwest::Client::new()
            .post(format!("http://{app_addr}/api/feedback/submit"))
            .json(&serde_json::json!({
                "category": "capture",
                "issue_code": "capture_wont_start",
                "message": "Capture did not start for my game.",
                "include_diagnostics": false
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let payload = captured_payload
            .lock()
            .await
            .clone()
            .expect("backend should receive a payload");
        assert_eq!(payload["surface"], serde_json::json!("web_ui"));
        assert_eq!(payload["category"], serde_json::json!("capture"));
        assert_eq!(
            payload["issue_code"],
            serde_json::json!("capture_wont_start")
        );
        assert_eq!(
            payload["message"],
            serde_json::json!("Capture did not start for my game.")
        );

        backend_handle.abort();
        app_handle.abort();
    }

    #[tokio::test]
    async fn submit_feedback_saves_offline_on_5xx() {
        let _guard = env_lock().lock().await;
        let temp_dir = TempDir::new().unwrap();
        let _data_dir = EnvVarGuard::set("XDG_DATA_HOME", temp_dir.path().to_string_lossy());
        let _config_path = EnvVarGuard::set(
            "FPS_TRACKER_CONFIG_PATH",
            temp_dir.path().join("config.toml").to_string_lossy(),
        );
        let _retries = EnvVarGuard::set("FPS_TRACKER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("FPS_TRACKER_API_TIMEOUT_SECONDS", "2");

        let backend_app = Router::new().route(
            "/api/v2/tracker/feedback",
            post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        );
        let (backend_addr, backend_handle) = spawn_server(backend_app).await;
        let _api_url = EnvVarGuard::set("FPS_TRACKER_API_URL", format!("http://{backend_addr}"));

        let app = api_routes();
        let (app_addr, app_handle) = spawn_server(app).await;

        let response = reqwest::Client::new()
            .post(format!("http://{app_addr}/api/feedback/submit"))
            .json(&serde_json::json!({
                "category": "web_ui",
                "issue_code": "page_wont_load",
                "message": "Blank screen.",
                "include_diagnostics": false
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let json: Value = response.json().await.unwrap();
        assert!(json.get("feedback_id").is_none());

        let storage = crate::storage::init_storage().unwrap();
        let pending_dir = storage.data_dir().join("pending_feedback");
        let pending_files = std::fs::read_dir(&pending_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
            .count();
        assert!(
            pending_files >= 1,
            "Expected at least one pending feedback file"
        );

        backend_handle.abort();
        app_handle.abort();
    }
}
