use crate::benchmark::BenchmarkSubmission;
use crate::config::Config;
use crate::games::KNOWN_GAMES;
use crate::hardware::SystemInfo;
use axum::{
    extract::{Json, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// --- Models ---

#[derive(Serialize)]
struct HardwareResponse {
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
    submission_id: String,
    status: String,
    message: String,
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

// --- Handlers ---

async fn detect_hardware() -> Json<HardwareResponse> {
    // Handle error by returning default empty/unknown if detection fails
    let info = SystemInfo::detect().unwrap_or(SystemInfo {
        gpu: crate::hardware::gpu::GpuInfo {
            name: "Unknown GPU".to_string(),
            vendor: crate::hardware::gpu::GpuVendor::Unknown,
            pci_id: None,
            vram_mb: None,
            gpu_clock_mhz: None,
            memory_clock_mhz: None,
            temperature_c: None,
            utilization_percent: None,
            driver_version: None,
        },
        cpu: crate::hardware::cpu::CpuInfo {
            name: "Unknown CPU".to_string(),
            cores: 0,
            threads: 0,
            frequency_mhz: None,
            max_frequency_mhz: None,
            architecture: None,
            vendor: "Unknown".to_string(),
        },
        ram: crate::hardware::ram::RamInfo {
            installed_mb: None,
            usable_mb: 0,
            speed_mhz: None,
            ram_type: None,
            stick_count: None,
            model: None,
        },
        os: "Unknown".to_string(),
        os_version: None,
    });

    Json(HardwareResponse {
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
        confidence: 0.95,
    })
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
        "Counter-Strike 2" | "Fortnite" | "Apex Legends" | "Call of Duty: Warzone" => "medium",
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
async fn submit_benchmark(Json(submission): Json<BenchmarkSubmission>) -> impl IntoResponse {
    match crate::api::submit_benchmark(&submission).await {
        Ok(api_response) => {
            let submission_id = api_response
                .effective_id()
                .map(|id| id.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

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

            let response = SubmissionResponse {
                submission_id,
                status,
                message,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            let should_save_offline = matches!(
                &err,
                crate::api::ApiError::Network(_)
                    | crate::api::ApiError::Unreachable
                    | crate::api::ApiError::Api {
                        status: 500..=u16::MAX,
                        ..
                    }
            );

            if should_save_offline {
                match crate::storage::init_storage()
                    .and_then(|storage| storage.save_pending_benchmark(&submission))
                {
                    Ok(pending_id) => {
                        let response = SubmissionResponse {
                            submission_id: pending_id,
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
                    crate::api::ApiError::Unreachable => {
                        (StatusCode::BAD_GATEWAY, err.to_string()).into_response()
                    }
                }
            }
        }
    }
}

pub fn api_routes() -> Router {
    Router::new()
        .route("/api/consent/status", get(consent_status))
        .route("/api/consent/accept", post(accept_consent))
        .route("/api/hardware/detect", post(detect_hardware))
        .route("/api/games/list", get(list_games))
        .route("/api/benchmark/submit", post(submit_benchmark))
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
        let _retries = EnvVarGuard::set("PCBUILDER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("PCBUILDER_API_TIMEOUT_SECONDS", "2");

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
        let _api_url = EnvVarGuard::set("PCBUILDER_API_URL", format!("http://{backend_addr}"));

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

        assert_eq!(json["submission_id"], "server-123");
        assert_eq!(json["status"], "accepted");
        assert_eq!(json["message"], "ok");

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
        let _retries = EnvVarGuard::set("PCBUILDER_API_MAX_RETRIES", "0");
        let _timeout = EnvVarGuard::set("PCBUILDER_API_TIMEOUT_SECONDS", "2");

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
        let _api_url = EnvVarGuard::set("PCBUILDER_API_URL", format!("http://{backend_addr}"));

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
        let pending_id = json["submission_id"].as_str().unwrap();

        assert!(pending_id.starts_with("pending_"));
        assert_eq!(json["status"], "queued");
        assert!(!json["message"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty());

        let storage = crate::storage::init_storage().unwrap();
        let path = storage
            .data_dir()
            .join("pending")
            .join(format!("{pending_id}.json"));
        assert!(path.exists());

        backend_handle.abort();
        app_handle.abort();
    }
}
