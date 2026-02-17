//! Feedback capture and submission.
//!
//! Goals:
//! - Let users report issues from the app (Web UI and terminal) without needing a GitHub account.
//! - Keep wording user-facing (no implementation details like tool names).
//! - Allow an opt-in diagnostics snapshot that helps debugging without including sensitive paths.

use serde::{Deserialize, Serialize};

use crate::deps;

pub(crate) mod cli;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FeedbackSurface {
    WebUi,
    TerminalUi,
}

impl FeedbackSurface {
    // Intentionally no `label()` helper here: surfaces are represented in UI text via the schema,
    // and the backend uses stable snake_case identifiers.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FeedbackCategory {
    InstallUpdate,
    WebUi,
    TerminalUi,
    HardwareDetection,
    Capture,
    SyntheticBenchmarks,
    SubmissionSync,
    DataQuality,
    Performance,
    PrivacyConsent,
    Other,
}

impl FeedbackCategory {
    pub(crate) fn label(self) -> &'static str {
        match self {
            FeedbackCategory::InstallUpdate => "Install / Update",
            FeedbackCategory::WebUi => "Web UI",
            FeedbackCategory::TerminalUi => "Terminal UI",
            FeedbackCategory::HardwareDetection => "Hardware Detection",
            FeedbackCategory::Capture => "Performance Capture",
            FeedbackCategory::SyntheticBenchmarks => "Synthetic Benchmarks",
            FeedbackCategory::SubmissionSync => "Submission & Sync",
            FeedbackCategory::DataQuality => "Data Quality",
            FeedbackCategory::Performance => "Performance / Stability",
            FeedbackCategory::PrivacyConsent => "Privacy / Consent",
            FeedbackCategory::Other => "Other",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            FeedbackCategory::InstallUpdate => {
                "Install steps, upgrades, dependencies, or setup problems."
            }
            FeedbackCategory::WebUi => "Browser-based interface issues or confusing flows.",
            FeedbackCategory::TerminalUi => {
                "Terminal prompts, formatting, or interaction problems."
            }
            FeedbackCategory::HardwareDetection => {
                "Incorrect or missing GPU/CPU/RAM/driver details."
            }
            FeedbackCategory::Capture => "Issues capturing FPS / lows from your tool or session.",
            FeedbackCategory::SyntheticBenchmarks => {
                "Optional synthetic benchmark run issues or odd results."
            }
            FeedbackCategory::SubmissionSync => {
                "Upload failures, queued retries, or sync problems."
            }
            FeedbackCategory::DataQuality => {
                "Numbers look wrong, rounding, mismatched lows, or confusion about metrics."
            }
            FeedbackCategory::Performance => "Crashes, freezes, slow UI, or high CPU usage.",
            FeedbackCategory::PrivacyConsent => {
                "Questions about what is collected, consent, or retention messaging."
            }
            FeedbackCategory::Other => "Anything else.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OsFamily {
    Windows,
    Macos,
    Linux,
    Other,
}

pub(crate) fn current_os_family() -> OsFamily {
    match std::env::consts::OS {
        "windows" => OsFamily::Windows,
        "macos" => OsFamily::Macos,
        "linux" => OsFamily::Linux,
        _ => OsFamily::Other,
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeedbackIssueOption {
    pub(crate) code: &'static str,
    pub(crate) label: &'static str,
    pub(crate) hint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeedbackCategorySchema {
    pub(crate) id: FeedbackCategory,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) issues: Vec<FeedbackIssueOption>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeedbackSchema {
    pub(crate) schema_version: u32,
    pub(crate) surface: FeedbackSurface,
    pub(crate) os: OsFamily,
    pub(crate) intro: &'static str,
    pub(crate) privacy_note: &'static str,
    pub(crate) categories: Vec<FeedbackCategorySchema>,
}

pub(crate) fn schema_for(surface: FeedbackSurface) -> FeedbackSchema {
    let os = current_os_family();

    let categories = vec![
        FeedbackCategorySchema {
            id: FeedbackCategory::InstallUpdate,
            label: FeedbackCategory::InstallUpdate.label(),
            description: FeedbackCategory::InstallUpdate.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "install_failed",
                    label: "Install failed",
                    hint: "The app or a required helper tool would not install correctly.",
                },
                FeedbackIssueOption {
                    code: "update_failed",
                    label: "Update failed",
                    hint: "Updating the app or tools failed or left things in a broken state.",
                },
                FeedbackIssueOption {
                    code: "dependency_prompt_confusing",
                    label: "Dependency prompt was confusing",
                    hint: "I wasn’t sure what I was approving or why it was needed.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::WebUi,
            label: FeedbackCategory::WebUi.label(),
            description: FeedbackCategory::WebUi.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "page_wont_load",
                    label: "Page would not load",
                    hint: "I saw a blank screen, error screen, or infinite loading state.",
                },
                FeedbackIssueOption {
                    code: "buttons_not_working",
                    label: "Buttons or inputs did not work",
                    hint: "Clicks didn’t register, forms wouldn’t submit, or controls felt broken.",
                },
                FeedbackIssueOption {
                    code: "confusing_flow",
                    label: "Flow was confusing",
                    hint: "I wasn’t sure what to do next or what a field meant.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::TerminalUi,
            label: FeedbackCategory::TerminalUi.label(),
            description: FeedbackCategory::TerminalUi.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "prompt_confusing",
                    label: "Prompts were confusing",
                    hint: "I wasn’t sure what to enter or how to proceed.",
                },
                FeedbackIssueOption {
                    code: "formatting_broken",
                    label: "Text formatting looked broken",
                    hint: "Output was hard to read, aligned wrong, or messy in my terminal.",
                },
                FeedbackIssueOption {
                    code: "input_painful",
                    label: "Entering data was painful",
                    hint: "Too many steps or too much typing for what I wanted to report.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::HardwareDetection,
            label: FeedbackCategory::HardwareDetection.label(),
            description: FeedbackCategory::HardwareDetection.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "gpu_wrong",
                    label: "GPU info looked wrong",
                    hint: "GPU name, VRAM, or driver info didn’t match my system.",
                },
                FeedbackIssueOption {
                    code: "cpu_wrong",
                    label: "CPU info looked wrong",
                    hint: "CPU name, core/thread count, or frequency looked incorrect.",
                },
                FeedbackIssueOption {
                    code: "ram_wrong",
                    label: "RAM info looked wrong",
                    hint: "Installed/usable RAM, speed, or type didn’t match my system.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::Capture,
            label: FeedbackCategory::Capture.label(),
            description: FeedbackCategory::Capture.description(),
            issues: capture_issue_options_for(os),
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::SyntheticBenchmarks,
            label: FeedbackCategory::SyntheticBenchmarks.label(),
            description: FeedbackCategory::SyntheticBenchmarks.description(),
            issues: synthetic_issue_options_for(os),
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::SubmissionSync,
            label: FeedbackCategory::SubmissionSync.label(),
            description: FeedbackCategory::SubmissionSync.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "upload_failed",
                    label: "Upload failed",
                    hint: "Submitting feedback or benchmarks returned an error.",
                },
                FeedbackIssueOption {
                    code: "queued_never_syncs",
                    label: "Queued but never syncs",
                    hint: "It said it was queued locally, but it never uploads later.",
                },
                FeedbackIssueOption {
                    code: "rate_limited",
                    label: "Rate limited",
                    hint: "I got blocked for sending too many requests.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::DataQuality,
            label: FeedbackCategory::DataQuality.label(),
            description: FeedbackCategory::DataQuality.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "metrics_mismatch",
                    label: "My numbers don’t match my tool",
                    hint: "Average/1%/0.1% lows don’t match the source I used.",
                },
                FeedbackIssueOption {
                    code: "asked_to_average_again",
                    label: "It asked me to re-average results",
                    hint: "My tool already calculated metrics, but the app felt like it recalculated again.",
                },
                FeedbackIssueOption {
                    code: "units_or_rounding",
                    label: "Rounding/units looked wrong",
                    hint: "Values looked rounded oddly or shown in the wrong units.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::Performance,
            label: FeedbackCategory::Performance.label(),
            description: FeedbackCategory::Performance.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "crash",
                    label: "App crashed",
                    hint: "The app exited unexpectedly or showed a crash error.",
                },
                FeedbackIssueOption {
                    code: "freeze",
                    label: "App froze or got stuck",
                    hint: "The UI stopped responding or the terminal hung.",
                },
                FeedbackIssueOption {
                    code: "slow",
                    label: "App felt slow",
                    hint: "Pages took too long to load or actions were sluggish.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::PrivacyConsent,
            label: FeedbackCategory::PrivacyConsent.label(),
            description: FeedbackCategory::PrivacyConsent.description(),
            issues: vec![
                FeedbackIssueOption {
                    code: "consent_unclear",
                    label: "Consent text was unclear",
                    hint: "I didn’t understand what I was agreeing to.",
                },
                FeedbackIssueOption {
                    code: "privacy_question",
                    label: "Privacy question",
                    hint: "I have a question about what data is collected or how it is used.",
                },
                FeedbackIssueOption {
                    code: "retention_concern",
                    label: "Retention concern",
                    hint: "I’m concerned about how long data is stored.",
                },
            ],
        },
        FeedbackCategorySchema {
            id: FeedbackCategory::Other,
            label: FeedbackCategory::Other.label(),
            description: FeedbackCategory::Other.description(),
            issues: vec![FeedbackIssueOption {
                code: "other",
                label: "Something else",
                hint: "Tell us what happened and what you expected.",
            }],
        },
    ];

    FeedbackSchema {
        schema_version: 1,
        surface,
        os,
        intro:
            "Help improve FPS Tracker by reporting bugs, confusing steps, or data quality issues.",
        privacy_note:
            "Please do not include passwords, emails, or private links. Diagnostics are optional.",
        categories,
    }
}

fn capture_issue_options_for(os: OsFamily) -> Vec<FeedbackIssueOption> {
    let mut issues = vec![
        FeedbackIssueOption {
            code: "capture_wont_start",
            label: "Capture would not start",
            hint: "I followed the steps, but the app couldn’t capture performance data.",
        },
        FeedbackIssueOption {
            code: "capture_numbers_wrong",
            label: "Capture numbers looked wrong",
            hint: "FPS/low values looked obviously incorrect compared to what I saw in-game.",
        },
        FeedbackIssueOption {
            code: "capture_stops",
            label: "Capture stopped unexpectedly",
            hint: "It started but stopped early or ended with an error.",
        },
        FeedbackIssueOption {
            code: "alt_tab_broke_capture",
            label: "Alt-tab / focus changes broke capture",
            hint: "Switching windows or losing focus seemed to ruin the capture.",
        },
    ];

    if os == OsFamily::Windows {
        issues.push(FeedbackIssueOption {
            code: "needs_admin",
            label: "It only works as Administrator",
            hint: "Capture or benchmarks only worked when running as admin.",
        });
    }

    issues
}

fn synthetic_issue_options_for(os: OsFamily) -> Vec<FeedbackIssueOption> {
    let mut issues = vec![
        FeedbackIssueOption {
            code: "synthetic_wont_run",
            label: "Benchmarks would not run",
            hint: "The optional synthetic benchmark step failed to start or errored.",
        },
        FeedbackIssueOption {
            code: "synthetic_scores_weird",
            label: "Scores looked unrealistic",
            hint: "The numbers looked way too high/low or didn’t match expectations.",
        },
    ];

    if os == OsFamily::Windows {
        issues.push(FeedbackIssueOption {
            code: "synthetic_permission",
            label: "Permission / security blocked it",
            hint: "I saw a permission error or Windows blocked the benchmark run.",
        });
    }

    issues
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FeedbackDiagnostics {
    pub(crate) app_version: String,
    pub(crate) os: String,
    pub(crate) os_version: Option<String>,
    pub(crate) surface: FeedbackSurface,
    pub(crate) dependency_statuses: Vec<SanitizedDependencyStatus>,
    #[cfg(target_os = "windows")]
    pub(crate) windows_runtime_probe: Option<WindowsRuntimeProbeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SanitizedDependencyStatus {
    pub(crate) name: String,
    pub(crate) required: bool,
    pub(crate) available: bool,
    pub(crate) details: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WindowsRuntimeProbeSummary {
    pub(crate) winget_available: bool,
    pub(crate) presentmon_help_ok: bool,
    pub(crate) presentmon_help_summary: String,
}

pub(crate) fn collect_diagnostics(surface: FeedbackSurface) -> FeedbackDiagnostics {
    let os = sysinfo::System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = sysinfo::System::os_version();

    let dependency_statuses = deps::collect_dependency_statuses()
        .into_iter()
        .map(|status| SanitizedDependencyStatus {
            name: status.name.to_string(),
            required: status.required,
            available: status.available,
            details: sanitize_dependency_details(status.name, &status.details),
        })
        .collect::<Vec<_>>();

    #[cfg(target_os = "windows")]
    let windows_runtime_probe = Some({
        let probe = deps::probe_windows_runtime();
        WindowsRuntimeProbeSummary {
            winget_available: probe.winget_available,
            presentmon_help_ok: probe.presentmon_help_ok,
            presentmon_help_summary: probe.presentmon_help_summary,
        }
    });

    FeedbackDiagnostics {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os,
        os_version,
        surface,
        dependency_statuses,
        #[cfg(target_os = "windows")]
        windows_runtime_probe,
    }
}

fn sanitize_dependency_details(name: &str, details: &str) -> String {
    // Avoid leaking local paths (often contain usernames) even in opt-in diagnostics.
    // Keep meaningful static hints intact.
    let looks_like_path = details.contains('\\')
        || details.contains('/')
        || details.contains(":\\")
        || details.starts_with("\\\\");

    if name == "presentmon" && looks_like_path {
        return "Detected (path hidden)".to_string();
    }

    if looks_like_path {
        return "Available (path hidden)".to_string();
    }

    details.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FeedbackSubmission {
    pub(crate) surface: FeedbackSurface,
    pub(crate) category: FeedbackCategory,
    pub(crate) issue_code: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics: Option<FeedbackDiagnostics>,
}

impl FeedbackSubmission {
    pub(crate) fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.issue_code.trim().is_empty() {
            errors.push("Issue is required".to_string());
        }
        let msg = self.message.trim();
        if msg.is_empty() {
            errors.push("Message is required".to_string());
        }
        if msg.len() > 2000 {
            errors.push("Message is too long (max 2000 characters)".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FeedbackBackendResponse {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
}
