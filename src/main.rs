//! FPS Tracker - Guided benchmark submission
//!
//! This is a LIGHTWEIGHT data collection tool that:
//! - Does NOT inject code into games (lower anti-cheat risk than hook-based tools)
//! - Guided submission flow does NOT run during gameplay (no performance impact)
//! - Guides users through benchmark submission
//! - Anonymizes hardware data before submission

mod api;
mod api_routes;
mod benchmark;
mod benchmark_runner;
mod config;
mod deps;
mod feedback;
mod games;
mod hardware;
mod idempotency;
mod import;
mod server;
mod storage;
mod tui;

use crate::benchmark::live::{run_live_capture, CaptureSource, FocusPolicy, LiveCaptureOptions};
use crate::benchmark::BenchmarkSubmission;
use crate::benchmark_runner::{print_benchmark_warning, run_benchmarks, show_benchmark_menu};
use crate::feedback::FeedbackCategory;
use crate::games::{GameInfo, KNOWN_GAMES};
use crate::hardware::SystemInfo;
use crate::import::{parse_capframex_csv, parse_mangohud_log};
#[cfg(target_os = "windows")]
use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
    ENABLE_PROCESSED_OUTPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WRAP_AT_EOL_OUTPUT,
    STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const DEFAULT_UI_PORT: u16 = 3000;

/// FPS Tracker - Collect gaming benchmarks
#[derive(Parser)]
#[command(name = "fps-tracker")]
#[command(author = "ForgeMyPC")]
#[command(version)]
#[command(about = "Contribute gaming benchmarks to help others build better PCs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the guided benchmark submission flow (recommended)
    Start,

    /// Start the interactive web UI
    Ui {
        /// Port to run the server on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Do not auto-open the browser
        #[arg(long, default_value_t = false)]
        no_open: bool,
    },

    /// Start app mode (alias for Browser Mode)
    App {
        /// Port to run the server on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Do not auto-open the browser
        #[arg(long, default_value_t = false)]
        no_open: bool,
    },

    /// Detect and display your system hardware
    Detect,

    /// Quick submit (for experienced users)
    Submit {
        /// Game name (e.g., "Cyberpunk 2077")
        #[arg(short, long)]
        game: String,

        /// Resolution (e.g., "1440p", "4K")
        #[arg(short, long)]
        resolution: String,

        /// Average FPS
        #[arg(short, long)]
        fps: f64,

        /// Graphics preset (e.g., "Ultra", "High", "Medium")
        #[arg(short, long)]
        preset: String,

        /// 1% low FPS (optional)
        #[arg(long)]
        fps_1_low: Option<f64>,

        /// Ray tracing enabled
        #[arg(long)]
        ray_tracing: bool,

        /// DLSS/FSR mode (e.g., "Quality", "Balanced", "Performance")
        #[arg(long)]
        upscaling: Option<String>,
    },

    /// List known games with benchmark guidance
    Games,

    /// Show detailed info about a specific game
    Game {
        /// Game name to look up
        name: String,
    },

    /// Import benchmark from CapFrameX or MangoHud capture file
    Import {
        /// Path to capture file (CSV from CapFrameX or MangoHud)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Auto-detect the latest capture file
        #[arg(long)]
        auto: bool,
    },

    /// Manage and check PC builds for compatibility
    Build {
        #[command(subcommand)]
        command: BuildCommands,
    },

    /// Run real benchmark capture preview (frametime-based)
    Benchmark {
        #[command(subcommand)]
        command: BenchmarkCommands,
    },

    /// Show configuration and data paths
    Config,

    /// Show install/update/uninstall guidance for your platform
    InstallInfo,

    /// Check capture/runtime dependencies and optionally auto-fix missing tools
    Doctor {
        /// Attempt secure auto-fix for missing required tools
        #[arg(long, default_value_t = false)]
        fix: bool,

        /// Assume "yes" for all fix prompts (useful in non-interactive shells)
        #[arg(long, default_value_t = false, requires = "fix")]
        yes: bool,

        /// Run additional Windows runtime checks (PresentMon execution, winget availability)
        #[arg(long, default_value_t = false)]
        windows_runtime: bool,
    },

    /// Start the fullscreen terminal UI (beta)
    Tui,

    /// Send feedback about bugs, capture issues, or confusing steps
    Feedback,
}

#[derive(Subcommand)]
enum BenchmarkCommands {
    /// Capture live frametime samples from external tools
    Preview {
        /// Capture source
        #[arg(long, value_enum, default_value = "auto")]
        source: BenchmarkSourceArg,

        /// Capture duration in seconds (10-900)
        #[arg(short, long, default_value_t = 90)]
        duration: u64,

        /// Capture file path (MangoHud log path or PresentMon output file)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Optional game name hint
        #[arg(long)]
        game: Option<String>,

        /// Explicit target process name (recommended in strict mode), e.g. game.exe
        #[arg(long)]
        process_name: Option<String>,

        /// Override anti-cheat safety guard for high-risk games (not recommended)
        #[arg(long, default_value_t = false)]
        allow_anti_cheat_risk: bool,

        /// Focus policy while capture runs
        #[arg(long, value_enum)]
        focus_policy: Option<FocusPolicyArg>,

        /// Pause and drop samples while target process is unfocused
        #[arg(long)]
        pause_on_unfocus: Option<bool>,

        /// Enable strict process validation (recommended)
        #[arg(long)]
        process_validation: Option<bool>,

        /// Tail polling interval in milliseconds (50-500)
        #[arg(long)]
        poll_ms: Option<u64>,

        /// Ignore frame times above this threshold (ms)
        #[arg(long)]
        max_frame_time_ms: Option<f64>,

        /// Grace period (ms) before strict focus mode marks capture invalid
        #[arg(long)]
        strict_unfocus_grace_ms: Option<u64>,

        /// Submit captured result immediately
        #[arg(long, default_value_t = false)]
        submit: bool,

        /// Resolution for submission (required when --submit)
        #[arg(long)]
        resolution: Option<String>,

        /// Preset for submission (required when --submit)
        #[arg(long)]
        preset: Option<String>,

        /// Ray tracing enabled (used with --submit)
        #[arg(long, default_value_t = false)]
        ray_tracing: bool,

        /// Upscaling mode (used with --submit)
        #[arg(long)]
        upscaling: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum BenchmarkSourceArg {
    Auto,
    Mangohud,
    Presentmon,
}

impl From<BenchmarkSourceArg> for CaptureSource {
    fn from(value: BenchmarkSourceArg) -> Self {
        match value {
            BenchmarkSourceArg::Auto => CaptureSource::Auto,
            BenchmarkSourceArg::Mangohud => CaptureSource::MangoHud,
            BenchmarkSourceArg::Presentmon => CaptureSource::PresentMon,
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum FocusPolicyArg {
    Strict,
    Lenient,
}

impl From<FocusPolicyArg> for FocusPolicy {
    fn from(value: FocusPolicyArg) -> Self {
        match value {
            FocusPolicyArg::Strict => FocusPolicy::Strict,
            FocusPolicyArg::Lenient => FocusPolicy::Lenient,
        }
    }
}

#[derive(Subcommand)]
enum BuildCommands {
    /// Check a build for compatibility issues
    Check {
        /// Build name (saved build) or 'current' for detected hardware
        #[arg(default_value = "current")]
        name: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Strict mode - fail on warnings too
        #[arg(long)]
        strict: bool,
    },

    /// List saved builds
    List,

    /// Save current hardware as a build
    Save {
        /// Name for the build
        name: String,

        /// Optional notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Delete a saved build
    Delete {
        /// Name of the build to delete
        name: String,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[cfg(target_os = "windows")]
fn init_windows_console() {
    // Best-effort enabling of ANSI/VT sequences for nicer output in legacy hosts.
    // If the handle isn't a console (e.g., redirected), these calls will fail harmlessly.
    unsafe {
        let _ = SetConsoleOutputCP(65001);
        let _ = SetConsoleCP(65001);

        for handle_id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = GetStdHandle(handle_id);
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                continue;
            }

            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                continue;
            }

            let desired = mode
                | ENABLE_PROCESSED_OUTPUT
                | ENABLE_WRAP_AT_EOL_OUTPUT
                | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            let _ = SetConsoleMode(handle, desired);
        }
    }
}

#[cfg(target_os = "windows")]
fn is_windows_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut returned: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        ) != 0;
        let _ = CloseHandle(token);

        ok && elevation.TokenIsElevated != 0
    }
}

#[cfg(target_os = "windows")]
fn ps_single_quote(value: &str) -> String {
    // PowerShell single-quoted strings escape a literal quote by doubling it.
    value.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn maybe_offer_windows_admin_relaunch() -> Result<()> {
    // Avoid prompts in non-interactive/CI contexts.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }

    if is_windows_elevated() {
        return Ok(());
    }

    let skip = std::env::var("FPS_TRACKER_SKIP_ADMIN_PROMPT")
        .ok()
        .as_deref()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "y" || v == "on"
        })
        .unwrap_or(false);
    if skip {
        return Ok(());
    }

    // Only prompt when elevation meaningfully unlocks functionality (WinSAT baselines).
    if !crate::deps::is_command_available("winsat") {
        return Ok(());
    }

    println!(
        "\n{}",
        "Windows note: running as Administrator enables the most complete synthetic baseline (WinSAT) and can help dependency installs."
            .bright_black()
    );
    print!(
        "{} ",
        "Relaunch FPS Tracker as Administrator now? [y/N]:".bright_yellow()
    );
    let _ = io::stdout().flush();
    let allow = prompt_yes_no(false, false, false);
    if !allow {
        return Ok(());
    }

    let exe = std::env::current_exe()?;
    let exe_str = exe.display().to_string();
    let exe_q = ps_single_quote(&exe_str);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_list = if args.is_empty() {
        String::new()
    } else {
        let rendered = args
            .iter()
            .map(|a| format!("'{}'", ps_single_quote(a)))
            .collect::<Vec<_>>()
            .join(",");
        format!(" -ArgumentList {}", rendered)
    };

    let wd = std::env::current_dir()
        .ok()
        .map(|p| ps_single_quote(&p.display().to_string()));
    let wd_part = wd
        .as_deref()
        .map(|w| format!(" -WorkingDirectory '{w}'"))
        .unwrap_or_default();

    let script = format!("Start-Process -Verb RunAs -FilePath '{exe_q}'{args_list}{wd_part}");

    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            // The elevated copy continues; this instance should stop cleanly.
            std::process::exit(0);
        }
        Ok(s) => {
            println!(
                "{} {}",
                "Could not relaunch elevated (PowerShell exit):".bright_red(),
                s.to_string().bright_red()
            );
        }
        Err(err) => {
            println!(
                "{} {}",
                "Could not relaunch elevated:".bright_red(),
                err.to_string().bright_red()
            );
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    #[cfg(target_os = "windows")]
    init_windows_console();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Start) | None => {
            #[cfg(target_os = "windows")]
            maybe_offer_windows_admin_relaunch()?;

            if should_offer_browser_mode() && prompt_browser_mode_choice() {
                match launch_browser_mode(DEFAULT_UI_PORT) {
                    Ok(()) => return Ok(()),
                    Err(err) => {
                        println!(
                            "{} {}",
                            "Could not start Browser Mode:".bright_red(),
                            err.to_string().bright_red()
                        );
                        println!("{}", "Falling back to terminal flow...".bright_yellow());
                    }
                }
            }

            // Default to the fullscreen TUI when running interactively; fall back to the classic
            // line-by-line guided flow for non-TTY environments (CI logs, redirected stdin/stdout).
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                let rt = tokio::runtime::Runtime::new()?;
                sync_pending_uploads(&rt);
                if let Err(err) = tui::run_tui(&rt) {
                    println!(
                        "{} {}",
                        "Could not start Terminal UI:".bright_red(),
                        err.to_string().bright_red()
                    );
                    println!(
                        "{}",
                        "Falling back to the classic guided flow...".bright_yellow()
                    );
                    run_guided_flow()?;
                }
            } else {
                run_guided_flow()?;
            }
        }
        Some(Commands::Ui { port, no_open }) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::start_server(port, !no_open))?;
        }
        Some(Commands::App { port, no_open }) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::start_server(port, !no_open))?;
        }
        Some(Commands::Detect) => {
            let system_info = SystemInfo::detect()?;
            println!("{}", system_info.display());
        }
        Some(Commands::Submit {
            game,
            resolution,
            fps,
            preset,
            fps_1_low,
            ray_tracing,
            upscaling,
        }) => {
            ensure_submission_consent()?;
            let system_info = SystemInfo::detect()?;
            let submission = BenchmarkSubmission::new(
                system_info,
                game,
                resolution,
                preset,
                fps,
                fps_1_low,
                ray_tracing,
                upscaling,
            );

            // Validate
            if let Err(errors) = submission.validate() {
                println!("{}", "Validation errors:".bright_red());
                for e in errors {
                    println!("  - {}", e);
                }
                return Ok(());
            }

            let rt = tokio::runtime::Runtime::new()?;
            sync_pending_uploads(&rt);

            println!("{}", "Submitting benchmark...".bright_cyan());
            match submit_with_offline_fallback(&rt, &submission)? {
                SubmissionOutcome::Uploaded(response) => {
                    print_submission_receipt(&response);
                }
                SubmissionOutcome::SavedOffline { reason } => {
                    println!(
                        "{} {}",
                        "✗ Failed to submit:".bright_red(),
                        reason.bright_red()
                    );
                    println!(
                        "{}",
                        "Benchmark saved locally for automatic retry.".bright_yellow()
                    );
                    let _ = feedback::cli::offer_feedback_prompt(
                        &rt,
                        FeedbackCategory::SubmissionSync,
                        "upload_failed",
                        "Submission failed in terminal quick submit. What happened:\n- What you tried:\n- What you expected:\n- What happened instead:\n",
                    );
                }
            }
        }
        Some(Commands::Games) => {
            print_games_list();
        }
        Some(Commands::Game { name }) => {
            if let Some(game) = GameInfo::find(&name) {
                print_game_details(game);
            } else {
                println!(
                    "{} '{}' not found in database.",
                    "Game".bright_red(),
                    name.bright_yellow()
                );
                println!(
                    "{}",
                    "You can still submit benchmarks for any game!".bright_cyan()
                );
            }
        }
        Some(Commands::Import { file, auto }) => {
            run_import_flow(file, auto)?;
        }
        Some(Commands::Build { command }) => {
            run_build_command(command)?;
        }
        Some(Commands::Benchmark { command }) => {
            run_benchmark_command(command)?;
        }
        Some(Commands::Config) => {
            show_config_info()?;
        }
        Some(Commands::InstallInfo) => {
            show_install_info();
        }
        Some(Commands::Doctor {
            fix,
            yes,
            windows_runtime,
        }) => {
            run_dependency_doctor(fix, yes, windows_runtime)?;
        }
        Some(Commands::Tui) => {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                println!(
                    "{}",
                    "The terminal UI requires an interactive TTY. Try `fps-tracker ui` (Web UI) or run from a real terminal."
                        .bright_yellow()
                );
                return Ok(());
            }

            let rt = tokio::runtime::Runtime::new()?;
            sync_pending_uploads(&rt);

            match tui::run_tui(&rt)? {
                tui::TuiExit::Quit => {}
            }
        }
        Some(Commands::Feedback) => {
            let rt = tokio::runtime::Runtime::new()?;
            sync_pending_uploads(&rt);
            feedback::cli::run_feedback_flow(&rt)?;
        }
    }

    Ok(())
}

