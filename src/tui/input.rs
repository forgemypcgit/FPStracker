//! TUI keyboard input handling.

use anyhow::Result;
use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::mpsc;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::Arc;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::time::Instant;

use crate::benchmark::BenchmarkSubmission;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::benchmark_runner;
use crate::config::Config;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::deps;
use crate::feedback::{self, FeedbackSubmission, FeedbackSurface};
use crate::games::KNOWN_GAMES;
use crate::hardware::SystemInfo;
use crate::{api, idempotency, storage};

use super::state::*;

pub(crate) fn handle_key(
    _rt: &tokio::runtime::Runtime,
    app: &mut App,
    key: KeyEvent,
) -> Result<bool> {
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.exit = Some(TuiExit::Quit);
        return Ok(true);
    }

    match app.screen {
        Screen::Home => handle_home_key(app, key),
        Screen::Contribute(step) => handle_contribute_key(app, step, key),
        Screen::ContributeResult => handle_contribute_result_key(app, key),
        Screen::Feedback(step) => handle_feedback_key(app, step, key),
        Screen::FeedbackResult => handle_feedback_result_key(app, key),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticRunning => handle_synthetic_running_key(app, key),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticResult => handle_synthetic_result_key(app, key),
        Screen::ErrorModal => handle_error_modal_key(app, key),
    }
}

fn handle_home_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.exit = Some(TuiExit::Quit);
            return Ok(true);
        }
        KeyCode::Up => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                app.home_choice = match app.home_choice {
                    HomeChoice::GuidedFlow => HomeChoice::Quit,
                    HomeChoice::Synthetic => HomeChoice::GuidedFlow,
                    HomeChoice::Feedback => HomeChoice::Synthetic,
                    HomeChoice::Quit => HomeChoice::Feedback,
                };
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            {
                app.home_choice = match app.home_choice {
                    HomeChoice::GuidedFlow => HomeChoice::Quit,
                    HomeChoice::Feedback => HomeChoice::GuidedFlow,
                    HomeChoice::Quit => HomeChoice::Feedback,
                };
            }
        }
        KeyCode::Down => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                app.home_choice = match app.home_choice {
                    HomeChoice::GuidedFlow => HomeChoice::Synthetic,
                    HomeChoice::Synthetic => HomeChoice::Feedback,
                    HomeChoice::Feedback => HomeChoice::Quit,
                    HomeChoice::Quit => HomeChoice::GuidedFlow,
                };
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            {
                app.home_choice = match app.home_choice {
                    HomeChoice::GuidedFlow => HomeChoice::Feedback,
                    HomeChoice::Feedback => HomeChoice::Quit,
                    HomeChoice::Quit => HomeChoice::GuidedFlow,
                };
            }
        }
        KeyCode::Enter => match app.home_choice {
            HomeChoice::GuidedFlow => {
                start_contribute(app);
            }
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            HomeChoice::Synthetic => start_synthetic(app),
            HomeChoice::Feedback => {
                app.feedback = FeedbackDraft::new();
                app.screen = Screen::Feedback(FeedbackStep::Category);
            }
            HomeChoice::Quit => {
                app.exit = Some(TuiExit::Quit);
                return Ok(true);
            }
        },
        KeyCode::Char('f') | KeyCode::Char('F') => {
            app.feedback = FeedbackDraft::new();
            app.screen = Screen::Feedback(FeedbackStep::Category);
        }
        #[cfg(target_os = "windows")]
        KeyCode::Char('i') | KeyCode::Char('I') => {
            if app.presentmon_install.is_some() {
                return Ok(false);
            }
            if deps::locate_presentmon_executable().is_none() {
                app.set_confirm(
                    "Install PresentMon",
                    "PresentMon enables Windows live auto-capture (frametime capture).\n\nWe will install Intel.PresentMon.Console via winget when available, otherwise bootstrap the official GitHub release.\n\nProceed?",
                    ConfirmAction::InstallPresentmon,
                );
            } else {
                app.set_info(
                    "PresentMon already installed",
                    "PresentMon is already available on this system.",
                );
            }
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn start_synthetic(app: &mut App) {
    app.synthetic_return = SyntheticReturn::Home;
    app.synthetic_return_screen = Screen::Home;

    let (tx, rx) = mpsc::channel();
    app.synthetic = Some(SyntheticState {
        started_at: Instant::now(),
        rx,
    });
    app.synthetic_result = None;
    app.synthetic_error = None;
    app.synthetic_progress = None;
    app.screen = Screen::SyntheticRunning;

    std::thread::spawn(move || {
        let progress_tx = tx.clone();
        let options = benchmark_runner::BenchmarkRunOptions {
            quiet: true,
            progress: Some(Arc::new(move |update| {
                let _ = progress_tx.send(SyntheticWorkerEvent::Progress(update));
            })),
        };
        let out = benchmark_runner::run_benchmarks_with_options(
            benchmark_runner::BenchmarkType::Standard,
            options,
        );
        let _ = tx.send(SyntheticWorkerEvent::Finished(Box::new(out)));
    });
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn handle_synthetic_running_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if key.code == KeyCode::Esc {
        app.synthetic = None;
        app.synthetic_progress = None;
        app.screen = app.synthetic_return_screen;
    }
    Ok(false)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn handle_synthetic_result_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => {
            if app.synthetic_return == SyntheticReturn::Contribute {
                if let Some(results) = app.synthetic_result.clone() {
                    app.contribute.baseline = Some(results);
                }
            }
            app.synthetic_progress = None;
            app.screen = app.synthetic_return_screen;
        }
        _ => {}
    }
    Ok(false)
}

