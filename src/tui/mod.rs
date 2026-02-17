//! Fullscreen terminal UI (TUI).
//!
//! This is intentionally small and self-contained so we can evolve it without
//! entangling the core capture/submission logic.

pub(crate) mod animation;
pub(crate) mod input;
pub(crate) mod screens;
pub(crate) mod state;
pub(crate) mod theme;
pub(crate) mod widgets;

use std::io;
use std::time::Duration;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::mpsc::TryRecvError;

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Terminal;

use state::*;
use theme::Theme;

const FRAME_TIME: Duration = Duration::from_millis(16);

// Re-export TuiExit for use by main.
pub(crate) use state::TuiExit;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub(crate) fn run_tui(rt: &tokio::runtime::Runtime) -> Result<TuiExit> {
    let _guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| draw(f.area(), f, &app))?;

        #[cfg(target_os = "windows")]
        {
            if matches!(app.screen, Screen::Home) {
                if let Some(state) = app.presentmon_install.as_ref() {
                    match state.rx.try_recv() {
                        Ok(outcome) => {
                            app.presentmon_install = None;
                            match outcome {
                                Ok(Some(path)) => {
                                    app.set_info(
                                        "PresentMon installed",
                                        format!("PresentMon ready:\n{}", path.display()),
                                    );
                                }
                                Ok(None) => {
                                    app.set_error(
                                        "Install incomplete",
                                        "PresentMon was not installed.".to_string(),
                                    );
                                }
                                Err(err) => {
                                    app.set_error("Install failed", err.to_string());
                                }
                            }
                            continue;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            app.presentmon_install = None;
                            app.set_error(
                                "Install failed",
                                "Install worker stopped unexpectedly.".to_string(),
                            );
                            continue;
                        }
                    }
                }
            }
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            if matches!(app.screen, Screen::Contribute(ContributeStep::Hardware)) {
                if let Some(state) = app.contribute.detect.as_ref() {
                    match state.rx.try_recv() {
                        Ok(outcome) => {
                            app.contribute.detect = None;
                            match outcome {
                                Ok(info) => {
                                    app.contribute.hardware = Some(HardwareForm::from_info(info));
                                }
                                Err(err) => {
                                    app.set_error("Detection failed", err.to_string());
                                }
                            }
                            continue;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            app.contribute.detect = None;
                            app.set_error(
                                "Detection failed",
                                "Hardware detection worker stopped unexpectedly.".to_string(),
                            );
                            continue;
                        }
                    }
                }
            }
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            if matches!(app.screen, Screen::SyntheticRunning) {
                if let Some(state) = app.synthetic.as_ref() {
                    match state.rx.try_recv() {
                        Ok(SyntheticWorkerEvent::Progress(update)) => {
                            app.synthetic_progress = Some(update);
                        }
                        Ok(SyntheticWorkerEvent::Finished(outcome)) => {
                            app.synthetic = None;
                            match *outcome {
                                Ok(results) => {
                                    app.synthetic_result = Some(results);
                                    app.synthetic_error = None;
                                }
                                Err(err) => {
                                    app.synthetic_result = None;
                                    app.synthetic_error = Some(err.to_string());
                                }
                            }
                            app.screen = Screen::SyntheticResult;
                            continue;
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            app.synthetic = None;
                            app.synthetic_result = None;
                            app.synthetic_error = Some(
                                "Synthetic benchmark worker stopped unexpectedly.".to_string(),
                            );
                            app.screen = Screen::SyntheticResult;
                            continue;
                        }
                    }
                }
            }
        }

        if let Some(action) = app.pending_action.take() {
            match action {
                Action::SubmitFeedback => input::handle_submit_feedback(rt, &mut app),
                Action::SubmitBenchmark => input::handle_submit_benchmark(rt, &mut app),
            }
            continue;
        }

        let timeout = FRAME_TIME.saturating_sub(app.last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if input::handle_key(rt, &mut app, key)? {
                    break;
                }
            }
        }

        if app.last_tick.elapsed() >= FRAME_TIME {
            app.last_tick = std::time::Instant::now();
            app.animation.advance();
        }
    }

    Ok(app.exit.unwrap_or(TuiExit::Quit))
}