/// The main guided benchmark flow
fn run_guided_flow() -> Result<()> {
    clear_screen();
    print_welcome();

    // Show TOS first - loops until user agrees or force quits
    if let Err(err) = show_tos_agreement() {
        println!("\n{} {}", "Consent not completed:".bright_yellow(), err);
        println!("{}", "Exiting FPS Tracker...".bright_white());
        return Ok(());
    }

    let rt = tokio::runtime::Runtime::new()?;
    sync_pending_uploads(&rt);

    println!("\nPress ENTER to continue...");
    wait_for_enter();

    // Step 1: Detect hardware
    clear_screen();
    println!("{}", "STEP 1/5: Hardware Detection\n".bright_cyan().bold());
    println!("{}", "Detecting your system hardware...\n".bright_white());

    let system_info = SystemInfo::detect()?;
    println!("{}\n", system_info.display());

    println!("{} [Y/n]: ", "Is this correct?".bright_yellow());
    let confirm = read_line().to_lowercase();
    if confirm == "n" || confirm == "no" {
        println!(
            "\n{}",
            "Please report this issue at github.com/forgemypcgit/FPStracker".bright_red()
        );
        return Ok(());
    }

    // Step 2: Optional synthetic benchmarks
    clear_screen();
    println!(
        "{}",
        "STEP 2/5: Synthetic Benchmarks (Optional)\n"
            .bright_cyan()
            .bold()
    );

    // Show available tools
    let tools = benchmark_runner::check_benchmark_tools();
    println!("{}", "Available benchmark tools:".bright_white());
    for (name, available) in &tools {
        let status = if *available {
            "✓ Available".bright_green()
        } else {
            "✗ Not found".bright_red()
        };
        println!("  • {}: {}", name.bright_cyan(), status);
    }
    println!();

    #[cfg(target_os = "windows")]
    prepare_windows_synthetic_tools(&tools);

    let mut synthetic_results: Option<benchmark_runner::BenchmarkResults> = None;
    let mut synthetic_profile: Option<String> = None;

    // Show warning and menu
    if let Some(bench_type) = show_benchmark_menu() {
        synthetic_profile = Some(bench_type.profile_key().to_string());
        print_benchmark_warning(bench_type);

        print!("{} ", "Start benchmark? [Y/n]:".bright_yellow());
        let _ = io::stdout().flush();
        let confirm = read_line().to_lowercase();

        if confirm != "n" && confirm != "no" {
            #[cfg(target_os = "linux")]
            maybe_offer_linux_synthetic_tool_install();

            match run_benchmarks(bench_type) {
                Ok(results) => {
                    synthetic_results = Some(results);
                    println!(
                        "{}",
                        "Benchmark run completed for this session.".bright_green()
                    );
                }
                Err(e) => {
                    println!(
                        "{} {}",
                        "Benchmark failed:".bright_red(),
                        e.to_string().bright_red()
                    );
                    println!("{}", "Continuing without benchmark data...".bright_yellow());
                }
            }
        }
    }

    println!("\nPress ENTER to continue to game selection...");
    wait_for_enter();

    // Step 3-5: Game submission loop
    let mut submission_count = 0usize;
    let mut queued_games: VecDeque<(String, Option<&'static GameInfo>)> = VecDeque::new();

    loop {
        if queued_games.is_empty() {
            clear_screen();
            println!(
                "{}",
                format!(
                    "STEP 3/5: Select Game(s) (Next submission #{})\n",
                    submission_count + 1
                )
                .bright_cyan()
                .bold()
            );
            print_games_list();

            println!(
                "\n{}",
                "You can queue multiple games in one run.".bright_cyan()
            );
            println!(
                "{}",
                "Examples: 20  OR  20 13  OR  20, 13, Cyberpunk 2077".bright_white()
            );
            print!("\n{} ", "Enter game name(s) or number(s):".bright_yellow());
            let _ = io::stdout().flush();
            let game_input = read_line();

            let (selected_games, warnings) = parse_game_batch_input(&game_input);
            if selected_games.is_empty() {
                println!(
                    "\n{}",
                    "No valid games selected. Please try again.".bright_yellow()
                );
                println!("\n{}", "Press ENTER to continue...".bright_white());
                wait_for_enter();
                continue;
            }

            if !warnings.is_empty() {
                println!("\n{}", "Input warnings:".bright_yellow().bold());
                for warning in warnings {
                    println!("  {} {}", "•".bright_yellow(), warning.bright_white());
                }
            }

            queued_games.extend(selected_games);

            println!("\n{}", "Queued games:".bright_green().bold());
            for (index, (name, _)) in queued_games.iter().enumerate() {
                println!(
                    "  {} {}",
                    format!("{}.", index + 1).bright_cyan(),
                    name.bright_white()
                );
            }
            println!("\n{}", "Press ENTER to continue...".bright_white());
            wait_for_enter();
        }

        let Some((game_name, game_info)) = queued_games.pop_front() else {
            continue;
        };
        submission_count += 1;

        clear_screen();
        println!(
            "{}",
            format!(
                "STEP 3/5: Selected Game (Submission #{})\n",
                submission_count
            )
            .bright_cyan()
            .bold()
        );
        if let Some(info) = game_info {
            println!("{} {}", "Selected:".bright_green(), info.name.bright_cyan());
            if info.has_benchmark {
                println!(
                    "{}",
                    "✓ This game has a built-in benchmark - recommended!".bright_green()
                );
            }
        } else {
            println!(
                "{} '{}'",
                "Selected custom game:".bright_yellow(),
                game_name.bright_cyan()
            );
            println!(
                "{}",
                "Not in database yet - that's okay, we can still collect data.".bright_white()
            );
        }

        // Benchmark guidance
        clear_screen();
        println!("{}", "STEP 4/5: Benchmark Setup\n".bright_cyan().bold());

        print_benchmark_guidance(game_info);

        if !prompt_anti_cheat_capture_consent(game_info) {
            println!(
                "\n{}",
                "Skipping this game because anti-cheat safety confirmation was not provided."
                    .bright_yellow()
            );
            println!("\n{}", "Press ENTER to continue...".bright_white());
            wait_for_enter();
            continue;
        }

        println!(
            "\n{} ",
            "Press ENTER when you're ready to record your results...".bright_yellow()
        );
        wait_for_enter();

        // Enter results (repeat for the same game until submitted/skipped)
        loop {
            clear_screen();
            println!("{}", "STEP 5/5: Enter Your Results\n".bright_cyan().bold());

            println!("{} ", "What resolution did you test at?".bright_white());
            println!("  Common: 1080p, 1440p, 4K");
            println!("  Tip: plain values like 1080 or 1440 are accepted.");
            print!("  {} ", "Enter resolution:".bright_yellow());
            let _ = io::stdout().flush();
            let resolution = read_line();

            println!(
                "\n{} ",
                "What graphics quality preset did you use?".bright_white()
            );
            println!("  Options: Low, Medium, High, Ultra, Custom");
            print!("  {} ", "Enter preset:".bright_yellow());
            let _ = io::stdout().flush();
            let preset = read_line();

            println!("\n{} ", "What was your average FPS?".bright_white());
            println!("  (The average framerate shown by your FPS counter)");
            print!("  {} ", "Enter FPS:".bright_yellow());
            let _ = io::stdout().flush();
            let fps: f64 = read_line().parse().unwrap_or(0.0);

            println!(
                "\n{} ",
                "What was your 1% low FPS? (optional)".bright_white()
            );
            println!("  (This measures stuttering - lower than average FPS)");
            println!(
                "  {} ",
                "Press Enter to skip if you don't know:".bright_yellow()
            );
            let fps_1_low_str = read_line();
            let fps_1_low: Option<f64> = if fps_1_low_str.is_empty() {
                None
            } else {
                fps_1_low_str.parse().ok()
            };

            println!(
                "\n{} ",
                "What was your 0.1% low FPS? (optional)".bright_white()
            );
            println!("  (More sensitive stutter metric - usually lower than 1% low)");
            println!(
                "  {} ",
                "Press Enter to skip if you don't know:".bright_yellow()
            );
            let fps_01_low_str = read_line();
            let fps_01_low: Option<f64> = if fps_01_low_str.is_empty() {
                None
            } else {
                fps_01_low_str.parse().ok()
            };

            let ray_tracing = prompt_ray_tracing(game_info);
            let upscaling = prompt_upscaling_mode(game_info);

            // Review and submit
            clear_screen();
            println!("{}", "Review & Submit\n".bright_cyan().bold());

            let mut submission = BenchmarkSubmission::new(
                system_info.clone(),
                game_name.clone(),
                resolution,
                preset,
                fps,
                fps_1_low,
                ray_tracing,
                upscaling,
            );
            submission.fps_01_low = fps_01_low;
            if let Some(results) = synthetic_results.as_ref() {
                submission.synthetic_cpu_score = results.cpu_score;
                submission.synthetic_gpu_score = results.gpu_score;
                submission.synthetic_ram_score = results.ram_score;
                submission.synthetic_disk_score = results.disk_score;
                submission.synthetic_cpu_source = results.cpu_score_source.clone();
                submission.synthetic_gpu_source = results.gpu_score_source.clone();
                submission.synthetic_ram_source = results.ram_score_source.clone();
                submission.synthetic_disk_source = results.disk_score_source.clone();
                submission.synthetic_profile = synthetic_profile.clone();
                submission.synthetic_suite_version =
                    Some(benchmark_runner::SYNTHETIC_SUITE_VERSION.to_string());
                submission.synthetic_extended = serde_json::to_value(results).ok();
            }

            if submission.synthetic_cpu_score.is_some()
                || submission.synthetic_gpu_score.is_some()
                || submission.synthetic_ram_score.is_some()
                || submission.synthetic_disk_score.is_some()
            {
                println!(
                    "\n{}",
                    "Synthetic Benchmarks (Auto-Detected)".bright_cyan().bold()
                );
                println!(
                    "  {} CPU:  {}",
                    "•".bright_white(),
                    submission
                        .synthetic_cpu_score
                        .map(|v| match submission.synthetic_cpu_source.as_deref() {
                            Some(source) if !source.trim().is_empty() => {
                                format!("{v} ({})", source.trim())
                            }
                            _ => v.to_string(),
                        })
                        .unwrap_or_else(|| "—".to_string())
                );
                println!(
                    "  {} GPU:  {}",
                    "•".bright_white(),
                    submission
                        .synthetic_gpu_score
                        .map(|v| match submission.synthetic_gpu_source.as_deref() {
                            Some(source) if !source.trim().is_empty() => {
                                format!("{v} ({})", source.trim())
                            }
                            _ => v.to_string(),
                        })
                        .unwrap_or_else(|| "—".to_string())
                );
                println!(
                    "  {} RAM:  {}",
                    "•".bright_white(),
                    submission
                        .synthetic_ram_score
                        .map(|v| match submission.synthetic_ram_source.as_deref() {
                            Some(source) if !source.trim().is_empty() => {
                                format!("{v} ({})", source.trim())
                            }
                            _ => v.to_string(),
                        })
                        .unwrap_or_else(|| "—".to_string())
                );
                println!(
                    "  {} SSD:  {}",
                    "•".bright_white(),
                    submission
                        .synthetic_disk_score
                        .map(|v| match submission.synthetic_disk_source.as_deref() {
                            Some(source) if !source.trim().is_empty() => {
                                format!("{v} ({})", source.trim())
                            }
                            _ => v.to_string(),
                        })
                        .unwrap_or_else(|| "—".to_string())
                );

                println!(
                    "\n{} ",
                    "Keep these synthetic scores? [Y/n]:".bright_yellow()
                );
                let keep = read_line().to_lowercase();
                if keep == "n" || keep == "no" {
                    submission.synthetic_cpu_score = None;
                    submission.synthetic_gpu_score = None;
                    submission.synthetic_ram_score = None;
                    submission.synthetic_disk_score = None;
                    submission.synthetic_cpu_source = None;
                    submission.synthetic_gpu_source = None;
                    submission.synthetic_ram_source = None;
                    submission.synthetic_disk_source = None;
                    submission.synthetic_profile = None;
                    submission.synthetic_suite_version = None;
                    submission.synthetic_extended = None;
                } else {
                    println!("{} ", "Edit any synthetic score? [y/N]:".bright_yellow());
                    let edit = read_line().to_lowercase();
                    if edit == "y" || edit == "yes" {
                        // If the user edits values, preserve the numeric data but drop tool provenance.
                        submission.synthetic_cpu_source = None;
                        submission.synthetic_gpu_source = None;
                        submission.synthetic_ram_source = None;
                        submission.synthetic_disk_source = None;
                        submission.synthetic_suite_version = None;
                        submission.synthetic_extended = None;
                        for (label, slot) in [
                            ("CPU", &mut submission.synthetic_cpu_score),
                            ("GPU", &mut submission.synthetic_gpu_score),
                            ("RAM", &mut submission.synthetic_ram_score),
                            ("SSD", &mut submission.synthetic_disk_score),
                        ] {
                            let current = slot
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".to_string());
                            println!(
                                "{} {} (current: {}). Enter to keep, type 'clear' to remove:",
                                "→".bright_cyan(),
                                label.bright_white(),
                                current.bright_white()
                            );
                            let text = read_line();
                            let trimmed = text.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if trimmed.eq_ignore_ascii_case("clear") {
                                *slot = None;
                                continue;
                            }
                            if let Ok(parsed) = trimmed.parse::<u64>() {
                                if parsed > 0 {
                                    *slot = Some(parsed);
                                } else {
                                    *slot = None;
                                }
                            }
                        }
                    }
                }
            }

            println!("{}\n", "Your submission:".bright_white());
            println!("{}", submission.display());

            // Validate
            if let Err(errors) = submission.validate() {
                println!("\n{}", "Validation issues:".bright_red());
                for e in &errors {
                    println!("  {} {}", "•".bright_red(), e);
                }
                println!("\n{}", "Please fix these and try again.".bright_yellow());

                println!("\n{} ", "Try this game again? [Y/n]:".bright_yellow());
                let retry = read_line().to_lowercase();
                if retry == "n" || retry == "no" {
                    println!("\n{}", "Skipping this game.".bright_yellow());
                    break;
                }
                continue;
            }

            println!("\n{}", "Data Privacy:".bright_cyan().bold());
            println!(
                "  {} GPU/CPU names are sent (needed for the model)",
                "•".bright_white()
            );
            println!(
                "  {} NO serial numbers or unique hardware IDs",
                "•".bright_white()
            );
            println!(
                "  {} Benchmark payload does not include IP or location fields",
                "•".bright_white()
            );
            println!(
                "  {} Submitted data is used to improve FPS prediction and build recommendations",
                "•".bright_white()
            );

            println!("\n{} [Y/n]: ", "Submit this benchmark?".bright_yellow());
            let confirm = read_line().to_lowercase();
            if confirm == "n" || confirm == "no" {
                println!("{}", "Cancelled. Your data was not sent.".bright_yellow());
            } else {
                println!("\n{}", "Submitting...".bright_cyan());
                match submit_with_offline_fallback(&rt, &submission)? {
                    SubmissionOutcome::Uploaded(response) => {
                        print_submission_receipt(&response);
                        println!(
                            "\n{}",
                            "Your benchmark helps others make better PC buying decisions."
                                .bright_white()
                        );
                    }
                    SubmissionOutcome::SavedOffline { reason } => {
                        println!(
                            "\n{} {}",
                            "✗ Couldn't reach server:".bright_red(),
                            reason.bright_red()
                        );
                        println!(
                            "{}",
                            "Your benchmark has been saved locally for automatic retry."
                                .bright_yellow()
                        );
                        println!(
                            "{}",
                            "It will be retried automatically on your next run.".bright_white()
                        );
                        let _ = feedback::cli::offer_feedback_prompt(
                            &rt,
                            FeedbackCategory::SubmissionSync,
                            "upload_failed",
                            "Upload failed and was queued locally. If you can, include the error message you saw and whether you're behind a VPN/proxy.\n",
                        );
                    }
                }
            }

            break;
        }

        if !queued_games.is_empty() {
            println!(
                "\n{} {}",
                "Queued games remaining:".bright_cyan(),
                queued_games.len().to_string().bright_white()
            );
            if let Some((next_game, _)) = queued_games.front() {
                println!("{} {}", "Next:".bright_white(), next_game.bright_cyan());
            }
            println!("\n{}", "Press ENTER to continue...".bright_white());
            wait_for_enter();
            continue;
        }

        println!(
            "\n{} [Y/n]: ",
            "Add more game benchmarks?".bright_cyan().bold()
        );
        let another = read_line().to_lowercase();
        if another == "n" || another == "no" {
            println!(
                "\n{}",
                "Thank you for your contributions!".bright_green().bold()
            );
            println!("{}", "Exiting FPS Tracker...".bright_white());
            break;
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn maybe_offer_linux_synthetic_tool_install() {
    let statuses = deps::collect_dependency_statuses();
    let Some(command) = deps::dependency_bulk_install_command(&statuses) else {
        return;
    };

    println!(
        "{}",
        "\nOptional: install synthetic benchmark tools to improve baseline coverage (glmark2/sysbench/fio/stress-ng)."
            .bright_white()
    );
    println!("{} {}", "Command:".bright_cyan(), command.bright_white());
    print!("{} ", "Install now? [y/N]:".bright_yellow());
    let _ = io::stdout().flush();

    if !prompt_yes_no(false, false, false) {
        return;
    }

    let status = std::process::Command::new("sh")
        .args(["-lc", &command])
        .status();
    match status {
        Ok(s) if s.success() => println!("{}", "✓ Install completed.".bright_green()),
        Ok(s) => println!(
            "{} {}",
            "✗ Install failed (exit):".bright_red(),
            s.to_string().bright_red()
        ),
        Err(err) => println!(
            "{} {}",
            "✗ Install failed:".bright_red(),
            err.to_string().bright_red()
        ),
    }
}

#[derive(Clone, Copy)]
struct ConsentPage {
    title: &'static str,
    lines: &'static [&'static str],
}

/// Show terms and consent pages in a paginated flow.
fn show_tos_agreement() -> Result<()> {
    let config = config::Config::load().unwrap_or_default();
    if config.consent.is_complete() {
        return Ok(());
    }

    let pages: &[ConsentPage] = &[
        ConsentPage {
            title: "SECTION 1: What Data Is Collected",
            lines: &[
                "We collect benchmark and hardware data needed for FPS analysis:",
                "- GPU model, VRAM amount, clocks, and driver version",
                "- CPU model, core/thread count, and frequency",
                "- RAM amount/speed/type and OS version",
                "- Game tested, resolution, preset, and FPS metrics",
                "- Optional benchmark settings (ray tracing/upscaling)",
                "",
                "The benchmark payload does NOT include serial numbers, UUIDs,",
                "files on your computer, or direct personal profile fields.",
            ],
        },
        ConsentPage {
            title: "SECTION 2: How Data Is Used",
            lines: &[
                "Your submitted data is used to:",
                "- Improve FPS prediction models",
                "- Improve PC build recommendations",
                "- Produce aggregated performance statistics",
                "- Build products/services that rely on aggregated performance data (commercial use)",
                "",
                "Data is stored in access-controlled systems.",
                "Retention is managed according to project policy and applicable law.",
                "",
                "Note: the submission payload does not include IP/location fields, but",
                "network/infra logs may still contain IP addresses as part of normal operations.",
            ],
        },
        ConsentPage {
            title: "SECTION 3: Your Choices and Rights",
            lines: &[
                "Participation is optional. You can exit now and submit nothing.",
                "Before submission, you can review and cancel each benchmark entry.",
                "",
                "Submissions are collected without accounts or direct identifiers",
                "(for example: name or email).",
                "",
                "Because submissions are not linked to an identity, we may be unable",
                "to locate, correct, or delete a specific submission later.",
                "If you need to report a problem, open an issue at:",
                "github.com/forgemypcgit/FPStracker",
            ],
        },
        ConsentPage {
            title: "SECTION 4: Agreement Summary",
            lines: &[
                "By typing I AGREE, you confirm that:",
                "- You reviewed these sections",
                "- You consent to the benchmark processing described above",
                "- You consent to de-identified benchmark data being used publicly, including commercial use",
                "- Optional benchmark tools (synthetic baselines) run locally only with your approval and are governed by their upstream licenses",
                "- You are 18+ or have required parental/guardian permission",
                "",
                "This in-app text is a plain-language notice and may be updated.",
                "For production legal use, have full terms reviewed by counsel.",
            ],
        },
    ];

    let mut page_index = 0usize;
    loop {
        clear_screen();
        render_consent_page(pages, page_index);

        let input = read_line().to_lowercase();
        match input.as_str() {
            "" => {
                if page_index + 1 < pages.len() {
                    page_index += 1;
                    continue;
                }
                break;
            }
            "b" | "back" => {
                page_index = page_index.saturating_sub(1);
            }
            "q" | "quit" | "exit" => anyhow::bail!("Consent not granted."),
            _ => {
                println!("\n{}", "Unrecognized input.".bright_yellow());
                println!(
                    "{}",
                    "Use ENTER (next), B (back), or Q (quit).".bright_white()
                );
                println!("\n{}", "Press ENTER to continue...".bright_white());
                wait_for_enter();
            }
        }
    }

    loop {
        clear_screen();
        println!("{}", "FINAL CONSENT".bright_cyan().bold());
        println!("{}", "=============".bright_cyan());
        println!(
            "\n{}",
            "Type I AGREE to continue, or Q to exit without submitting any data.".bright_white()
        );
        print!("\n{} ", ">>>".bright_cyan());
        let _ = io::stdout().flush();

        let input = read_line();
        if input.eq_ignore_ascii_case("I AGREE") {
            persist_consent()?;
            return Ok(());
        }
        if matches!(input.to_lowercase().as_str(), "q" | "quit" | "exit") {
            anyhow::bail!("Consent not granted.");
        }

        println!(
            "\n{}",
            "You need to type I AGREE to continue.".bright_yellow()
        );
        println!("{}", "Press ENTER to try again...".bright_white());
        wait_for_enter();
    }
}

fn ensure_submission_consent() -> Result<()> {
    let cfg = config::Config::load().unwrap_or_default();
    if cfg.consent.is_complete() {
        return Ok(());
    }

    if !io::stdin().is_terminal() {
        anyhow::bail!(
            "Consent is required before submission. Run `fps-tracker start` in an interactive terminal to accept consent."
        );
    }

    println!(
        "{}",
        "Consent is required before submission. Launching consent flow...".bright_yellow()
    );
    show_tos_agreement()?;

    let refreshed_cfg = config::Config::load().unwrap_or_default();
    if refreshed_cfg.consent.is_complete() {
        Ok(())
    } else {
        anyhow::bail!("Consent not completed; submission cancelled.")
    }
}

fn persist_consent() -> Result<()> {
    let mut config = config::Config::load().unwrap_or_default();
    config.consent.tos_accepted = true;
    config.consent.consent_public_use = true;
    config.consent.retention_acknowledged = true;
    config.consent.accepted_at_utc = Some(chrono::Utc::now());
    config.save()?;
    Ok(())
}

fn render_consent_page(pages: &[ConsentPage], page_index: usize) {
    let page = pages[page_index];
    println!(
        "{}",
        format!(
            "TERMS OF SERVICE & CONSENT (Page {}/{})",
            page_index + 1,
            pages.len()
        )
        .bright_cyan()
        .bold()
    );
    println!("{}", "====================================".bright_cyan());
    println!("\n{}", page.title.bright_white().bold());
    println!("{}", "-".repeat(page.title.len()).bright_white());

    for line in page.lines {
        if line.is_empty() {
            println!();
        } else {
            println!("{}", line.bright_white());
        }
    }

    println!(
        "\n{}",
        "Controls: ENTER = next, B = back, Q = quit".bright_yellow()
    );
    if page_index + 1 == pages.len() {
        println!(
            "{}",
            "Press ENTER to open final consent confirmation.".bright_green()
        );
    }
}

/// Import benchmark from external tool (CapFrameX/MangoHud)
fn run_import_flow(file: Option<PathBuf>, auto: bool) -> Result<()> {
    clear_screen();
    println!(
        "{}",
        "IMPORT BENCHMARK FROM EXTERNAL TOOL\n".bright_cyan().bold()
    );

    let file_path = if auto {
        println!("{}", "Searching for latest capture file...".bright_white());

        // Try CapFrameX first (Windows)
        if let Some(path) = import::capframex::find_latest_capframex_capture() {
            println!(
                "{} {}",
                "✓ Found CapFrameX capture:".bright_green(),
                path.display().to_string().bright_cyan()
            );
            path
        } else if let Some(path) = import::mangohud::find_latest_mangohud_log() {
            println!(
                "{} {}",
                "✓ Found MangoHud log:".bright_green(),
                path.display().to_string().bright_cyan()
            );
            path
        } else {
            println!(
                "{}",
                "No capture files found in default locations.".bright_red()
            );
            println!("\n{}", "To enable capture:".bright_yellow());
            println!("  Windows (CapFrameX): Install from capframex.com");
            println!("  Linux (MangoHud): Set MANGOHUD_LOG=1 before running game");
            return Ok(());
        }
    } else if let Some(path) = file {
        path
    } else {
        println!(
            "{}",
            "Please provide a file path with --file, or use --auto to detect.".bright_yellow()
        );
        return Ok(());
    };

    println!("\n{}", "Parsing capture file...".bright_white());

    let capture_format = detect_capture_format(&file_path)?;
    let frame_data = match capture_format {
        CaptureFormat::CapFrameX => {
            parse_capframex_csv(&file_path).or_else(|_| parse_mangohud_log(&file_path))?
        }
        CaptureFormat::MangoHud => {
            parse_mangohud_log(&file_path).or_else(|_| parse_capframex_csv(&file_path))?
        }
    };

    let result = frame_data
        .calculate_stats()
        .ok_or_else(|| anyhow::anyhow!("Failed to calculate statistics from capture"))?;

    println!("\n{}", result);

    let game_name = result.application.clone().unwrap_or_else(|| {
        println!(
            "\n{}",
            "Game name not detected in capture file.".bright_yellow()
        );
        print!("{} ", "Enter game name:".bright_cyan());
        let _ = io::stdout().flush();
        read_line()
    });

    println!("\n{}", "Detecting hardware...".bright_white());
    let system_info = SystemInfo::detect()?;
    println!("{}\n", system_info.display());

    print!("{} ", "Resolution (1080p/1440p/4K):".bright_yellow());
    let _ = io::stdout().flush();
    let resolution = read_line();

    print!(
        "{} ",
        "Graphics Preset (Low/Medium/High/Ultra):".bright_yellow()
    );
    let _ = io::stdout().flush();
    let preset = read_line();

    print!("{} ", "Ray Tracing enabled? [y/N]:".bright_yellow());
    let _ = io::stdout().flush();
    let rt_input = read_line().to_lowercase();
    let ray_tracing = rt_input == "y" || rt_input == "yes";

    print!(
        "{} ",
        "Upscaling (DLSS/FSR mode, or press Enter for none):".bright_yellow()
    );
    let _ = io::stdout().flush();
    let upscaling_str = read_line();
    let upscaling = if upscaling_str.is_empty() {
        None
    } else {
        Some(upscaling_str)
    };

    let mut submission = BenchmarkSubmission::new(
        system_info,
        game_name,
        resolution,
        preset,
        result.avg_fps,
        Some(result.fps_1_low),
        ray_tracing,
        upscaling,
    );
    submission.fps_01_low = result.fps_01_low;
    submission.duration_secs = Some(result.duration_secs);
    submission.sample_count = Some(result.frame_count as u32);
    submission.benchmark_tool = Some(result.source.clone());
    submission.capture_method = Some("external_tool".to_string());

    println!("\n{}\n", "Review your submission:".bright_white());
    println!("{}", submission.display());

    if let Err(errors) = submission.validate() {
        println!("\n{}", "Validation issues:".bright_red());
        for e in &errors {
            println!("  {} {}", "•".bright_red(), e);
        }
        println!("\n{}", "Please fix these and try again.".bright_yellow());
        return Ok(());
    }

    println!("\n{} [Y/n]: ", "Submit this benchmark?".bright_yellow());
    let confirm = read_line().to_lowercase();
    if confirm == "n" || confirm == "no" {
        println!("{}", "Cancelled. Your data was not sent.".bright_yellow());
        return Ok(());
    }
    ensure_submission_consent()?;

    let rt = tokio::runtime::Runtime::new()?;
    sync_pending_uploads(&rt);

    println!("\n{}", "Submitting...".bright_cyan());
    match submit_with_offline_fallback(&rt, &submission)? {
        SubmissionOutcome::Uploaded(response) => {
            print_submission_receipt(&response);
            println!(
                "\n{}",
                "Your benchmark helps others make better PC buying decisions.".bright_white()
            );
        }
        SubmissionOutcome::SavedOffline { reason } => {
            println!(
                "\n{} {}",
                "✗ Couldn't reach server:".bright_red(),
                reason.bright_red()
            );
            println!(
                "{}",
                "Your benchmark has been saved locally for automatic retry.".bright_yellow()
            );
            let _ = feedback::cli::offer_feedback_prompt(
                &rt,
                FeedbackCategory::SubmissionSync,
                "upload_failed",
                "Submission failed and was queued locally. If you can, include whether this happened on Wi‑Fi or Ethernet, and if it repeats.\n",
            );
        }
    }

    Ok(())
}

fn print_welcome() {
    let banner = r#"
╔═══════════════════════════════════════════════════════════════════════════╗
║                                                                           ║
║   ███████╗██████╗ ███████╗  ████████╗██████╗  █████╗  ██████╗██╗  ██╗    ║
║   ██╔════╝██╔══██╗██╔════╝  ╚══██╔══╝██╔══██╗██╔══██╗██╔════╝██║ ██╔╝    ║
║   █████╗  ██████╔╝███████╗     ██║   ██████╔╝███████║██║     █████╔╝     ║
║   ██╔══╝  ██╔═══╝ ╚════██║     ██║   ██╔══██╗██╔══██║██║     ██╔═██╗     ║
║   ██║     ██║     ███████║     ██║   ██║  ██║██║  ██║╚██████╗██║  ██╗    ║
║   ╚═╝     ╚═╝     ╚══════╝     ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝    ║
║                                                                           ║
║                        PC Hardware & FPS Tracker                          ║
╚═══════════════════════════════════════════════════════════════════════════╝
"#;

    for line in banner.lines() {
        println!("{}", line.bright_cyan());
    }

    println!(
        "{}",
        "Welcome! This tool detects your PC hardware and helps you record gaming".bright_white()
    );
    println!("{}", "performance data.\n".bright_white());

    println!("{}", "HOW IT WORKS:".bright_yellow().bold());
    println!(
        "  {} {}",
        "1.".bright_cyan(),
        "We detect your PC hardware (GPU, CPU, RAM, specs)".bright_white()
    );
    println!(
        "  {} {}",
        "2.".bright_cyan(),
        "You play a game and check your FPS (Steam/NVIDIA/AMD overlay)".bright_white()
    );
    println!(
        "  {} {}",
        "3.".bright_cyan(),
        "You enter the game, settings, and FPS you achieved".bright_white()
    );
    println!(
        "  {} {}",
        "4.".bright_cyan(),
        "Data is collected for analysis".bright_white()
    );

    println!(
        "\n{}",
        "IMPORTANT FOR ACCURATE DATA:".bright_yellow().bold()
    );
    println!(
        "  {} {}",
        "•".bright_green(),
        "Disable V-Sync in game settings (uncap your FPS)".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_green(),
        "Close background apps (Chrome, Discord, etc.)".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_green(),
        "Play for 2-3 minutes to warm up the GPU".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_green(),
        "Use the game's built-in benchmark if available".bright_white()
    );

    println!("\n{}", "WHAT WE COLLECT:".bright_green().bold());
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "GPU model, VRAM, clocks".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "CPU model, cores, frequency".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "RAM amount, model, speed".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "Game name, settings, FPS results".bright_white()
    );

    println!("\n{}", "WHAT WE DON'T ASK FOR:".bright_red().bold());
    println!(
        "  {} {}",
        "•".bright_red(),
        "Serial numbers or unique hardware IDs".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_red(),
        "Your precise location".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_red(),
        "Personal information".bright_white()
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AntiCheatRiskLevel {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for AntiCheatRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AntiCheatRiskLevel::Low => write!(f, "Low"),
            AntiCheatRiskLevel::Medium => write!(f, "Medium"),
            AntiCheatRiskLevel::High => write!(f, "High"),
        }
    }
}

fn anti_cheat_risk_for_game_name(game_name: &str) -> AntiCheatRiskLevel {
    let Some(game) = GameInfo::find(game_name) else {
        return AntiCheatRiskLevel::Medium;
    };

    match game.name {
        // Games with strict anti-cheat stacks where tooling compatibility changes often.
        "Valorant" | "League of Legends" => AntiCheatRiskLevel::High,
        // Popular competitive titles where capture overlays can occasionally conflict.
        "Counter-Strike 2" | "Fortnite" | "Apex Legends" | "Call of Duty: Warzone" => {
            AntiCheatRiskLevel::Medium
        }
        _ => AntiCheatRiskLevel::Low,
    }
}

fn anti_cheat_risk_for_game(game_info: Option<&'static GameInfo>) -> AntiCheatRiskLevel {
    game_info
        .map(|game| anti_cheat_risk_for_game_name(game.name))
        .unwrap_or(AntiCheatRiskLevel::Medium)
}

fn print_anti_cheat_guidance(game_info: Option<&'static GameInfo>) {
    let risk = anti_cheat_risk_for_game(game_info);

    println!("\n{}", "ANTI-CHEAT SAFETY:".bright_yellow().bold());
    match (game_info, risk) {
        (Some(game), AntiCheatRiskLevel::High) => {
            println!(
                "{}",
                format!(
                    "  ⚠ {} is marked HIGH risk for third-party capture tools.",
                    game.name
                )
                .bright_red()
                .bold()
            );
            println!(
                "{}",
                "  Use in-game FPS counter or built-in benchmark only for safest operation."
                    .bright_white()
            );
            println!(
                "{}",
                "  Avoid injected overlays/capture hooks unless you accept account risk."
                    .bright_white()
            );
        }
        (Some(game), AntiCheatRiskLevel::Medium) => {
            println!(
                "{}",
                format!(
                    "  ⚠ {} has MEDIUM risk with some third-party overlays.",
                    game.name
                )
                .bright_yellow()
            );
            println!(
                "{}",
                "  Prefer built-in counters/benchmark scenes. If you use external capture, test carefully."
                    .bright_white()
            );
        }
        (Some(_), AntiCheatRiskLevel::Low) => {
            println!(
                "{}",
                "  ✓ Low known anti-cheat risk for external frame capture.".bright_green()
            );
            println!(
                "{}",
                "  Still prefer trusted tools and avoid anything that injects into protected processes."
                    .bright_white()
            );
        }
        (None, _) => {
            println!(
                "{}",
                "  Unknown game: treat as MEDIUM risk by default.".bright_yellow()
            );
            println!(
                "{}",
                "  Use the game's own FPS counter first if anti-cheat policy is unclear."
                    .bright_white()
            );
        }
    }
}

fn guard_live_capture_safety(game_name: Option<&str>, allow_anti_cheat_risk: bool) -> Result<()> {
    let Some(name) = game_name else {
        if allow_anti_cheat_risk {
            println!(
                "{}",
                "⚠ No --game provided; anti-cheat pre-check is bypassed with --allow-anti-cheat-risk."
                    .bright_yellow()
            );
            println!(
                "{}",
                "Proceed only if you are sure your game allows external frame capture."
                    .bright_white()
            );
            return Ok(());
        }

        anyhow::bail!(
            "Live capture is blocked when --game is omitted because anti-cheat safety cannot be checked.\nRe-run with --game \"<name>\" for safe pre-checking, or pass --allow-anti-cheat-risk if you explicitly accept risk."
        );
    };

    match anti_cheat_risk_for_game_name(name) {
        AntiCheatRiskLevel::High if !allow_anti_cheat_risk => {
            anyhow::bail!(
                "Live capture is blocked for '{}' due to high anti-cheat risk.\nUse in-game FPS counters/manual mode instead.\nIf you accept risk, re-run with --allow-anti-cheat-risk.",
                name
            );
        }
        AntiCheatRiskLevel::High => {
            println!(
                "{}",
                format!(
                    "⚠ Proceeding with HIGH anti-cheat-risk capture for '{}' (--allow-anti-cheat-risk enabled).",
                    name
                )
                .bright_red()
                .bold()
            );
        }
        AntiCheatRiskLevel::Medium => {
            println!(
                "{}",
                format!(
                    "⚠ '{}' has medium anti-cheat risk for third-party capture tools.",
                    name
                )
                .bright_yellow()
            );
            println!(
                "{}",
                "Prefer built-in game counters/benchmarks if available.".bright_white()
            );
        }
        AntiCheatRiskLevel::Low => {
            println!(
                "{}",
                format!("Anti-cheat risk check for '{}': low.", name).bright_green()
            );
        }
    }

    Ok(())
}

fn prompt_anti_cheat_capture_consent(game_info: Option<&'static GameInfo>) -> bool {
    match anti_cheat_risk_for_game(game_info) {
        AntiCheatRiskLevel::Low => true,
        AntiCheatRiskLevel::Medium => {
            println!("\n{}", "ANTI-CHEAT CONSENT CHECK".bright_yellow().bold());
            println!(
                "{}",
                "This title is marked MEDIUM anti-cheat risk for third-party capture tools."
                    .bright_yellow()
            );
            println!(
                "{}",
                "Use in-game counters or built-in benchmark when possible.".bright_white()
            );
            println!(
                "{}",
                "Type OK to continue, or press ENTER to skip this game:".bright_white()
            );
            print!("{} ", ">>>".bright_cyan());
            let _ = io::stdout().flush();
            let input = read_line();
            input.eq_ignore_ascii_case("OK")
        }
        AntiCheatRiskLevel::High => {
            println!("\n{}", "STRICT ANTI-CHEAT CONSENT".bright_red().bold());
            if let Some(game) = game_info {
                println!(
                    "{}",
                    format!("{} is marked HIGH anti-cheat risk.", game.name)
                        .bright_red()
                        .bold()
                );
            } else {
                println!(
                    "{}",
                    "This game is treated as HIGH anti-cheat risk.".bright_red()
                );
            }
            println!(
                "{}",
                "Only continue if you will use in-game counter / built-in benchmark (no injection/hooking)."
                    .bright_white()
            );
            println!(
                "{}",
                "Type SAFE MODE to continue, or press ENTER to skip this game:".bright_white()
            );
            print!("{} ", ">>>".bright_cyan());
            let _ = io::stdout().flush();
            read_line().eq_ignore_ascii_case("SAFE MODE")
        }
    }
}

fn print_benchmark_guidance(game_info: Option<&'static GameInfo>) {
    println!("{}", "HOW TO BENCHMARK:\n".bright_yellow().bold());

    if let Some(game) = game_info {
        println!("{} {}", "Game:".bright_cyan(), game.name.bright_white());
        println!(
            "{} {}\n",
            "GPU Demand:".bright_cyan(),
            game.difficulty.to_string().bright_yellow()
        );

        if game.has_benchmark {
            println!(
                "{}",
                "✓ This game has a BUILT-IN BENCHMARK tool!".bright_green()
            );
            println!(
                "{}\n",
                "  Go to: Settings > Graphics > Benchmark".bright_white()
            );
        }

        println!(
            "{} {}",
            "Where to test:".bright_cyan(),
            game.benchmark_notes.bright_white()
        );

        let mut features = Vec::new();
        if game.supports_rt {
            features.push("Ray Tracing");
        }
        if game.supports_dlss {
            features.push("DLSS (NVIDIA cards)");
        }
        if game.supports_fsr {
            features.push("FSR (AMD/Intel cards)");
        }

        if !features.is_empty() {
            println!(
                "\n{}",
                "Optional features this game supports:".bright_cyan()
            );
            for feature in features {
                println!("  {} {}", "•".bright_green(), feature.bright_white());
            }
        }

        let process_suggestions = game.process_name_suggestions();
        if !process_suggestions.is_empty() {
            println!(
                "\n{} {}",
                "Recommended process name(s) for".bright_cyan(),
                "--process-name".bright_white()
            );
            println!(
                "  {} {}",
                "•".bright_green(),
                process_suggestions.join(", ").bright_white()
            );
        }
    } else {
        println!("{}", "Tips for this game:".bright_yellow());
        println!(
            "  {} {}",
            "1.".bright_cyan(),
            "Play in a busy area (cities, battles, many NPCs)".bright_white()
        );
        println!(
            "  {} {}",
            "2.".bright_cyan(),
            "Play for 60-120 seconds".bright_white()
        );
        println!(
            "  {} {}",
            "3.".bright_cyan(),
            "Look at your FPS counter and note the average".bright_white()
        );
    }

    print_anti_cheat_guidance(game_info);

    println!("\n{}", "HOW TO SEE YOUR FPS:".bright_yellow().bold());
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "Steam: Settings → In-Game → FPS Counter".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "NVIDIA: Press Alt+R (GeForce Experience)".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "AMD: Press Ctrl+Shift+O (Radeon Software)".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "In-game: Check Settings → Display/Graphics".bright_white()
    );

    println!("\n{}", "IMPORTANT SETTINGS:".bright_yellow().bold());
    println!(
        "  {} {}",
        "✓".bright_green(),
        "Turn OFF V-Sync (allows FPS above 60/144)".bright_white()
    );
    println!(
        "  {} {}",
        "✓".bright_green(),
        "Turn OFF any FPS limiters".bright_white()
    );
    println!(
        "  {} {}",
        "✓".bright_green(),
        "Close Chrome/Discord before testing".bright_white()
    );
}

fn prompt_ray_tracing(game_info: Option<&'static GameInfo>) -> bool {
    if let Some(game) = game_info {
        if !game.supports_rt {
            println!(
                "\n{}",
                format!(
                    "Ray Tracing is not listed for {}. Recording as Off.",
                    game.name
                )
                .bright_yellow()
            );
            return false;
        }
    }

    println!("\n{} ", "Was Ray Tracing enabled?".bright_white());
    print!("  {} ", "[y/N]:".bright_yellow());
    let _ = io::stdout().flush();
    let rt_input = read_line().to_lowercase();
    rt_input == "y" || rt_input == "yes"
}

fn prompt_upscaling_mode(game_info: Option<&'static GameInfo>) -> Option<String> {
    if let Some(game) = game_info {
        let mut supported = Vec::new();
        if game.supports_dlss {
            supported.push("DLSS");
        }
        if game.supports_fsr {
            supported.push("FSR");
        }

        if supported.is_empty() {
            println!(
                "\n{}",
                format!(
                    "No built-in upscaling support is listed for {}. Leaving empty.",
                    game.name
                )
                .bright_yellow()
            );
            return None;
        }

        println!("\n{} ", "Were you using upscaling?".bright_white());
        println!(
            "  {} {}",
            "Supported in this game:".bright_cyan(),
            supported.join(", ").bright_white()
        );
        println!("  Examples: DLSS Quality, FSR Balanced");
        println!("  {} ", "Press Enter if none:".bright_yellow());
        let mode = read_line();
        return if mode.is_empty() { None } else { Some(mode) };
    }

    println!(
        "\n{} ",
        "Were you using upscaling (DLSS/FSR/XeSS)?".bright_white()
    );
    println!("  Examples: DLSS Quality, DLSS Performance, FSR Balanced");
    println!("  {} ", "Press Enter if none:".bright_yellow());
    let mode = read_line();
    if mode.is_empty() {
        None
    } else {
        Some(mode)
    }
}

fn anti_cheat_list_tag(game: &GameInfo) -> colored::ColoredString {
    match anti_cheat_risk_for_game_name(game.name) {
        AntiCheatRiskLevel::High => "[Strict AC]".bright_red(),
        AntiCheatRiskLevel::Medium => "[AC Caution]".bright_yellow(),
        AntiCheatRiskLevel::Low => "".normal(),
    }
}

fn print_games_list() {
    println!(
        "{}",
        "KNOWN GAMES (sorted by GPU demand):\n"
            .bright_yellow()
            .bold()
    );

    println!("{}", "EXTREME (Most demanding):".bright_red().bold());
    for (i, game) in KNOWN_GAMES.iter().enumerate() {
        if game.difficulty == games::GameDifficulty::Extreme {
            let bench = if game.has_benchmark {
                "[Benchmark]".bright_green()
            } else {
                "".normal()
            };
            let anti_cheat_tag = anti_cheat_list_tag(game);
            println!(
                "  {:2}. {} {} {}",
                i + 1,
                game.name.bright_white(),
                bench,
                anti_cheat_tag
            );
        }
    }

    println!("\n{}", "HEAVY:".bright_magenta().bold());
    for (i, game) in KNOWN_GAMES.iter().enumerate() {
        if game.difficulty == games::GameDifficulty::Heavy {
            let bench = if game.has_benchmark {
                "[Benchmark]".bright_green()
            } else {
                "".normal()
            };
            let anti_cheat_tag = anti_cheat_list_tag(game);
            println!(
                "  {:2}. {} {} {}",
                i + 1,
                game.name.bright_white(),
                bench,
                anti_cheat_tag
            );
        }
    }

    println!("\n{}", "MEDIUM:".bright_yellow().bold());
    for (i, game) in KNOWN_GAMES.iter().enumerate() {
        if game.difficulty == games::GameDifficulty::Medium {
            let bench = if game.has_benchmark {
                "[Benchmark]".bright_green()
            } else {
                "".normal()
            };
            let anti_cheat_tag = anti_cheat_list_tag(game);
            println!(
                "  {:2}. {} {} {}",
                i + 1,
                game.name.bright_white(),
                bench,
                anti_cheat_tag
            );
        }
    }

    println!("\n{}", "LIGHT:".bright_green().bold());
    for (i, game) in KNOWN_GAMES.iter().enumerate() {
        if game.difficulty == games::GameDifficulty::Light {
            let bench = if game.has_benchmark {
                "[Benchmark]".bright_green()
            } else {
                "".normal()
            };
            let anti_cheat_tag = anti_cheat_list_tag(game);
            println!(
                "  {:2}. {} {} {}",
                i + 1,
                game.name.bright_white(),
                bench,
                anti_cheat_tag
            );
        }
    }

    println!(
        "\n{} = Game has built-in benchmark (recommended)",
        "[Benchmark]".bright_green()
    );
    println!(
        "{}",
        "You can also enter any game not on this list.".bright_cyan()
    );
    println!(
        "{}",
        "Tip: enter multiple games with commas (example: 20, 13, Cyberpunk 2077).".bright_cyan()
    );
    println!(
        "{}",
        "Also supported: space-separated game numbers (example: 20 13 25).".bright_cyan()
    );
    println!(
        "{}",
        "Tags: [Strict AC] = high anti-cheat risk, [AC Caution] = medium risk.".bright_cyan()
    );
}

fn print_game_details(game: &'static GameInfo) {
    println!("\n{}", game.name.bright_cyan().bold());
    println!("{}", "=".repeat(game.name.len()).bright_cyan());
    println!(
        "{} {}",
        "Difficulty:".bright_yellow(),
        game.difficulty.to_string().bright_white()
    );
    println!(
        "{} {}",
        "Built-in Benchmark:".bright_yellow(),
        if game.has_benchmark {
            "Yes".bright_green()
        } else {
            "No".bright_red()
        }
    );
    println!(
        "{} {}",
        "Anti-Cheat Capture Risk:".bright_yellow(),
        match anti_cheat_risk_for_game_name(game.name) {
            AntiCheatRiskLevel::High => "High".bright_red(),
            AntiCheatRiskLevel::Medium => "Medium".bright_yellow(),
            AntiCheatRiskLevel::Low => "Low".bright_green(),
        }
    );

    println!("\n{}", "Features:".bright_yellow().bold());
    println!(
        "  {} {}: {}",
        "•".bright_cyan(),
        "Ray Tracing".bright_white(),
        if game.supports_rt {
            "Yes".bright_green()
        } else {
            "No".bright_red()
        }
    );
    println!(
        "  {} {}: {}",
        "•".bright_cyan(),
        "DLSS".bright_white(),
        if game.supports_dlss {
            "Yes".bright_green()
        } else {
            "No".bright_red()
        }
    );
    println!(
        "  {} {}: {}",
        "•".bright_cyan(),
        "FSR".bright_white(),
        if game.supports_fsr {
            "Yes".bright_green()
        } else {
            "No".bright_red()
        }
    );

    println!("\n{}", "Benchmark Notes:".bright_yellow().bold());
    println!("  {}", game.benchmark_notes.bright_white());

    let process_suggestions = game.process_name_suggestions();
    if !process_suggestions.is_empty() {
        println!("\n{}", "Process Name Hints:".bright_yellow().bold());
        println!(
            "  {} {}",
            "--process-name".bright_cyan(),
            process_suggestions.join(", ").bright_white()
        );
    }
}

fn parse_game_input(input: &str) -> (String, Option<&'static GameInfo>) {
    let trimmed = input.trim();

    // Try parsing as number
    if let Ok(num) = trimmed.parse::<usize>() {
        if num > 0 && num <= KNOWN_GAMES.len() {
            let game = &KNOWN_GAMES[num - 1];
            return (game.name.to_string(), Some(game));
        }
    }

    // Try finding by name
    if let Some(game) = GameInfo::find(trimmed) {
        return (game.name.to_string(), Some(game));
    }

    // Return as-is
    (trimmed.to_string(), None)
}

fn parse_game_batch_input(input: &str) -> (Vec<(String, Option<&'static GameInfo>)>, Vec<String>) {
    let mut selected = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();

    let raw = input.trim();
    if raw.is_empty() {
        return (selected, warnings);
    }

    let parts: Vec<&str> = if raw.contains(',') || raw.contains(';') {
        raw.split([',', ';'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        let tokens: Vec<&str> = raw.split_whitespace().collect();
        if tokens.len() > 1 && tokens.iter().all(|token| token.parse::<usize>().is_ok()) {
            tokens
        } else {
            vec![raw]
        }
    };

    if parts.is_empty() {
        return (selected, warnings);
    }

    for part in parts {
        if let Ok(num) = part.parse::<usize>() {
            if num == 0 || num > KNOWN_GAMES.len() {
                warnings.push(format!(
                    "'{}' is out of range (valid game numbers are 1-{}).",
                    part,
                    KNOWN_GAMES.len()
                ));
                continue;
            }
        }

        let parsed = parse_game_input(part);
        if parsed.0.is_empty() {
            continue;
        }

        let dedupe_key = parsed.0.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            warnings.push(format!(
                "'{}' was entered multiple times and will be queued once.",
                parsed.0
            ));
            continue;
        }
        selected.push(parsed);
    }

    (selected, warnings)
}

enum SubmissionOutcome {
    Uploaded(benchmark::SubmissionResponse),
    SavedOffline { reason: String },
}

fn print_submission_receipt(response: &benchmark::SubmissionResponse) {
    use colored::*;

    if response.is_rejected() {
        println!("\n{}", "✗ Submission rejected.".bright_red().bold());
        let reason = response.rejection_summary();
        if !reason.trim().is_empty() {
            println!("{} {}", "Reason:".bright_white(), reason.bright_red());
        }
        if !response.message.trim().is_empty() {
            println!(
                "{} {}",
                "Message:".bright_white(),
                response.message.bright_white()
            );
        }
        return;
    }

    println!(
        "\n{}",
        "✓ Thank you for contributing!".bright_green().bold()
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureFormat {
    CapFrameX,
    MangoHud,
}

fn submit_with_offline_fallback(
    rt: &tokio::runtime::Runtime,
    submission: &BenchmarkSubmission,
) -> Result<SubmissionOutcome> {
    let idempotency_key = idempotency::new_submit_key();
    match rt.block_on(api::submit_benchmark_with_idempotency_key(
        submission,
        &idempotency_key,
    )) {
        Ok(response) => Ok(SubmissionOutcome::Uploaded(response)),
        Err(err) => {
            if api::should_queue_offline(&err) {
                let storage = storage::init_storage()?;
                let pending_id = storage
                    .save_pending_benchmark_with_idempotency_key(submission, &idempotency_key)?;
                let _ = pending_id;
                Ok(SubmissionOutcome::SavedOffline {
                    reason: err.to_string(),
                })
            } else {
                Err(anyhow::anyhow!(err.to_string()))
            }
        }
    }
}

fn sync_pending_uploads(rt: &tokio::runtime::Runtime) {
    sync_pending_benchmarks(rt);
    sync_pending_feedback(rt);
}

fn sync_pending_benchmarks(rt: &tokio::runtime::Runtime) {
    let storage = match storage::init_storage() {
        Ok(storage) => storage,
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not initialize local storage:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    let _sync_lock = match storage.try_acquire_pending_sync_lock() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            println!(
                "{}",
                "Another fps-tracker instance is already syncing pending benchmarks; skipping."
                    .bright_black()
            );
            return;
        }
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not acquire pending sync lock:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    let pending = match storage.load_pending_benchmarks() {
        Ok(pending) => pending,
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not load pending submissions:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    if pending.is_empty() {
        return;
    }

    println!(
        "{} {}",
        "Found pending benchmarks to retry:".bright_cyan(),
        pending.len().to_string().bright_white()
    );

    let mut uploaded = 0usize;
    let mut failed = 0usize;
    let mut dropped_permanent = 0usize;

    for pending_record in pending {
        match rt.block_on(api::submit_benchmark_with_idempotency_key(
            &pending_record.submission,
            &pending_record.idempotency_key,
        )) {
            Ok(_) => {
                let finalized = storage
                    .mark_pending_benchmark_uploaded(&pending_record.id)
                    .or_else(|_| storage.remove_pending_benchmark(&pending_record.id));
                if finalized.is_ok() {
                    uploaded += 1;
                } else {
                    failed += 1;
                }
            }
            Err(err) => {
                if matches!(err, api::ApiError::ConsentRequired(_)) {
                    failed += 1;
                    continue;
                }

                if api::should_queue_offline(&err) {
                    failed += 1;
                    continue;
                }

                match storage.remove_pending_benchmark(&pending_record.id) {
                    Ok(_) => {
                        dropped_permanent += 1;
                    }
                    Err(remove_err) => {
                        failed += 1;
                        println!(
                            "{} {} ({})",
                            "⚠ Could not clear permanently rejected pending submission:"
                                .bright_yellow(),
                            pending_record.id.bright_white(),
                            remove_err.to_string().bright_red()
                        );
                    }
                }
            }
        }
    }

    if uploaded > 0 {
        println!(
            "{} {}",
            "Uploaded pending benchmarks:".bright_green(),
            uploaded.to_string().bright_white()
        );
    }

    if failed > 0 {
        println!(
            "{} {}",
            "Still pending (will retry later):".bright_yellow(),
            failed.to_string().bright_white()
        );
    }
    if dropped_permanent > 0 {
        println!(
            "{} {}",
            "Dropped permanently invalid pending submissions:".bright_yellow(),
            dropped_permanent.to_string().bright_white()
        );
    }
}

fn sync_pending_feedback(rt: &tokio::runtime::Runtime) {
    let storage = match storage::init_storage() {
        Ok(storage) => storage,
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not initialize local storage:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    let _sync_lock = match storage.try_acquire_feedback_sync_lock() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            println!(
                "{}",
                "Another fps-tracker instance is already syncing pending feedback; skipping."
                    .bright_black()
            );
            return;
        }
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not acquire feedback sync lock:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    let pending = match storage.load_pending_feedback() {
        Ok(pending) => pending,
        Err(err) => {
            println!(
                "{} {}",
                "⚠ Could not load pending feedback:".bright_yellow(),
                err.to_string().bright_red()
            );
            return;
        }
    };

    if pending.is_empty() {
        return;
    }

    println!(
        "{} {}",
        "Found pending feedback to retry:".bright_cyan(),
        pending.len().to_string().bright_white()
    );

    let mut uploaded = 0usize;
    let mut failed = 0usize;
    let mut dropped_permanent = 0usize;

    for pending_record in pending {
        match rt.block_on(api::submit_feedback_with_idempotency_key(
            &pending_record.feedback,
            &pending_record.idempotency_key,
        )) {
            Ok(_) => {
                let finalized = storage
                    .mark_pending_feedback_uploaded(&pending_record.id)
                    .or_else(|_| storage.remove_pending_feedback(&pending_record.id));
                if finalized.is_ok() {
                    uploaded += 1;
                } else {
                    failed += 1;
                }
            }
            Err(err) => {
                if api::should_queue_offline_feedback(&err) {
                    failed += 1;
                    continue;
                }

                match storage.remove_pending_feedback(&pending_record.id) {
                    Ok(_) => dropped_permanent += 1,
                    Err(remove_err) => {
                        failed += 1;
                        println!(
                            "{} {} ({})",
                            "⚠ Could not clear permanently rejected pending feedback:"
                                .bright_yellow(),
                            pending_record.id.bright_white(),
                            remove_err.to_string().bright_red()
                        );
                    }
                }
            }
        }
    }

    if uploaded > 0 {
        println!(
            "{} {}",
            "Uploaded pending feedback:".bright_green(),
            uploaded.to_string().bright_white()
        );
    }

    if failed > 0 {
        println!(
            "{} {}",
            "Still pending feedback (will retry later):".bright_yellow(),
            failed.to_string().bright_white()
        );
    }
    if dropped_permanent > 0 {
        println!(
            "{} {}",
            "Dropped permanently invalid pending feedback:".bright_yellow(),
            dropped_permanent.to_string().bright_white()
        );
    }
}

fn detect_capture_format(path: &Path) -> Result<CaptureFormat> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if file_name.contains("capframex") {
        return Ok(CaptureFormat::CapFrameX);
    }
    if file_name.contains("mangohud") {
        return Ok(CaptureFormat::MangoHud);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line_result in reader.lines().take(25) {
        let line = line_result?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if looks_like_capframex_line(trimmed) {
            return Ok(CaptureFormat::CapFrameX);
        }
        if looks_like_mangohud_line(trimmed) {
            return Ok(CaptureFormat::MangoHud);
        }
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("log") => Ok(CaptureFormat::MangoHud),
        Some(ext) if ext.eq_ignore_ascii_case("csv") => Ok(CaptureFormat::CapFrameX),
        _ => anyhow::bail!(
            "Could not detect capture format for {}. Use a CapFrameX CSV or MangoHud log.",
            path.display()
        ),
    }
}

fn looks_like_capframex_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("msbetweenpresents")
        || (lower.contains("application") && lower.contains("timeinseconds"))
}

fn looks_like_mangohud_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("fps,") || lower.contains("frametime")
}

fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
    let _ = io::stdout().flush();
}

fn read_line() -> String {
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or_default();
    input.trim().to_string()
}

fn prompt_yes_no(default_yes: bool, non_interactive_default: bool, assume_yes: bool) -> bool {
    if assume_yes {
        return true;
    }

    if !io::stdin().is_terminal() {
        return non_interactive_default;
    }

    let answer = read_line().to_ascii_lowercase();
    if answer.is_empty() {
        return default_yes;
    }

    match answer.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default_yes,
    }
}

#[cfg(target_os = "windows")]
fn prepare_windows_synthetic_tools(tools: &[(String, bool)]) {
    let winsat_available = tools
        .iter()
        .any(|(name, available)| name.eq_ignore_ascii_case("winsat") && *available);
    let seven_zip_available = tools
        .iter()
        .any(|(name, available)| name.eq_ignore_ascii_case("7z") && *available);

    if !winsat_available {
        println!(
            "{}",
            "WinSAT is missing, so WinSAT-based CPU/RAM/SSD/GPU scoring is unavailable on this Windows install."
                .bright_yellow()
        );
        println!(
            "{}",
            "You can still continue with manual FPS data, or run best-effort baseline tools (7z/DiskSpd/Blender/internal)."
                .bright_white()
        );
        println!();
    }

    if !seven_zip_available {
        println!(
            "{}",
            "Optional install: 7-Zip (open-source, LGPL) for local CPU fallback benchmarking."
                .bright_white()
        );
        println!(
            "{}",
            "This installs via winget and is only used locally for synthetic CPU measurement when needed."
                .bright_white()
        );
        print!("{}", "Install 7-Zip now? [y/N]: ".bright_yellow());
        let _ = io::stdout().flush();

        let allow_install = prompt_yes_no(false, false, false);
        match deps::ensure_7zip_for_session(allow_install) {
            Ok(Some(path)) => {
                println!(
                    "{} {}",
                    "✓ 7z ready:".bright_green(),
                    path.display().to_string().bright_white()
                );
            }
            Ok(None) => println!("{}", "7z install skipped.".bright_yellow()),
            Err(err) => {
                println!("{} {}", "7z install failed:".bright_red(), err);
                println!(
                    "{}",
                    "Run manually: winget install --id 7zip.7zip --exact".bright_white()
                );
            }
        }
        println!();
    }
}

fn wait_for_enter() {
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn should_offer_browser_mode() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn prompt_browser_mode_choice() -> bool {
    println!("{}", "Launch mode".bright_cyan().bold());
    println!(
        "{}",
        "Use Browser Mode for the modern web UI, or continue in terminal mode.".bright_white()
    );
    print!("{} ", "Start Browser Mode now? [y/N]:".bright_yellow());
    let _ = io::stdout().flush();
    let input = read_line();
    input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes")
}

fn launch_browser_mode(port: u16) -> Result<()> {
    println!(
        "{}",
        format!("Starting Browser Mode at http://127.0.0.1:{port}").bright_cyan()
    );
    println!(
        "{}",
        "Your default browser should open automatically.".bright_white()
    );
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(server::start_server(port, true))
}

fn quote_arg(value: &str) -> String {
    if value.contains(' ') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn capture_source_arg(source: CaptureSource) -> &'static str {
    match source {
        CaptureSource::Auto => "auto",
        CaptureSource::MangoHud => "mangohud",
        CaptureSource::PresentMon => "presentmon",
    }
}

#[cfg(any(test, target_os = "windows"))]
fn source_requires_presentmon(source: CaptureSource, has_mangohud_fallback: bool) -> bool {
    match source {
        CaptureSource::PresentMon => true,
        CaptureSource::MangoHud => false,
        CaptureSource::Auto => !has_mangohud_fallback,
    }
}

#[cfg(any(test, target_os = "windows"))]
fn is_existing_mangohud_capture_file(path: &Path) -> bool {
    import::mangohud::looks_like_mangohud_capture_file(path)
}

#[derive(Clone, Copy, Debug)]
struct ProcessNameHint {
    game_name: &'static str,
    primary: &'static str,
    alternatives: &'static [&'static str],
}

fn process_name_hint_for_game(game: Option<&str>) -> Option<ProcessNameHint> {
    let game_name = game?;
    let info = GameInfo::find(game_name)?;
    let suggestions = info.process_name_suggestions();
    let primary = suggestions.first().copied()?;
    let alternatives = if suggestions.len() > 1 {
        &suggestions[1..]
    } else {
        &[]
    };
    Some(ProcessNameHint {
        game_name: info.name,
        primary,
        alternatives,
    })
}

fn print_capture_retry_guidance(err: &anyhow::Error, options: &LiveCaptureOptions) {
    println!(
        "{} {}",
        "Live capture failed:".bright_red().bold(),
        err.to_string().bright_red()
    );

    let err_text = err.to_string().to_ascii_lowercase();
    if err_text.contains("strict focus policy")
        || err_text.contains("sustained focus loss")
        || err_text.contains("validation failed")
    {
        let suggested_process = options.process_name.clone().or_else(|| {
            process_name_hint_for_game(options.game_hint.as_deref())
                .map(|hint| hint.primary.to_string())
        });

        let mut cmd = vec![
            "fps-tracker".to_string(),
            "benchmark".to_string(),
            "preview".to_string(),
            "--source".to_string(),
            capture_source_arg(options.source).to_string(),
            "--duration".to_string(),
            options.duration_secs.to_string(),
            "--focus-policy".to_string(),
            "lenient".to_string(),
            "--pause-on-unfocus".to_string(),
            "true".to_string(),
            "--process-validation".to_string(),
            "false".to_string(),
            "--strict-unfocus-grace-ms".to_string(),
            options
                .strict_unfocus_grace_ms
                .saturating_add(500)
                .to_string(),
            "--poll-ms".to_string(),
            options.poll_ms.to_string(),
        ];

        if let Some(game) = options.game_hint.as_deref() {
            cmd.push("--game".to_string());
            cmd.push(quote_arg(game));
        }
        if let Some(process) = suggested_process.as_deref() {
            cmd.push("--process-name".to_string());
            cmd.push(quote_arg(process));
        }
        if let Some(path) = options.file.as_ref().and_then(|p| p.to_str()) {
            cmd.push("--file".to_string());
            cmd.push(quote_arg(path));
        }

        println!("{}", "Suggested retry command:".bright_yellow().bold());
        println!("{}", cmd.join(" ").bright_white());
    }
}

/// Run benchmark subcommands
fn run_benchmark_command(command: BenchmarkCommands) -> Result<()> {
    match command {
        BenchmarkCommands::Preview {
            source,
            duration,
            file,
            game,
            process_name: cli_process_name,
            allow_anti_cheat_risk,
            focus_policy,
            pause_on_unfocus,
            process_validation,
            poll_ms,
            max_frame_time_ms,
            strict_unfocus_grace_ms,
            submit,
            resolution,
            preset,
            ray_tracing,
            upscaling,
        } => {
            guard_live_capture_safety(game.as_deref(), allow_anti_cheat_risk)?;

            #[cfg(target_os = "windows")]
            {
                let will_use_presentmon = match source {
                    BenchmarkSourceArg::Presentmon => true,
                    BenchmarkSourceArg::Mangohud => false,
                    BenchmarkSourceArg::Auto => match file.as_deref() {
                        Some(path) => {
                            !crate::import::mangohud::looks_like_mangohud_capture_file(path)
                        }
                        None => true,
                    },
                };

                if will_use_presentmon && crate::deps::locate_presentmon_executable().is_none() {
                    println!(
                        "{}",
                        "PresentMon is required for Windows live auto-capture, but it is not installed."
                            .bright_yellow()
                    );
                    println!(
                        "{}",
                        "We can install Intel.PresentMon.Console via winget (preferred), or bootstrap a verified fallback if winget is unavailable."
                            .bright_white()
                    );
                    print!("{} ", "Install PresentMon now? [Y/n]:".bright_yellow());
                    let _ = io::stdout().flush();
                    let answer = read_line().trim().to_ascii_lowercase();
                    if answer == "n" || answer == "no" {
                        anyhow::bail!(
                            "PresentMon is missing. Install it with `fps-tracker doctor --fix`, then retry."
                        );
                    }

                    let _path = crate::deps::ensure_presentmon_for_session(true)
                        .context("Failed to install PresentMon")?;
                }
            }

            let cfg = config::Config::load().unwrap_or_default();
            let effective_focus_policy = focus_policy.map(Into::into).unwrap_or_else(|| match cfg
                .capture
                .focus_policy
            {
                config::FocusPolicy::Strict => FocusPolicy::Strict,
                config::FocusPolicy::Lenient => FocusPolicy::Lenient,
            });
            let effective_pause_on_unfocus =
                pause_on_unfocus.unwrap_or(cfg.capture.pause_on_unfocus);
            let mut effective_process_validation =
                process_validation.unwrap_or(cfg.capture.process_validation);
            let effective_poll_ms = poll_ms.unwrap_or(cfg.capture.default_poll_ms);
            let effective_max_frame_time_ms =
                max_frame_time_ms.unwrap_or(cfg.capture.max_frame_time_ms);
            let effective_strict_unfocus_grace_ms =
                strict_unfocus_grace_ms.unwrap_or(cfg.capture.strict_unfocus_grace_ms);

            let resolved_process_name = cli_process_name.or_else(|| {
                process_name_hint_for_game(game.as_deref()).map(|hint| {
                    println!(
                        "{} {} {}",
                        "Auto-selected process hint:".bright_cyan(),
                        hint.primary.bright_white(),
                        format!("(from game '{}')", hint.game_name).bright_black()
                    );
                    if !hint.alternatives.is_empty() {
                        println!(
                            "{} {}",
                            "Alternative process names:".bright_cyan(),
                            hint.alternatives.join(", ").bright_white()
                        );
                    }
                    hint.primary.to_string()
                })
            });

            if effective_process_validation && resolved_process_name.is_none() {
                println!(
                    "{}",
                    "Process validation is enabled but no target process was resolved; proceeding with validation disabled for this run."
                        .bright_yellow()
                );
                effective_process_validation = false;
            }

            let options = LiveCaptureOptions {
                source: source.into(),
                duration_secs: duration,
                file,
                game_hint: game.clone(),
                process_name: resolved_process_name,
                focus_policy: effective_focus_policy,
                pause_on_unfocus: effective_pause_on_unfocus,
                poll_ms: effective_poll_ms,
                process_validation: effective_process_validation,
                max_frame_time_ms: effective_max_frame_time_ms,
                strict_unfocus_grace_ms: effective_strict_unfocus_grace_ms,
            };

            #[cfg(target_os = "windows")]
            {
                let needs_presentmon = matches!(
                    options.source,
                    CaptureSource::Auto | CaptureSource::PresentMon
                ) && source_requires_presentmon(
                    options.source,
                    options
                        .file
                        .as_deref()
                        .map(is_existing_mangohud_capture_file)
                        .unwrap_or(false)
                        || import::mangohud::find_latest_mangohud_log().is_some(),
                );
                let presentmon_ready = if needs_presentmon {
                    match deps::ensure_presentmon_for_session(false) {
                        Ok(Some(_)) => true,
                        Ok(None) => false,
                        Err(err) => {
                            println!(
                                "{} {}",
                                "Could not verify PresentMon availability:".bright_red(),
                                err.to_string().bright_red()
                            );
                            false
                        }
                    }
                } else {
                    true
                };

                if needs_presentmon && !presentmon_ready {
                    println!(
                        "{}",
                        "PresentMon was not found. It is required for Windows live capture."
                            .bright_yellow()
                    );
                    println!(
                        "{}",
                        "Install securely via winget package Intel.PresentMon.Console now? [Y/n]"
                            .bright_white()
                    );
                    let allow_install = prompt_yes_no(true, false, false);
                    match deps::ensure_presentmon_for_session(allow_install) {
                        Ok(Some(path)) => {
                            println!(
                                "{} {}",
                                "✓ PresentMon ready:".bright_green(),
                                path.display().to_string().bright_white()
                            );
                        }
                        Ok(None) => {
                            println!(
                                "{}",
                                "Live capture skipped. Use manual mode or rerun and accept PresentMon install."
                                    .bright_yellow()
                            );
                            println!(
                                "{} {}",
                                "Manual install:".bright_white(),
                                "winget install --id Intel.PresentMon.Console --exact"
                                    .bright_cyan()
                            );
                            println!(
                                "{}",
                                "Then rerun: fps-tracker benchmark preview --source auto ..."
                                    .bright_white()
                            );
                            return Ok(());
                        }
                        Err(err) => {
                            println!(
                                "{} {}",
                                "Could not prepare PresentMon:".bright_red(),
                                err.to_string().bright_red()
                            );
                            println!(
                                "{}",
                                "Use manual mode for this run, then install PresentMon and retry."
                                    .bright_white()
                            );
                            println!(
                                "{} {}",
                                "Manual install:".bright_white(),
                                "winget install --id Intel.PresentMon.Console --exact"
                                    .bright_cyan()
                            );
                            return Ok(());
                        }
                    }

                    let diskspd_available = deps::locate_diskspd_executable().is_some();
                    let blender_available = deps::locate_blender_executable().is_some();

                    if !diskspd_available {
                        println!(
                "{}",
                "Optional install: DiskSpd (Microsoft) for a local SSD throughput baseline."
                    .bright_white()
        );
                        println!(
            "{}",
            "This installs via winget (or a signed fallback) and is only used locally when you opt in."
                .bright_white()
        );
                        print!("{}", "Install DiskSpd now? [y/N]: ".bright_yellow());
                        let _ = io::stdout().flush();

                        let allow_install = prompt_yes_no(false, false, false);
                        match deps::ensure_diskspd_for_session(allow_install) {
                            Ok(Some(path)) => {
                                println!(
                                    "{} {}",
                                    "✓ diskspd ready:".bright_green(),
                                    path.display().to_string().bright_white()
                                );
                            }
                            Ok(None) => println!("{}", "diskspd install skipped.".bright_yellow()),
                            Err(err) => {
                                println!("{} {}", "diskspd install failed:".bright_red(), err);
                                println!(
                                    "{}",
                                    "Run manually: winget install --id Microsoft.DiskSpd --exact"
                                        .bright_white()
                                );
                            }
                        }
                        println!();
                    }

                    if !blender_available {
                        println!(
            "{}",
            "Optional install: Blender (open-source) for a local CPU render baseline."
                .bright_white()
        );
                        println!(
            "{}",
            "Blender is large. Install only if you're comfortable running a short render benchmark."
                .bright_white()
        );
                        print!("{}", "Install Blender now? [y/N]: ".bright_yellow());
                        let _ = io::stdout().flush();

                        let allow_install = prompt_yes_no(false, false, false);
                        match deps::ensure_blender_for_session(allow_install) {
                            Ok(Some(path)) => {
                                println!(
                                    "{} {}",
                                    "✓ blender ready:".bright_green(),
                                    path.display().to_string().bright_white()
                                );
                            }
                            Ok(None) => println!("{}", "blender install skipped.".bright_yellow()),
                            Err(err) => {
                                println!("{} {}", "blender install failed:".bright_red(), err);
                                println!(
                    "{}",
                    "Run manually: winget install --id BlenderFoundation.Blender --exact"
                        .bright_white()
                );
                            }
                        }
                        println!();
                    }
                }
            }

            println!("{}", "Starting live benchmark preview...".bright_cyan());
            println!(
                "{}",
                "Tip: keep your game in the target benchmark scene while capture runs."
                    .bright_white()
            );
            println!(
                "{} {} | {} {} | {} {}ms | {} {}ms",
                "Focus policy:".bright_cyan(),
                options.focus_policy.to_string().bright_white(),
                "Process validation:".bright_cyan(),
                if options.process_validation {
                    "enabled".bright_green()
                } else {
                    "disabled".bright_yellow()
                },
                "Poll:".bright_cyan(),
                options.poll_ms,
                "Strict grace:".bright_cyan(),
                options.strict_unfocus_grace_ms
            );

            let result = match run_live_capture(&options) {
                Ok(value) => value,
                Err(err) => {
                    print_capture_retry_guidance(&err, &options);
                    return Err(err);
                }
            };
            println!("\n{}", result);

            if game.is_none() {
                if let Some(detected_game) = result.game_hint.as_deref() {
                    match anti_cheat_risk_for_game_name(detected_game) {
                        AntiCheatRiskLevel::High => {
                            println!(
                                "{}",
                                format!(
                                    "⚠ Detected game '{}' has HIGH anti-cheat capture risk.",
                                    detected_game
                                )
                                .bright_red()
                                .bold()
                            );
                            println!(
                                "{}",
                                "For future runs, prefer manual in-game FPS counters for this title."
                                    .bright_white()
                            );
                        }
                        AntiCheatRiskLevel::Medium => {
                            println!(
                                "{}",
                                format!(
                                    "⚠ Detected game '{}' has MEDIUM anti-cheat capture risk.",
                                    detected_game
                                )
                                .bright_yellow()
                            );
                        }
                        AntiCheatRiskLevel::Low => {}
                    }
                } else {
                    println!(
                        "{}",
                        "Anti-cheat note: game could not be identified from capture source."
                            .bright_yellow()
                    );
                }
            }

            if submit {
                ensure_submission_consent()?;
                let game_name = if let Some(name) = game.or_else(|| result.game_hint.clone()) {
                    name
                } else {
                    println!(
                        "{}",
                        "Game name is required for submission. Re-run with --game \"<name>\"."
                            .bright_red()
                    );
                    return Ok(());
                };

                let resolution = if let Some(value) = resolution {
                    value
                } else {
                    println!(
                        "{}",
                        "Resolution is required for submission. Re-run with --resolution."
                            .bright_red()
                    );
                    return Ok(());
                };

                let preset = if let Some(value) = preset {
                    value
                } else {
                    println!(
                        "{}",
                        "Preset is required for submission. Re-run with --preset.".bright_red()
                    );
                    return Ok(());
                };

                let mut submission = BenchmarkSubmission::new(
                    SystemInfo::detect()?,
                    game_name,
                    resolution,
                    preset,
                    result.avg_fps,
                    Some(result.fps_1_low),
                    ray_tracing,
                    upscaling,
                );
                submission.fps_01_low = result.fps_01_low;
                submission.duration_secs = Some(result.duration_secs);
                submission.sample_count = Some(result.frame_count as u32);
                submission.benchmark_tool = Some(result.source.clone());
                submission.capture_quality_score = Some(result.capture_quality_score);
                submission.unstable_capture = Some(result.unstable_capture);
                submission.capture_method = Some("external_tool".to_string());
                submission.notes = Some(format!(
                    "Live capture preview: {} (started {})",
                    result.source,
                    result.started_at.to_rfc3339()
                ));

                let rt = tokio::runtime::Runtime::new()?;
                sync_pending_uploads(&rt);
                println!("\n{}", "Submitting captured benchmark...".bright_cyan());
                match submit_with_offline_fallback(&rt, &submission)? {
                    SubmissionOutcome::Uploaded(response) => {
                        print_submission_receipt(&response);
                    }
                    SubmissionOutcome::SavedOffline { reason } => {
                        println!(
                            "{} {}",
                            "⚠ Could not submit right now:".bright_yellow(),
                            reason.bright_red()
                        );
                        println!("{}", "Saved locally for automatic retry.".bright_yellow());
                        let _ = feedback::cli::offer_feedback_prompt(
                            &rt,
                            FeedbackCategory::SubmissionSync,
                            "upload_failed",
                            "Auto-submit from capture preview failed and was queued locally.\nIf possible include:\n- OS\n- Game\n- Capture tool you used\n- Any error message shown\n",
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// Run build subcommands
fn run_build_command(command: BuildCommands) -> Result<()> {
    use chrono::Utc;
    use colored::*;
    use storage::{BuildComponents, BuildConfig, ComponentSpec};

    let storage = storage::init_storage()?;

    match command {
        BuildCommands::Check {
            name,
            format,
            strict,
        } => {
            if name == "current" {
                // Check currently detected hardware
                println!("{}", "Detecting current hardware...".bright_cyan());
                let system_info = SystemInfo::detect()?;

                let build = BuildConfig {
                    name: "Current System".to_string(),
                    created_at: Utc::now(),
                    components: BuildComponents {
                        cpu: Some(
                            ComponentSpec::new(&system_info.cpu.name)
                                .with_brand(&system_info.cpu.vendor)
                                .with_spec("cores", system_info.cpu.cores as i32)
                                .with_spec("threads", system_info.cpu.threads as i32)
                                .with_spec(
                                    "frequency_mhz",
                                    system_info.cpu.frequency_mhz.unwrap_or(0) as i32,
                                ),
                        ),
                        gpu: Some(
                            ComponentSpec::new(&system_info.gpu.name)
                                .with_brand(system_info.gpu.vendor.to_string())
                                .with_spec("vram_mb", system_info.gpu.vram_mb.unwrap_or(0) as i32),
                        ),
                        ram: Some(
                            ComponentSpec::new(format!("{}MB RAM", system_info.ram.usable_mb))
                                .with_spec("usable_mb", system_info.ram.usable_mb as i32)
                                .with_spec(
                                    "type",
                                    system_info.ram.ram_type.as_deref().unwrap_or("Unknown"),
                                )
                                .with_spec(
                                    "speed_mhz",
                                    system_info.ram.speed_mhz.unwrap_or(0) as i32,
                                ),
                        ),
                        ..Default::default()
                    },
                    notes: Some("Auto-detected from current system".to_string()),
                };

                check_build_compatibility(&build, format, strict)?;
            } else {
                // Check saved build
                match storage.load_build(&name) {
                    Ok(build) => {
                        check_build_compatibility(&build, format, strict)?;
                    }
                    Err(_) => {
                        println!(
                            "{} Build '{}' not found.",
                            "Error:".bright_red(),
                            name.bright_yellow()
                        );
                        println!(
                            "{} Run 'fps-tracker build list' to see saved builds.",
                            "Tip:".bright_cyan()
                        );
                    }
                }
            }
        }

        BuildCommands::List => {
            let builds = storage.list_builds()?;

            if builds.is_empty() {
                println!("{}", "No saved builds found.".bright_yellow());
                println!(
                    "{} Use 'fps-tracker build save <name>' to save your current hardware.",
                    "Tip:".bright_cyan()
                );
            } else {
                println!("{}", "Saved Builds:\n".bright_cyan().bold());
                for (i, name) in builds.iter().enumerate() {
                    println!("  {}. {}", i + 1, name.bright_white());
                }
                println!(
                    "\n{} Use 'fps-tracker build check <name>' to check compatibility.",
                    "Tip:".bright_cyan()
                );
            }
        }

        BuildCommands::Save { name, notes } => {
            println!("{}", "Detecting hardware...".bright_cyan());
            let system_info = SystemInfo::detect()?;

            let build = BuildConfig {
                name: name.clone(),
                created_at: Utc::now(),
                components: BuildComponents {
                    cpu: Some(
                        ComponentSpec::new(&system_info.cpu.name)
                            .with_brand(&system_info.cpu.vendor)
                            .with_spec("cores", system_info.cpu.cores as i32)
                            .with_spec("threads", system_info.cpu.threads as i32)
                            .with_spec(
                                "frequency_mhz",
                                system_info.cpu.frequency_mhz.unwrap_or(0) as i32,
                            ),
                    ),
                    gpu: Some(
                        ComponentSpec::new(&system_info.gpu.name)
                            .with_brand(system_info.gpu.vendor.to_string())
                            .with_spec("vram_mb", system_info.gpu.vram_mb.unwrap_or(0) as i32),
                    ),
                    ram: Some(
                        ComponentSpec::new(format!("{}MB RAM", system_info.ram.usable_mb))
                            .with_spec("usable_mb", system_info.ram.usable_mb as i32)
                            .with_spec(
                                "type",
                                system_info.ram.ram_type.as_deref().unwrap_or("Unknown"),
                            )
                            .with_spec("speed_mhz", system_info.ram.speed_mhz.unwrap_or(0) as i32),
                    ),
                    ..Default::default()
                },
                notes,
            };

            storage.save_build(&name, &build)?;
            println!(
                "{} Build '{}' saved successfully!",
                "✓".bright_green(),
                name.bright_cyan()
            );
            println!(
                "{} Run 'fps-tracker build check {}' to validate.",
                "Tip:".bright_cyan(),
                name
            );
        }

        BuildCommands::Delete { name } => match storage.delete_build(&name) {
            Ok(_) => {
                println!(
                    "{} Build '{}' deleted.",
                    "✓".bright_green(),
                    name.bright_cyan()
                );
            }
            Err(_) => {
                println!(
                    "{} Build '{}' not found.",
                    "Error:".bright_red(),
                    name.bright_yellow()
                );
            }
        },
    }

    Ok(())
}

/// Check build compatibility and display results
fn check_build_compatibility(
    build: &storage::BuildConfig,
    format: OutputFormat,
    strict: bool,
) -> Result<()> {
    use colored::*;
    use serde_json::json;

    // Simple compatibility check based on available data
    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    // Check if we have minimal required components
    if build.components.cpu.is_none() {
        issues.push("No CPU detected".to_string());
    }
    if build.components.gpu.is_none() {
        issues.push("No GPU detected".to_string());
    }
    if build.components.ram.is_none() {
        issues.push("No RAM detected".to_string());
    }

    // Check RAM amount
    if let Some(ref ram) = build.components.ram {
        let usable_mb = ram
            .specs
            .get("usable_mb")
            .and_then(|v| v.as_i64())
            .or_else(|| ram.specs.get("total_mb").and_then(|v| v.as_i64()));
        if let Some(mb) = usable_mb {
            let total_gb = mb / 1024;
            if total_gb < 16 {
                warnings.push(format!(
                    "Low RAM: {}GB (16GB recommended for gaming)",
                    total_gb
                ));
            }
        }
    }

    // Check GPU VRAM
    if let Some(ref gpu) = build.components.gpu {
        if let Some(vram_mb) = gpu.specs.get("vram_mb").and_then(|v| v.as_i64()) {
            let vram_gb = vram_mb / 1024;
            if vram_gb < 8 {
                warnings.push(format!(
                    "Low VRAM: {}GB (8GB recommended for modern games)",
                    vram_gb
                ));
            }
        }
    }

    // Output results
    match format {
        OutputFormat::Json => {
            let result = json!({
                "build_name": build.name,
                "is_compatible": issues.is_empty() && (!strict || warnings.is_empty()),
                "issues": issues,
                "warnings": warnings,
                "components": {
                    "cpu": build.components.cpu.as_ref().map(|c| &c.name),
                    "gpu": build.components.gpu.as_ref().map(|c| &c.name),
                    "ram": build.components.ram.as_ref().map(|c| &c.name),
                }
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        OutputFormat::Text => {
            println!(
                "\n{}",
                format!("Build Check: {}", build.name).bright_cyan().bold()
            );
            println!("{}", "=".repeat(50).bright_cyan());

            // Components
            println!("\n{}", "Components:".bright_white().bold());
            if let Some(ref cpu) = build.components.cpu {
                let cores = cpu.specs.get("cores").and_then(|v| v.as_i64()).unwrap_or(0);
                println!(
                    "  {} {} {}",
                    "CPU:".bright_yellow(),
                    cpu.name.bright_white(),
                    format!("({} cores)", cores).bright_black()
                );
            }
            if let Some(ref gpu) = build.components.gpu {
                let vram_mb = gpu
                    .specs
                    .get("vram_mb")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let vram_gb = vram_mb / 1024;
                println!(
                    "  {} {} {}",
                    "GPU:".bright_yellow(),
                    gpu.name.bright_white(),
                    format!("({}GB VRAM)", vram_gb).bright_black()
                );
            }
            if let Some(ref ram) = build.components.ram {
                let speed = ram
                    .specs
                    .get("speed_mhz")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                println!(
                    "  {} {} {}",
                    "RAM:".bright_yellow(),
                    ram.name.bright_white(),
                    format!("({}MHz)", speed).bright_black()
                );
            }

            // Issues
            if !issues.is_empty() {
                println!("\n{}", "❌ BLOCKING ISSUES:".bright_red().bold());
                for issue in &issues {
                    println!("  {} {}", "•".bright_red(), issue.bright_white());
                }
            }

            // Warnings
            if !warnings.is_empty() {
                println!("\n{}", "⚠️  WARNINGS:".bright_yellow().bold());
                for warning in &warnings {
                    println!("  {} {}", "•".bright_yellow(), warning.bright_white());
                }
            }

            // Summary
            if issues.is_empty() && warnings.is_empty() {
                println!("\n{}", "✅ All checks passed!".bright_green().bold());
            } else if issues.is_empty() {
                if strict {
                    println!(
                        "\n{}",
                        "❌ Build has warnings (strict mode)".bright_red().bold()
                    );
                } else {
                    println!(
                        "\n{}",
                        "✅ Build is compatible (with warnings)"
                            .bright_green()
                            .bold()
                    );
                }
            } else {
                println!("\n{}", "❌ Build has blocking issues".bright_red().bold());
            }

            if let Some(ref notes) = build.notes {
                println!("\n{} {}", "Notes:".bright_cyan(), notes.bright_white());
            }
        }
    }

    Ok(())
}

/// Show configuration information
fn show_config_info() -> Result<()> {
    use colored::*;

    println!("{}", "FPS Tracker Configuration\n".bright_cyan().bold());

    // Config file
    match config::get_config_path() {
        Ok(path) => {
            println!("{} {}", "Config file:".bright_yellow(), path.bright_white());
            if std::path::Path::new(&path).exists() {
                println!("  {} {}", "Status:".bright_cyan(), "Exists".bright_green());
            } else {
                println!(
                    "  {} {}",
                    "Status:".bright_cyan(),
                    "Not created yet (will use defaults)".bright_yellow()
                );
            }
        }
        Err(e) => {
            println!(
                "{} Could not determine config path: {}",
                "Error:".bright_red(),
                e
            );
        }
    }

    let cfg = config::Config::load().unwrap_or_default();
    if let Err(err) = config::init_config() {
        println!(
            "  {} {}",
            "Note:".bright_yellow(),
            format!("Could not create config file yet: {}", err).bright_black()
        );
    }

    println!("\n{}", "API settings:".bright_white().bold());
    println!(
        "  {} {}",
        "Base URL:".bright_cyan(),
        cfg.api.base_url.bright_white()
    );
    println!(
        "  {} {}",
        "Timeout:".bright_cyan(),
        format!("{}s", cfg.api.timeout_seconds).bright_white()
    );
    println!(
        "  {} {}",
        "Retry attempts:".bright_cyan(),
        cfg.api.max_retries.to_string().bright_white()
    );
    println!(
        "  {} {}",
        "Verify SSL:".bright_cyan(),
        if cfg.api.verify_ssl {
            "true".bright_green()
        } else {
            "false".bright_yellow()
        }
    );

    println!("\n{}", "Consent:".bright_white().bold());
    if cfg.consent.is_complete() {
        println!(
            "  {} {}",
            "Status:".bright_cyan(),
            "Accepted".bright_green()
        );
        if let Some(at) = cfg.consent.accepted_at_utc {
            println!(
                "  {} {}",
                "Accepted at:".bright_cyan(),
                at.to_rfc3339().bright_white()
            );
        }
    } else {
        println!(
            "  {} {}",
            "Status:".bright_cyan(),
            "Not accepted (submissions will be blocked)".bright_yellow()
        );
        println!(
            "  {} {}",
            "Fix:".bright_cyan(),
            "Run `fps-tracker start` or `fps-tracker ui` to accept consent.".bright_white()
        );
    }

    println!("\n{}", "Capture settings:".bright_white().bold());
    println!(
        "  {} {}",
        "Focus policy:".bright_cyan(),
        match cfg.capture.focus_policy {
            config::FocusPolicy::Strict => "strict".bright_green(),
            config::FocusPolicy::Lenient => "lenient".bright_yellow(),
        }
    );
    println!(
        "  {} {}",
        "Pause on unfocus:".bright_cyan(),
        if cfg.capture.pause_on_unfocus {
            "true".bright_green()
        } else {
            "false".bright_yellow()
        }
    );
    println!(
        "  {} {}",
        "Poll interval:".bright_cyan(),
        format!("{}ms", cfg.capture.default_poll_ms).bright_white()
    );
    println!(
        "  {} {}",
        "Process validation:".bright_cyan(),
        if cfg.capture.process_validation {
            "true".bright_green()
        } else {
            "false".bright_yellow()
        }
    );
    println!(
        "  {} {}",
        "Max frame time:".bright_cyan(),
        format!("{:.1}ms", cfg.capture.max_frame_time_ms).bright_white()
    );
    println!(
        "  {} {}",
        "Strict unfocus grace:".bright_cyan(),
        format!("{}ms", cfg.capture.strict_unfocus_grace_ms).bright_white()
    );

    println!(
        "\n{} {}",
        "Distribution channel:".bright_yellow(),
        cfg.distribution.channel.bright_white()
    );

    // Data directory
    match storage::init_storage() {
        Ok(storage) => {
            println!(
                "\n{} {}",
                "Data directory:".bright_yellow(),
                storage.data_dir().display().to_string().bright_white()
            );

            // Pending benchmarks
            match storage.pending_count() {
                Ok(count) => {
                    println!(
                        "  {} {} pending benchmark{}",
                        "Pending uploads:".bright_cyan(),
                        count.to_string().bright_white(),
                        if count == 1 { "" } else { "s" }
                    );
                }
                Err(e) => {
                    println!("  {} Could not count pending: {}", "Error:".bright_red(), e);
                }
            }

            // Saved builds
            match storage.list_builds() {
                Ok(builds) => {
                    println!(
                        "  {} {} saved build{}",
                        "Saved builds:".bright_cyan(),
                        builds.len().to_string().bright_white(),
                        if builds.len() == 1 { "" } else { "s" }
                    );
                }
                Err(e) => {
                    println!("  {} Could not list builds: {}", "Error:".bright_red(), e);
                }
            }
        }
        Err(e) => {
            println!(
                "\n{} Could not initialize storage: {}",
                "Error:".bright_red(),
                e
            );
        }
    }

    println!("\n{}", "Commands:".bright_white().bold());
    println!(
        "  {} fps-tracker build list      - List saved builds",
        "•".bright_cyan()
    );
    println!(
        "  {} fps-tracker build save NAME - Save current hardware",
        "•".bright_cyan()
    );
    println!(
        "  {} fps-tracker build check     - Check current hardware",
        "•".bright_cyan()
    );
    println!(
        "  {} fps-tracker install-info    - Show install/update guidance",
        "•".bright_cyan()
    );
    println!(
        "  {} fps-tracker doctor          - Check/fix runtime dependencies",
        "•".bright_cyan()
    );
    println!(
        "  {} fps-tracker app             - Start Browser Mode app UI",
        "•".bright_cyan()
    );

    Ok(())
}

fn show_install_info() {
    println!("{}", "FPS Tracker Install Info\n".bright_cyan().bold());

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    println!(
        "{} {} / {}",
        "Detected platform:".bright_yellow(),
        os.bright_white(),
        arch.bright_white()
    );

    println!("\n{}", "Recommended command installs".bright_white().bold());
    println!(
        "  {} Linux/macOS: {}",
        "•".bright_cyan(),
        "curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash".bright_white()
    );
    println!(
        "  {} Windows (PowerShell): {}",
        "•".bright_cyan(),
        "iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex".bright_white()
    );

    println!("\n{}", "Package managers".bright_white().bold());
    println!(
        "  {} Homebrew (macOS/Linux): {}",
        "•".bright_cyan(),
        "brew install --formula https://github.com/forgemypcgit/FPStracker/releases/latest/download/fps-tracker.rb"
            .bright_white()
    );
    println!(
        "  {} winget (Windows): {}",
        "•".bright_cyan(),
        "winget install --id ForgeMyPC.FPSTracker".bright_white()
    );

    println!("\n{}", "Integrity verification".bright_white().bold());
    println!(
        "  {} Installers always verify SHA-256 and verify signatures when available.",
        "•".bright_cyan()
    );
    println!(
        "  {} Linux/macOS opt-out: FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1",
        "•".bright_cyan()
    );
    println!(
        "  {} Windows opt-out: $env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY='1'",
        "•".bright_cyan()
    );
    println!(
        "  {} Require signatures: FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY=1",
        "•".bright_cyan()
    );
    println!(
        "  {} Custom mirror: FPS_TRACKER_BASE_URL=<url> (or $env:FPS_TRACKER_BASE_URL on Windows)",
        "•".bright_cyan()
    );
    println!(
        "  {} Windows PATH opt-out: $env:FPS_TRACKER_SKIP_PATH_UPDATE='1'",
        "•".bright_cyan()
    );

    println!("\n{}", "After install".bright_white().bold());
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker app        # Browser-mode app UI".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker start      # Terminal guided flow".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker benchmark preview --help".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker doctor --fix".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker doctor --fix --yes    # non-interactive auto-approve".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "fps-tracker doctor --windows-runtime    # deep Windows runtime probe".bright_white()
    );

    println!("\n{}", "Uninstall (manual)".bright_white().bold());
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "Delete the fps-tracker binary from your user-local bin directory.".bright_white()
    );
}

fn run_dependency_doctor(fix: bool, assume_yes: bool, windows_runtime: bool) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    let _ = windows_runtime;
    #[cfg(not(target_os = "windows"))]
    let _ = assume_yes;

    println!("{}", "FPS Tracker Dependency Doctor\n".bright_cyan().bold());

    let statuses = deps::collect_dependency_statuses();
    println!(
        "{} {} / {}",
        "Platform:".bright_yellow(),
        std::env::consts::OS.bright_white(),
        std::env::consts::ARCH.bright_white()
    );
    println!();

    for status in &statuses {
        let icon = if status.available {
            "✓".bright_green()
        } else if status.required {
            "✗".bright_red()
        } else {
            "!".bright_yellow()
        };
        let kind = if status.required {
            "required"
        } else {
            "optional"
        };
        println!(
            "{} {} ({}) - {}",
            icon,
            status.name.bright_white(),
            kind.bright_black(),
            status.details.bright_white()
        );
        if !status.available {
            if let Some(hint) = dependency_install_hint(status.name) {
                println!("  {} {}", "Install:".bright_cyan(), hint.bright_white());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if windows_runtime {
            print_windows_runtime_probe("Windows runtime probe", &deps::probe_windows_runtime());
        }

        if fix {
            if !io::stdin().is_terminal() && !assume_yes {
                println!(
                    "\n{}",
                    "Non-interactive mode detected. Re-run with --yes to apply fixes automatically."
                        .bright_yellow()
                );
            }

            let presentmon_missing = statuses
                .iter()
                .any(|item| item.name == "presentmon" && !item.available);
            if presentmon_missing {
                println!(
                    "\n{}",
                    "Install missing required tool 'presentmon' now? [Y/n]".bright_white()
                );
                let allow = prompt_yes_no(true, false, assume_yes);
                match deps::ensure_presentmon_for_session(allow) {
                    Ok(Some(path)) => {
                        println!(
                            "{} {}",
                            "✓ PresentMon ready:".bright_green(),
                            path.display().to_string().bright_white()
                        );
                    }
                    Ok(None) => println!("{}", "PresentMon install skipped.".bright_yellow()),
                    Err(err) => {
                        println!("{} {}", "PresentMon install failed:".bright_red(), err);
                        println!(
                            "{}",
                            "Run manually: winget install --id Intel.PresentMon.Console --exact"
                                .bright_white()
                        );
                    }
                }
            }

            let seven_zip_missing = statuses
                .iter()
                .any(|item| item.name == "7z" && !item.available);
            if seven_zip_missing {
                println!(
                    "\n{}",
                    "Install optional tool '7z' for CPU fallback benchmark now? [y/N]"
                        .bright_white()
                );
                let allow = prompt_yes_no(false, false, assume_yes);
                match deps::ensure_7zip_for_session(allow) {
                    Ok(Some(path)) => {
                        println!(
                            "{} {}",
                            "✓ 7z ready:".bright_green(),
                            path.display().to_string().bright_white()
                        );
                    }
                    Ok(None) => println!("{}", "7z install skipped.".bright_yellow()),
                    Err(err) => {
                        println!("{} {}", "7z install failed:".bright_red(), err);
                        println!(
                            "{}",
                            "Run manually: winget install --id 7zip.7zip --exact".bright_white()
                        );
                    }
                }
            }

            let diskspd_missing = statuses
                .iter()
                .any(|item| item.name == "diskspd" && !item.available);
            if diskspd_missing {
                println!(
                    "\n{}",
                    "Install optional tool 'diskspd' for SSD throughput baseline now? [y/N]"
                        .bright_white()
                );
                let allow = prompt_yes_no(false, false, assume_yes);
                match deps::ensure_diskspd_for_session(allow) {
                    Ok(Some(path)) => {
                        println!(
                            "{} {}",
                            "✓ diskspd ready:".bright_green(),
                            path.display().to_string().bright_white()
                        );
                    }
                    Ok(None) => println!("{}", "diskspd install skipped.".bright_yellow()),
                    Err(err) => {
                        println!("{} {}", "diskspd install failed:".bright_red(), err);
                        println!(
                            "{}",
                            "Run manually: winget install --id Microsoft.DiskSpd --exact"
                                .bright_white()
                        );
                    }
                }
            }

            let blender_missing = statuses
                .iter()
                .any(|item| item.name == "blender" && !item.available);
            if blender_missing {
                println!(
                    "\n{}",
                    "Install optional tool 'blender' for CPU render baseline now? [y/N]"
                        .bright_white()
                );
                let allow = prompt_yes_no(false, false, assume_yes);
                match deps::ensure_blender_for_session(allow) {
                    Ok(Some(path)) => {
                        println!(
                            "{} {}",
                            "✓ blender ready:".bright_green(),
                            path.display().to_string().bright_white()
                        );
                    }
                    Ok(None) => println!("{}", "blender install skipped.".bright_yellow()),
                    Err(err) => {
                        println!("{} {}", "blender install failed:".bright_red(), err);
                        println!(
                            "{}",
                            "Run manually: winget install --id BlenderFoundation.Blender --exact"
                                .bright_white()
                        );
                    }
                }
            }

            let refreshed = deps::collect_dependency_statuses();
            println!("\n{}", "Post-fix status".bright_white().bold());
            for status in &refreshed {
                let icon = if status.available {
                    "✓".bright_green()
                } else if status.required {
                    "✗".bright_red()
                } else {
                    "!".bright_yellow()
                };
                println!(
                    "{} {} - {}",
                    icon,
                    status.name.bright_white(),
                    status.details.bright_white()
                );
            }

            if windows_runtime {
                print_windows_runtime_probe(
                    "Windows runtime probe (after fixes)",
                    &deps::probe_windows_runtime(),
                );
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if fix {
            if let Some(command) = deps::dependency_bulk_install_command(&statuses) {
                println!(
                    "\n{} {}",
                    "Suggested install command:".bright_cyan(),
                    command.bright_white()
                );
                if io::stdin().is_terminal() && io::stdout().is_terminal() {
                    println!(
                        "\n{}",
                        "Run this command now? (May prompt for sudo password) [y/N]".bright_white()
                    );
                    let allow = prompt_yes_no(false, false, assume_yes);
                    if allow {
                        let status = std::process::Command::new("sh")
                            .args(["-lc", &command])
                            .status();
                        match status {
                            Ok(s) if s.success() => {
                                println!("{}", "\n✓ Install command completed.".bright_green());
                            }
                            Ok(s) => {
                                println!(
                                    "{} {}",
                                    "\n✗ Install command failed (exit):".bright_red(),
                                    s.to_string().bright_red()
                                );
                            }
                            Err(err) => {
                                println!(
                                    "{} {}",
                                    "\n✗ Install command failed:".bright_red(),
                                    err.to_string().bright_red()
                                );
                            }
                        }

                        let refreshed = deps::collect_dependency_statuses();
                        println!("\n{}", "Post-fix status".bright_white().bold());
                        for status in &refreshed {
                            let icon = if status.available {
                                "✓".bright_green()
                            } else if status.required {
                                "✗".bright_red()
                            } else {
                                "!".bright_yellow()
                            };
                            println!(
                                "{} {} - {}",
                                icon,
                                status.name.bright_white(),
                                status.details.bright_white()
                            );
                        }
                    } else {
                        println!(
                            "{}",
                            "Skipped. You can run the suggested command manually anytime."
                                .bright_black()
                        );
                    }
                } else {
                    println!(
                        "{}",
                        "Non-interactive mode detected. Run the suggested command manually, then rerun: fps-tracker doctor"
                            .bright_yellow()
                    );
                }
            } else {
                println!(
                    "\n{}",
                    "No known package-manager command for this platform. Install listed tools manually."
                        .bright_yellow()
                );
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn print_windows_runtime_probe(title: &str, probe: &deps::WindowsRuntimeProbe) {
    println!("\n{}", title.bright_white().bold());
    let winget_icon = if probe.winget_available {
        "✓".bright_green()
    } else {
        "!".bright_yellow()
    };
    println!(
        "{} {} - {}",
        winget_icon,
        "winget".bright_white(),
        if probe.winget_available {
            "available".bright_white()
        } else {
            "missing (PresentMon fallback install path will be used)".bright_yellow()
        }
    );

    let path_text = probe
        .presentmon_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "not found".to_string());
    let presentmon_icon = if probe.presentmon_path.is_some() {
        "✓".bright_green()
    } else {
        "✗".bright_red()
    };
    println!(
        "{} {} - {}",
        presentmon_icon,
        "presentmon path".bright_white(),
        path_text.bright_white()
    );

    let help_icon = if probe.presentmon_help_ok {
        "✓".bright_green()
    } else {
        "✗".bright_red()
    };
    println!(
        "{} {} - {}",
        help_icon,
        "presentmon --help".bright_white(),
        probe.presentmon_help_summary.bright_white()
    );
}

fn dependency_install_hint(tool: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        return match tool {
            "presentmon" => Some(
                "winget install --id Intel.PresentMon.Console --exact  (or run: fps-tracker doctor --fix)"
                    .to_string(),
            ),
            "winget" => Some(
                "Install Microsoft App Installer, or run PowerShell: Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe"
                    .to_string(),
            ),
            "7z" => Some("winget install --id 7zip.7zip --exact".to_string()),
            "diskspd" => Some("winget install --id Microsoft.DiskSpd --exact".to_string()),
            "blender" => Some(
                "winget install --id BlenderFoundation.Blender --exact".to_string(),
            ),
            "winsat" => Some("Built into most Windows editions (no package install).".to_string()),
            _ => None,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let package = match tool {
            "glmark2" => "glmark2",
            "sysbench" => "sysbench",
            "fio" => "fio",
            "stress-ng" => "stress-ng",
            _ => return None,
        };

        if deps::is_command_available("apt-get") {
            return Some(format!("sudo apt-get install -y {package}"));
        }
        if deps::is_command_available("dnf") {
            return Some(format!("sudo dnf install -y {package}"));
        }
        if deps::is_command_available("pacman") {
            return Some(format!("sudo pacman -S --needed {package}"));
        }

        return Some(format!(
            "Install '{package}' using your distro package manager"
        ));
    }

    #[cfg(target_os = "macos")]
    {
        return match tool {
            "glmark2" => Some("brew install glmark2   (if available on your tap)".to_string()),
            "sysbench" => Some("brew install sysbench".to_string()),
            "fio" => Some("brew install fio".to_string()),
            "stress-ng" => Some("brew install stress-ng".to_string()),
            _ => None,
        };
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests {
    use super::{
        guard_live_capture_safety, is_existing_mangohud_capture_file, parse_game_batch_input,
        process_name_hint_for_game, source_requires_presentmon, CaptureSource, Cli,
    };
    use crate::games::KNOWN_GAMES;
    use clap::Parser;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn game_index_1_based(game_name: &str) -> usize {
        KNOWN_GAMES
            .iter()
            .position(|g| g.name == game_name)
            .unwrap_or_else(|| panic!("Expected game to exist in KNOWN_GAMES: {game_name}"))
            + 1
    }

    #[test]
    fn batch_input_supports_space_separated_numbers() {
        let a = game_index_1_based("Valorant");
        let b = game_index_1_based("Fortnite");
        let c = game_index_1_based("Overwatch 2");
        let (games, warnings) = parse_game_batch_input(&format!("{a} {b} {c}"));
        assert_eq!(warnings.len(), 0);
        assert_eq!(games.len(), 3);
        assert_eq!(games[0].0, "Valorant");
        assert_eq!(games[1].0, "Fortnite");
        assert_eq!(games[2].0, "Overwatch 2");
    }

    #[test]
    fn batch_input_deduplicates_games() {
        let idx = game_index_1_based("Valorant");
        let (games, warnings) = parse_game_batch_input(&format!("{idx}, Valorant, {idx}"));
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].0, "Valorant");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("queued once")),
            "Expected duplicate warning, got: {warnings:?}"
        );
    }

    #[test]
    fn batch_input_preserves_multi_word_game_names_without_delimiters() {
        let (games, warnings) = parse_game_batch_input("Call of Duty: Warzone");
        assert_eq!(warnings.len(), 0);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].0, "Call of Duty: Warzone");
    }

    #[test]
    fn live_capture_guard_requires_game_when_not_overridden() {
        assert!(guard_live_capture_safety(None, false).is_err());
        assert!(guard_live_capture_safety(None, true).is_ok());
    }

    #[test]
    fn live_capture_guard_blocks_high_risk_without_override() {
        assert!(guard_live_capture_safety(Some("Valorant"), false).is_err());
        assert!(guard_live_capture_safety(Some("Valorant"), true).is_ok());
    }

    #[test]
    fn process_name_hint_resolves_for_known_game() {
        let hint = process_name_hint_for_game(Some("Counter-Strike 2"))
            .expect("Expected process-name hint for known game");
        assert_eq!(hint.primary, "cs2.exe");
        assert_eq!(hint.game_name, "Counter-Strike 2");
    }

    #[test]
    fn process_name_hint_resolves_aliases() {
        let hint =
            process_name_hint_for_game(Some("LoL")).expect("Expected alias to resolve for LoL");
        assert_eq!(hint.primary, "League of Legends.exe");
    }

    #[test]
    fn process_name_hint_returns_none_for_unknown_game() {
        assert!(process_name_hint_for_game(Some("Totally Unknown Game")).is_none());
    }

    #[test]
    fn source_requirement_for_presentmon_matches_auto_fallback_rules() {
        assert!(source_requires_presentmon(CaptureSource::PresentMon, true));
        assert!(!source_requires_presentmon(CaptureSource::MangoHud, false));
        assert!(source_requires_presentmon(CaptureSource::Auto, false));
        assert!(!source_requires_presentmon(CaptureSource::Auto, true));
    }

    #[test]
    fn existing_mangohud_file_is_detected() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "fps,frametime,cpu_load").expect("write header");
        writeln!(file, "120,8.33,40").expect("write row");
        assert!(is_existing_mangohud_capture_file(file.path()));
    }

    #[test]
    fn doctor_yes_flag_requires_fix_flag() {
        assert!(Cli::try_parse_from(["fps-tracker", "doctor", "--yes"]).is_err());
        assert!(Cli::try_parse_from(["fps-tracker", "doctor", "--fix", "--yes"]).is_ok());
    }

    #[test]
    fn doctor_windows_runtime_flag_parses() {
        assert!(Cli::try_parse_from(["fps-tracker", "doctor", "--windows-runtime"]).is_ok());
    }
}