pub(crate) fn start_contribute(app: &mut App) {
    let cfg = Config::load().unwrap_or_default();
    app.contribute = ContributeState::new(&cfg);
    app.synthetic_return = SyntheticReturn::Home;
    app.synthetic_return_screen = Screen::Home;

    if app.contribute.consent.is_complete() {
        app.screen = Screen::Contribute(ContributeStep::Hardware);
    } else {
        app.screen = Screen::Contribute(ContributeStep::Consent);
    }
}

fn handle_contribute_key(app: &mut App, step: ContributeStep, key: KeyEvent) -> Result<bool> {
    match step {
        ContributeStep::Consent => handle_contribute_consent_key(app, key),
        ContributeStep::Hardware => handle_contribute_hardware_key(app, key),
        ContributeStep::Baseline => handle_contribute_baseline_key(app, key),
        ContributeStep::Game => handle_contribute_game_key(app, key),
        ContributeStep::Results => handle_contribute_results_key(app, key),
        ContributeStep::Review => handle_contribute_review_key(app, key),
        ContributeStep::Submitting => Ok(false),
    }
}

fn handle_contribute_result_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => {
            app.contribute.result_message = None;
            app.screen = Screen::Home;
        }
        _ => {}
    }
    Ok(false)
}

fn handle_contribute_consent_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Home,
        KeyCode::Up => {
            app.contribute.consent.cursor = app.contribute.consent.cursor.saturating_sub(1)
        }
        KeyCode::Down => {
            app.contribute.consent.cursor = (app.contribute.consent.cursor + 1).min(2);
        }
        KeyCode::Char(' ') => app.contribute.consent.toggle_current(),
        KeyCode::Enter => {
            if app.contribute.consent.is_complete() {
                let mut cfg = Config::load().unwrap_or_default();
                cfg.consent.tos_accepted = true;
                cfg.consent.consent_public_use = true;
                cfg.consent.retention_acknowledged = true;
                cfg.consent.accepted_at_utc = Some(Utc::now());
                if let Err(err) = cfg.save() {
                    app.set_error("Save failed", err.to_string());
                    return Ok(false);
                }
                app.screen = Screen::Contribute(ContributeStep::Hardware);
            }
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn start_detect_hardware(app: &mut App) {
    let (tx, rx) = mpsc::channel();
    app.contribute.detect = Some(DetectState {
        started_at: Instant::now(),
        rx,
    });
    std::thread::spawn(move || {
        let out = SystemInfo::detect();
        let _ = tx.send(out);
    });
}

fn handle_contribute_hardware_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Home,
        KeyCode::Char('d') | KeyCode::Char('D') => {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            start_detect_hardware(app);
        }
        KeyCode::Char('g') | KeyCode::Char('G') => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.confirm_gpu = !hw.confirm_gpu;
            }
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.confirm_cpu = !hw.confirm_cpu;
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.confirm_ram = !hw.confirm_ram;
            }
        }
        KeyCode::Tab => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.next_field();
            }
        }
        KeyCode::BackTab => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.prev_field();
            }
        }
        KeyCode::Up => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.prev_field();
            }
        }
        KeyCode::Down => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                hw.next_field();
            }
        }
        KeyCode::Enter => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                if hw.mode == InputMode::Edit {
                    hw.commit_into_info();
                    hw.mode = InputMode::Navigate;
                } else if hw.can_continue() {
                    hw.commit_into_info();
                    app.screen = Screen::Contribute(ContributeStep::Baseline);
                } else {
                    hw.mode = InputMode::Edit;
                }
            } else {
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                start_detect_hardware(app);
            }
        }
        KeyCode::Backspace => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                if hw.mode == InputMode::Edit {
                    hw.active_value_mut().pop();
                }
            }
        }
        KeyCode::Char(ch) => {
            if let Some(hw) = app.contribute.hardware.as_mut() {
                if hw.mode == InputMode::Edit {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        return Ok(false);
                    }
                    hw.active_value_mut().push(ch);
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_contribute_baseline_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Contribute(ContributeStep::Hardware),
        KeyCode::Enter => app.screen = Screen::Contribute(ContributeStep::Game),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.screen = Screen::Contribute(ContributeStep::Game)
        }
        #[cfg(target_os = "linux")]
        KeyCode::Char('i') | KeyCode::Char('I') => {
            let statuses = deps::collect_dependency_statuses();
            if let Some(cmd) = deps::dependency_bulk_install_command(&statuses) {
                app.set_info(
                    "Install optional tools",
                    format!(
                        "To improve Linux baseline coverage, install optional tools (glmark2/sysbench/fio/stress-ng).\n\nRun this in a terminal:\n{cmd}\n\nThen come back and press B to run the baseline."
                    ),
                );
            } else {
                app.set_info(
                    "Optional tools already installed",
                    "glmark2/sysbench/fio/stress-ng are already available on this system.",
                );
            }
        }
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        KeyCode::Char('b') | KeyCode::Char('B') => {
            app.synthetic_return = SyntheticReturn::Contribute;
            app.synthetic_return_screen = Screen::Contribute(ContributeStep::Baseline);
            start_synthetic_contribute(app);
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn start_synthetic_contribute(app: &mut App) {
    let (tx, rx) = mpsc::channel();
    app.synthetic = Some(SyntheticState {
        started_at: Instant::now(),
        rx,
    });
    app.synthetic_result = None;
    app.synthetic_error = None;
    app.synthetic_progress = None;
    app.screen = Screen::SyntheticRunning;

    std::thread::spawn(move || {
        let progress_tx = tx.clone();
        let options = benchmark_runner::BenchmarkRunOptions {
            quiet: true,
            progress: Some(Arc::new(move |update| {
                let _ = progress_tx.send(SyntheticWorkerEvent::Progress(update));
            })),
        };
        let out = benchmark_runner::run_benchmarks_with_options(
            benchmark_runner::BenchmarkType::Standard,
            options,
        );
        let _ = tx.send(SyntheticWorkerEvent::Finished(Box::new(out)));
    });
}

fn handle_contribute_game_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let games = filtered_games(&app.contribute.game.query);
    match key.code {
        KeyCode::Esc => app.screen = Screen::Contribute(ContributeStep::Baseline),
        KeyCode::Up => app.contribute.game.cursor = app.contribute.game.cursor.saturating_sub(1),
        KeyCode::Down => {
            app.contribute.game.cursor =
                (app.contribute.game.cursor + 1).min(games.len().saturating_sub(1));
        }
        KeyCode::Backspace => {
            app.contribute.game.query.pop();
            app.contribute.game.cursor = 0;
        }
        KeyCode::Enter => {
            if let Some(idx) = games.get(app.contribute.game.cursor).copied() {
                app.contribute.selected_game = Some(idx);
                app.screen = Screen::Contribute(ContributeStep::Results);
            }
        }
        KeyCode::Char(ch) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(false);
            }
            if !ch.is_control() {
                app.contribute.game.query.push(ch);
                app.contribute.game.cursor = 0;
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_contribute_results_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Contribute(ContributeStep::Game),
        KeyCode::Up => {
            app.contribute.results.cursor = app.contribute.results.cursor.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Tab => {
            app.contribute.results.cursor = (app.contribute.results.cursor + 1).min(8)
        }
        KeyCode::BackTab => {
            app.contribute.results.cursor = app.contribute.results.cursor.saturating_sub(1)
        }
        KeyCode::Char(' ') => match app.contribute.results.cursor {
            5 => app.contribute.results.ray_tracing = !app.contribute.results.ray_tracing,
            7 => app.contribute.results.anti_cheat_ack = !app.contribute.results.anti_cheat_ack,
            _ => {}
        },
        KeyCode::Left => {
            if app.contribute.results.cursor == 6 {
                app.contribute.results.capture_method = match app.contribute.results.capture_method
                {
                    CaptureMethodChoice::InGameCounter => CaptureMethodChoice::ExternalTool,
                    CaptureMethodChoice::BuiltInBenchmark => CaptureMethodChoice::InGameCounter,
                    CaptureMethodChoice::ExternalTool => CaptureMethodChoice::BuiltInBenchmark,
                };
            }
        }
        KeyCode::Right => {
            if app.contribute.results.cursor == 6 {
                app.contribute.results.capture_method = match app.contribute.results.capture_method
                {
                    CaptureMethodChoice::InGameCounter => CaptureMethodChoice::BuiltInBenchmark,
                    CaptureMethodChoice::BuiltInBenchmark => CaptureMethodChoice::ExternalTool,
                    CaptureMethodChoice::ExternalTool => CaptureMethodChoice::InGameCounter,
                };
            }
        }
        KeyCode::Enter => {
            if app.contribute.results.mode == InputMode::Edit {
                app.contribute.results.mode = InputMode::Navigate;
            } else {
                app.screen = Screen::Contribute(ContributeStep::Review);
            }
        }
        KeyCode::Backspace => {
            if app.contribute.results.mode == InputMode::Edit {
                active_results_field_mut(&mut app.contribute.results).pop();
            }
        }
        KeyCode::Char(ch) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(false);
            }
            match app.contribute.results.cursor {
                5..=7 => {}
                _ => {
                    if app.contribute.results.mode == InputMode::Navigate {
                        app.contribute.results.mode = InputMode::Edit;
                    }
                    active_results_field_mut(&mut app.contribute.results).push(ch);
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_contribute_review_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = Screen::Contribute(ContributeStep::Results),
        KeyCode::Enter => {
            app.screen = Screen::Contribute(ContributeStep::Submitting);
            app.pending_action = Some(Action::SubmitBenchmark);
        }
        KeyCode::Char('1') => {
            app.contribute.review_expanded[0] = !app.contribute.review_expanded[0]
        }
        KeyCode::Char('2') => {
            app.contribute.review_expanded[1] = !app.contribute.review_expanded[1]
        }
        KeyCode::Char('3') => {
            app.contribute.review_expanded[2] = !app.contribute.review_expanded[2]
        }
        _ => {}
    }
    Ok(false)
}

fn handle_feedback_key(app: &mut App, step: FeedbackStep, key: KeyEvent) -> Result<bool> {
    match step {
        FeedbackStep::Category => match key.code {
            KeyCode::Esc => app.screen = Screen::Home,
            KeyCode::Up => {
                app.feedback.category_index = app.feedback.category_index.saturating_sub(1);
                app.feedback.issue_index = 0;
            }
            KeyCode::Down => {
                app.feedback.category_index = (app.feedback.category_index + 1)
                    .min(app.schema.categories.len().saturating_sub(1));
                app.feedback.issue_index = 0;
            }
            KeyCode::Enter => app.screen = Screen::Feedback(FeedbackStep::Issue),
            _ => {}
        },
        FeedbackStep::Issue => match key.code {
            KeyCode::Esc => app.screen = Screen::Feedback(FeedbackStep::Category),
            KeyCode::Up => app.feedback.issue_index = app.feedback.issue_index.saturating_sub(1),
            KeyCode::Down => {
                app.feedback.issue_index = (app.feedback.issue_index + 1)
                    .min(app.category().issues.len().saturating_sub(1))
            }
            KeyCode::Enter => app.screen = Screen::Feedback(FeedbackStep::Message),
            _ => {}
        },
        FeedbackStep::Message => {
            if key.code == KeyCode::F(5)
                || (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                app.screen = Screen::Feedback(FeedbackStep::Submitting);
                app.pending_action = Some(Action::SubmitFeedback);
                return Ok(false);
            }

            match key.code {
                KeyCode::Esc => app.screen = Screen::Feedback(FeedbackStep::Issue),
                KeyCode::Tab => {
                    app.feedback.include_diagnostics = !app.feedback.include_diagnostics
                }
                KeyCode::Backspace => {
                    app.feedback.message.pop();
                }
                KeyCode::Enter => app.feedback.message.push('\n'),
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        return Ok(false);
                    }
                    app.feedback.message.push(c);
                }
                _ => {}
            }
        }
        FeedbackStep::Submitting => {}
    }

    Ok(false)
}

fn handle_feedback_result_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => {
            app.feedback_result = None;
            app.screen = Screen::Home;
        }
        _ => {}
    }
    Ok(false)
}