fn draw(area: Rect, f: &mut ratatui::Frame, app: &App) {
    let theme = Theme::default();

    // Top header bar + content area
    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Min(0),    // content
        ])
        .split(area);

    // Header with context
    let context = match app.screen {
        Screen::Home => None,
        Screen::Contribute(step) => {
            let (idx, label) = match step {
                ContributeStep::Consent => (1, "Consent"),
                ContributeStep::Hardware => (2, "Hardware"),
                ContributeStep::Baseline => (3, "Baseline"),
                ContributeStep::Game => (4, "Game"),
                ContributeStep::Results => (5, "Results"),
                ContributeStep::Review | ContributeStep::Submitting => (6, "Review"),
            };
            Some(format!("CONTRIBUTE  {idx}/6  {label}"))
        }
        Screen::ContributeResult => Some("RESULT".to_string()),
        Screen::Feedback(_) => Some("FEEDBACK".to_string()),
        Screen::FeedbackResult => Some("FEEDBACK".to_string()),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticRunning => Some("SYNTHETIC BASELINE".to_string()),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticResult => Some("SYNTHETIC RESULT".to_string()),
        Screen::ErrorModal => None,
    };
    widgets::header::draw_header(outer_layout[0], f, &theme, context.as_deref());

    let inner = outer_layout[1];

    match app.screen {
        Screen::Home => screens::home::draw_home(inner, f, app, theme),
        Screen::Contribute(step) => draw_contribute(inner, f, app, theme, step),
        Screen::ContributeResult => screens::review::draw_contribute_result(inner, f, app, theme),
        Screen::Feedback(step) => screens::feedback::draw_feedback(inner, f, app, theme, step),
        Screen::FeedbackResult => screens::feedback::draw_feedback_result(inner, f, app, theme),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticRunning => screens::baseline::draw_synthetic_running(inner, f, app, theme),
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        Screen::SyntheticResult => screens::baseline::draw_synthetic_result(inner, f, app, theme),
        Screen::ErrorModal => {
            match app.error_return_screen {
                Screen::Home => screens::home::draw_home(inner, f, app, theme),
                Screen::Contribute(step) => draw_contribute(inner, f, app, theme, step),
                Screen::ContributeResult => {
                    screens::review::draw_contribute_result(inner, f, app, theme)
                }
                Screen::Feedback(step) => {
                    screens::feedback::draw_feedback(inner, f, app, theme, step)
                }
                Screen::FeedbackResult => {
                    screens::feedback::draw_feedback_result(inner, f, app, theme)
                }
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                Screen::SyntheticRunning => {
                    screens::baseline::draw_synthetic_running(inner, f, app, theme)
                }
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                Screen::SyntheticResult => {
                    screens::baseline::draw_synthetic_result(inner, f, app, theme)
                }
                Screen::ErrorModal => screens::home::draw_home(inner, f, app, theme),
            }
            screens::error::draw_error_modal(inner, f, app, theme);
        }
    }
}

fn draw_contribute(
    area: Rect,
    f: &mut ratatui::Frame,
    app: &App,
    theme: Theme,
    step: ContributeStep,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // progress stepper
            Constraint::Min(0),    // content
            Constraint::Length(1), // footer
        ])
        .split(area);

    let step_idx = match step {
        ContributeStep::Consent => 1,
        ContributeStep::Hardware => 2,
        ContributeStep::Baseline => 3,
        ContributeStep::Game => 4,
        ContributeStep::Results => 5,
        ContributeStep::Review | ContributeStep::Submitting => 6,
    };

    // Progress stepper
    widgets::progress::draw_progress(layout[0], f, &theme, step_idx);

    // Screen content
    match step {
        ContributeStep::Consent => {
            screens::consent::draw_contribute_consent(layout[1], f, app, theme)
        }
        ContributeStep::Hardware => {
            screens::hardware::draw_contribute_hardware(layout[1], f, app, theme)
        }
        ContributeStep::Baseline => {
            screens::baseline::draw_contribute_baseline(layout[1], f, app, theme)
        }
        ContributeStep::Game => screens::game::draw_contribute_game(layout[1], f, app, theme),
        ContributeStep::Results => {
            screens::results::draw_contribute_results(layout[1], f, app, theme)
        }
        ContributeStep::Review => screens::review::draw_contribute_review(layout[1], f, app, theme),
        ContributeStep::Submitting => {
            let spinner = app.animation.spinner_char();
            let para = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {spinner}  "), Style::default().fg(theme.oracle)),
                Span::styled(
                    "Submitting...",
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ]))
            .wrap(Wrap { trim: true });
            f.render_widget(para, layout[1]);
        }
    }

    // Context-sensitive footer
    let hints: &[(&str, &str)] = match step {
        ContributeStep::Consent => &[
            ("↑/↓", "Select"),
            ("Space", "Toggle"),
            ("Enter", "Continue"),
            ("Esc", "Back"),
        ],
        ContributeStep::Hardware => &[
            ("D", "Detect"),
            ("↑/↓/Tab", "Field"),
            ("Enter", "Edit"),
            ("G/C/R", "Confirm"),
            ("Esc", "Back"),
        ],
        ContributeStep::Baseline => &[
            ("B", "Run baseline"),
            ("S", "Skip"),
            ("Enter", "Continue"),
            ("Esc", "Back"),
        ],
        ContributeStep::Game => &[
            ("Type", "Search"),
            ("↑/↓", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Back"),
        ],
        ContributeStep::Results => &[
            ("↑/↓/Tab", "Field"),
            ("Type", "Edit"),
            ("◄/►", "Cycle"),
            ("Space", "Toggle"),
            ("Enter", "Continue"),
            ("Esc", "Back"),
        ],
        ContributeStep::Review => &[("1/2/3", "Expand"), ("Enter", "Submit"), ("Esc", "Edit")],
        ContributeStep::Submitting => &[],
    };
    widgets::footer::draw_footer(layout[2], f, &theme, hints);
}