fn handle_error_modal_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.error_modal = None;
            app.screen = app.error_return_screen;
        }
        KeyCode::Enter => {
            #[cfg(target_os = "windows")]
            let action = app.error_modal.as_ref().and_then(|m| m.confirm_action);
            app.error_modal = None;
            app.screen = app.error_return_screen;

            #[cfg(target_os = "windows")]
            {
                if action == Some(ConfirmAction::InstallPresentmon) {
                    let (tx, rx) = mpsc::channel();
                    app.presentmon_install = Some(PresentmonInstallState { rx });

                    std::thread::spawn(move || {
                        let outcome = deps::ensure_presentmon_for_session(true);
                        let _ = tx.send(outcome);
                    });
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

pub(crate) fn handle_submit_feedback(rt: &tokio::runtime::Runtime, app: &mut App) {
    let category_schema = app.category().clone();
    let category_id = category_schema.id;
    let issue = app.issue();

    let message = app.feedback.message.trim().to_string();
    if message.is_empty() {
        app.set_error(
            "Missing message",
            "Please type a short description before submitting.",
        );
        app.screen = Screen::Feedback(FeedbackStep::Message);
        return;
    }

    let diagnostics = app
        .feedback
        .include_diagnostics
        .then(|| feedback::collect_diagnostics(FeedbackSurface::TerminalUi));

    let submission = FeedbackSubmission {
        surface: FeedbackSurface::TerminalUi,
        category: category_id,
        issue_code: issue.code.to_string(),
        message,
        diagnostics,
    };

    if let Err(errors) = submission.validate() {
        let mut msg = String::new();
        for e in errors {
            msg.push_str("- ");
            msg.push_str(&e);
            msg.push('\n');
        }
        app.set_error("Validation issues", msg.trim_end().to_string());
        app.screen = Screen::Feedback(FeedbackStep::Message);
        return;
    }

    let idempotency_key = idempotency::new_feedback_key();
    match rt.block_on(api::submit_feedback_with_idempotency_key(
        &submission,
        &idempotency_key,
    )) {
        Ok(_response) => {
            app.feedback_result = Some(FeedbackResultState {
                title: "Feedback sent".to_string(),
                body: "Thanks. Your feedback was sent.\n\nPress Enter to return.".to_string(),
            });
            app.screen = Screen::FeedbackResult;
        }
        Err(err) => {
            if api::should_queue_offline_feedback(&err) {
                match storage::init_storage().and_then(|s| {
                    s.save_pending_feedback_with_idempotency_key(&submission, &idempotency_key)
                }) {
                    Ok(_pending_id) => {
                        app.feedback_result = Some(FeedbackResultState {
                            title: "Queued locally".to_string(),
                            body: "Could not send right now.\nSaved locally for retry.\n\nWe will retry automatically.\n\nPress Enter to return.".to_string(),
                        });
                        app.screen = Screen::FeedbackResult;
                    }
                    Err(store_err) => {
                        app.set_error(
                            "Submit failed",
                            format!("{err}\n\nAlso failed to queue locally: {store_err}"),
                        );
                        app.screen = Screen::Feedback(FeedbackStep::Message);
                    }
                }
            } else {
                app.set_error("Submit failed", err.to_string());
                app.screen = Screen::Feedback(FeedbackStep::Message);
            }
        }
    }
}

pub(crate) fn handle_submit_benchmark(rt: &tokio::runtime::Runtime, app: &mut App) {
    let (submission, issues) = build_submission_preview(app);
    if let Some(issues) = issues {
        let mut msg = String::new();
        for issue in issues {
            msg.push_str("- ");
            msg.push_str(&issue);
            msg.push('\n');
        }
        app.set_error("Cannot submit", msg.trim_end().to_string());
        app.screen = Screen::Contribute(ContributeStep::Review);
        return;
    }
    let submission = match submission {
        Some(s) => s,
        None => {
            app.set_error("Cannot submit", "Missing submission.".to_string());
            app.screen = Screen::Contribute(ContributeStep::Review);
            return;
        }
    };

    let idempotency_key = idempotency::new_submit_key();
    match rt.block_on(api::submit_benchmark_with_idempotency_key(
        &submission,
        &idempotency_key,
    )) {
        Ok(_response) => {
            app.contribute.result_message = Some(MessageResultState {
                title: "Submitted".to_string(),
                body: "Thanks. Your benchmark was submitted.\n\nPress Enter to return.".to_string(),
            });
            app.screen = Screen::ContributeResult;
        }
        Err(err) => {
            if api::should_queue_offline(&err) {
                match storage::init_storage().and_then(|s| {
                    s.save_pending_benchmark_with_idempotency_key(&submission, &idempotency_key)
                }) {
                    Ok(_pending_id) => {
                        app.contribute.result_message = Some(MessageResultState {
                            title: "Queued locally".to_string(),
                            body: "Could not submit right now.\nSaved locally for retry.\n\nWe will retry automatically.\n\nPress Enter to return.".to_string(),
                        });
                        app.screen = Screen::ContributeResult;
                    }
                    Err(store_err) => {
                        app.set_error(
                            "Submit failed",
                            format!("{err}\n\nAlso failed to queue locally: {store_err}"),
                        );
                        app.screen = Screen::Contribute(ContributeStep::Review);
                    }
                }
            } else {
                app.set_error("Submit failed", err.to_string());
                app.screen = Screen::Contribute(ContributeStep::Review);
            }
        }
    }
}

pub(crate) fn build_submission_preview(
    app: &App,
) -> (Option<BenchmarkSubmission>, Option<Vec<String>>) {
    let hw = match app.contribute.hardware.as_ref() {
        Some(hw) => hw,
        None => return (None, Some(vec!["Hardware is missing.".to_string()])),
    };
    let game_idx = match app.contribute.selected_game {
        Some(idx) => idx,
        None => return (None, Some(vec!["No game selected.".to_string()])),
    };

    let mut hw_clone = hw.clone();
    hw_clone.commit_into_info();
    let info = hw_clone.info;

    let avg_fps: f64 = match app.contribute.results.avg_fps.trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                None,
                Some(vec!["Average FPS must be a number.".to_string()]),
            )
        }
    };

    let fps_1_low = app
        .contribute
        .results
        .fps_1_low
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| *v > 0.0);
    let fps_01_low = app
        .contribute
        .results
        .fps_01_low
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| *v > 0.0);

    let upscaling = app.contribute.results.upscaling.trim().to_string();
    let upscaling = if upscaling.is_empty() {
        None
    } else {
        Some(upscaling)
    };

    if !app.contribute.results.anti_cheat_ack {
        return (
            None,
            Some(vec!["Anti-cheat acknowledgment is required.".to_string()]),
        );
    }

    let game = KNOWN_GAMES[game_idx].name.to_string();

    let mut submission = BenchmarkSubmission::new(
        info,
        game,
        app.contribute.results.resolution.trim().to_string(),
        app.contribute.results.preset.trim().to_string(),
        avg_fps,
        fps_1_low,
        app.contribute.results.ray_tracing,
        upscaling,
    );

    submission.fps_01_low = fps_01_low;
    submission.capture_method = Some(
        app.contribute
            .results
            .capture_method
            .as_api_value()
            .to_string(),
    );
    submission.anti_cheat_acknowledged = Some(true);

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Some(b) = app.contribute.baseline.as_ref() {
        submission.synthetic_cpu_score = b.cpu_score;
        submission.synthetic_gpu_score = b.gpu_score;
        submission.synthetic_ram_score = b.ram_score;
        submission.synthetic_disk_score = b.disk_score;
        submission.synthetic_cpu_source = b.cpu_score_source.clone();
        submission.synthetic_gpu_source = b.gpu_score_source.clone();
        submission.synthetic_ram_source = b.ram_score_source.clone();
        submission.synthetic_disk_source = b.disk_score_source.clone();
        submission.synthetic_profile = Some("standard".to_string());
        submission.synthetic_suite_version =
            Some(benchmark_runner::SYNTHETIC_SUITE_VERSION.to_string());
        submission.synthetic_extended = serde_json::to_value(b).ok();
        submission.duration_secs = Some(b.duration_secs);
    }

    match submission.validate() {
        Ok(()) => (Some(submission), None),
        Err(errs) => (None, Some(errs)),
    }
}
