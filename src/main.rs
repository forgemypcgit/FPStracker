//! FPS Tracker - Guided benchmark submission for PC Builder
//!
//! This is a LIGHTWEIGHT data collection tool that:
//! - Does NOT inject code into games (lower anti-cheat risk than hook-based tools)
//! - Does NOT run during gameplay (no performance impact)
//! - Guides users through benchmark submission
//! - Anonymizes hardware data before submission

mod api;
mod api_routes;
mod benchmark;
mod benchmark_runner;
mod config;
mod games;
mod hardware;
mod import;
mod server;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::benchmark::live::{run_live_capture, CaptureSource, FocusPolicy, LiveCaptureOptions};
use crate::benchmark::BenchmarkSubmission;
use crate::benchmark_runner::{print_benchmark_warning, run_benchmarks, show_benchmark_menu};
use crate::games::{GameInfo, KNOWN_GAMES};
use crate::hardware::SystemInfo;
use crate::import::{parse_capframex_csv, parse_mangohud_log};

const DEFAULT_UI_PORT: u16 = 3000;

/// FPS Tracker - Collect gaming benchmarks for PC Builder
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Start) | None => {
            if should_offer_browser_mode() && prompt_browser_mode_choice() {
                match launch_browser_mode(DEFAULT_UI_PORT) {
                    Ok(()) => return Ok(()),
                    Err(err) => {
                        println!(
                            "{} {}",
                            "Could not start Browser Mode:".bright_red(),
                            err.to_string().bright_red()
                        );
                        println!(
                            "{}",
                            "Falling back to terminal guided flow...".bright_yellow()
                        );
                    }
                }
            }
            run_guided_flow()?;
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
            sync_pending_benchmarks(&rt);

            println!("{}", "Submitting benchmark...".bright_cyan());
            match submit_with_offline_fallback(&rt, &submission)? {
                SubmissionOutcome::Uploaded(response) => {
                    println!(
                        "{} Submission ID: {}",
                        "✓ Success!".bright_green(),
                        format_submission_id(&response).bright_cyan()
                    );
                }
                SubmissionOutcome::SavedOffline { pending_id, reason } => {
                    println!(
                        "{} {}",
                        "✗ Failed to submit:".bright_red(),
                        reason.bright_red()
                    );
                    println!(
                        "{} {}",
                        "Benchmark saved locally with ID:".bright_yellow(),
                        pending_id.bright_cyan()
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
    sync_pending_benchmarks(&rt);

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
    for (name, available) in tools {
        let status = if available {
            "✓ Available".bright_green()
        } else {
            "✗ Not found".bright_red()
        };
        println!("  • {}: {}", name.bright_cyan(), status);
    }
    println!();

    // Show warning and menu
    if let Some(bench_type) = show_benchmark_menu() {
        print_benchmark_warning(bench_type);

        print!("{} ", "Start benchmark? [Y/n]:".bright_yellow());
        let _ = io::stdout().flush();
        let confirm = read_line().to_lowercase();

        if confirm != "n" && confirm != "no" {
            match run_benchmarks(bench_type) {
                Ok(results) => {
                    println!(
                        "{}",
                        "Benchmark results will be included in your submissions.".bright_green()
                    );
                    // Store results for later use
                    let _ = results;
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

            let ray_tracing = prompt_ray_tracing(game_info);
            let upscaling = prompt_upscaling_mode(game_info);

            // Review and submit
            clear_screen();
            println!("{}", "Review & Submit\n".bright_cyan().bold());

            let submission = BenchmarkSubmission::new(
                system_info.clone(),
                game_name.clone(),
                resolution,
                preset,
                fps,
                fps_1_low,
                ray_tracing,
                upscaling,
            );

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
                "  {} All data is used only for FPS prediction and build recommendations",
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
                        println!(
                            "\n{}",
                            "✓ Thank you for contributing!".bright_green().bold()
                        );
                        println!(
                            "{} {}",
                            "Submission ID:".bright_white(),
                            format_submission_id(&response).bright_cyan()
                        );
                        println!(
                            "\n{}",
                            "Your benchmark helps others make better PC buying decisions."
                                .bright_white()
                        );
                    }
                    SubmissionOutcome::SavedOffline { pending_id, reason } => {
                        println!(
                            "\n{} {}",
                            "✗ Couldn't reach server:".bright_red(),
                            reason.bright_red()
                        );
                        println!(
                            "{} {}",
                            "Your benchmark has been saved locally with ID:".bright_yellow(),
                            pending_id.bright_cyan()
                        );
                        println!(
                            "{}",
                            "It will be retried automatically on your next run.".bright_white()
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
                "",
                "Data is stored in access-controlled systems.",
                "Retention target: up to 10 years, then deletion or anonymization",
                "according to project policy and applicable law.",
            ],
        },
        ConsentPage {
            title: "SECTION 3: Your Choices and Rights",
            lines: &[
                "Participation is optional. You can exit now and submit nothing.",
                "Before submission, you can review and cancel each benchmark entry.",
                "",
                "For submitted data, you may request access, correction, or deletion",
                "where required by applicable law.",
                "Contact for requests: open an issue at github.com/forgemypcgit/FPStracker",
            ],
        },
        ConsentPage {
            title: "SECTION 4: Agreement Summary",
            lines: &[
                "By typing I AGREE, you confirm that:",
                "- You reviewed these sections",
                "- You consent to the benchmark processing described above",
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
            "TERMS OF SERVICE & AI CONSENT (Page {}/{})",
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

    let submission = BenchmarkSubmission::new(
        system_info,
        game_name,
        resolution,
        preset,
        result.avg_fps,
        Some(result.fps_1_low),
        ray_tracing,
        upscaling,
    );

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

    let rt = tokio::runtime::Runtime::new()?;
    sync_pending_benchmarks(&rt);

    println!("\n{}", "Submitting...".bright_cyan());
    match submit_with_offline_fallback(&rt, &submission)? {
        SubmissionOutcome::Uploaded(response) => {
            println!(
                "\n{}",
                "✓ Thank you for contributing!".bright_green().bold()
            );
            println!(
                "{} {}",
                "Submission ID:".bright_white(),
                format_submission_id(&response).bright_cyan()
            );
            println!(
                "\n{}",
                "Your benchmark helps others make better PC buying decisions.".bright_white()
            );
        }
        SubmissionOutcome::SavedOffline { pending_id, reason } => {
            println!(
                "\n{} {}",
                "✗ Couldn't reach server:".bright_red(),
                reason.bright_red()
            );
            println!(
                "{} {}",
                "Your benchmark has been saved locally with ID:".bright_yellow(),
                pending_id.bright_cyan()
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

    println!("\n{}", "WHAT WE DON'T COLLECT:".bright_red().bold());
    println!(
        "  {} {}",
        "•".bright_red(),
        "Serial numbers or hardware IDs".bright_white()
    );
    println!(
        "  {} {}",
        "•".bright_red(),
        "Your IP address or location".bright_white()
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
    SavedOffline { pending_id: String, reason: String },
}

fn format_submission_id(response: &benchmark::SubmissionResponse) -> String {
    response
        .effective_id()
        .map(|id| id.to_string())
        .unwrap_or_else(|| "n/a".to_string())
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
    match rt.block_on(api::submit_benchmark(submission)) {
        Ok(response) => Ok(SubmissionOutcome::Uploaded(response)),
        Err(err) => {
            let should_save_offline = matches!(
                err,
                api::ApiError::Network(_)
                    | api::ApiError::Unreachable
                    | api::ApiError::Api {
                        status: 500..=u16::MAX,
                        ..
                    }
            );

            if should_save_offline {
                let storage = storage::init_storage()?;
                let pending_id = storage.save_pending_benchmark(submission)?;
                Ok(SubmissionOutcome::SavedOffline {
                    pending_id,
                    reason: err.to_string(),
                })
            } else {
                Err(anyhow::anyhow!(err.to_string()))
            }
        }
    }
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

    for (id, submission) in pending {
        match rt.block_on(api::submit_benchmark(&submission)) {
            Ok(_) => {
                if storage.remove_pending_benchmark(&id).is_ok() {
                    uploaded += 1;
                } else {
                    failed += 1;
                }
            }
            Err(_) => {
                failed += 1;
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
            process_name,
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
            let effective_process_validation =
                process_validation.unwrap_or(cfg.capture.process_validation);
            let effective_poll_ms = poll_ms.unwrap_or(cfg.capture.default_poll_ms);
            let effective_max_frame_time_ms =
                max_frame_time_ms.unwrap_or(cfg.capture.max_frame_time_ms);
            let effective_strict_unfocus_grace_ms =
                strict_unfocus_grace_ms.unwrap_or(cfg.capture.strict_unfocus_grace_ms);

            let process_name = process_name.or_else(|| {
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

            let options = LiveCaptureOptions {
                source: source.into(),
                duration_secs: duration,
                file,
                game_hint: game.clone(),
                process_name,
                focus_policy: effective_focus_policy,
                pause_on_unfocus: effective_pause_on_unfocus,
                poll_ms: effective_poll_ms,
                process_validation: effective_process_validation,
                max_frame_time_ms: effective_max_frame_time_ms,
                strict_unfocus_grace_ms: effective_strict_unfocus_grace_ms,
            };

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
                    return Ok(());
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
                submission.notes = Some(format!(
                    "Captured via {} live preview at {} | quality={} | unstable={} | focus_pauses={} | dropped_unfocused={} | dropped_ratio={:.3} | spikes={} ({:.3})",
                    result.source,
                    result.started_at.to_rfc3339(),
                    result.capture_quality_score,
                    result.unstable_capture,
                    result.focus_pauses,
                    result.samples_dropped_unfocused,
                    result.dropped_sample_ratio,
                    result.stutter_spike_count,
                    result.stutter_spike_ratio
                ));

                let rt = tokio::runtime::Runtime::new()?;
                sync_pending_benchmarks(&rt);
                println!("\n{}", "Submitting captured benchmark...".bright_cyan());
                match submit_with_offline_fallback(&rt, &submission)? {
                    SubmissionOutcome::Uploaded(response) => {
                        println!(
                            "{} {}",
                            "✓ Live benchmark submitted. Submission ID:".bright_green(),
                            format_submission_id(&response).bright_cyan()
                        );
                    }
                    SubmissionOutcome::SavedOffline { pending_id, reason } => {
                        println!(
                            "{} {}",
                            "⚠ Could not submit right now:".bright_yellow(),
                            reason.bright_red()
                        );
                        println!(
                            "{} {}",
                            "Saved for retry with pending ID:".bright_yellow(),
                            pending_id.bright_cyan()
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
        "winget install --id PCBuilder.FPSTracker".bright_white()
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

    println!("\n{}", "Uninstall (manual)".bright_white().bold());
    println!(
        "  {} {}",
        "•".bright_cyan(),
        "Delete the fps-tracker binary from your user-local bin directory.".bright_white()
    );
}

#[cfg(test)]
mod tests {
    use super::{guard_live_capture_safety, parse_game_batch_input, process_name_hint_for_game};

    #[test]
    fn batch_input_supports_space_separated_numbers() {
        let (games, warnings) = parse_game_batch_input("20 13 25");
        assert_eq!(warnings.len(), 0);
        assert_eq!(games.len(), 3);
        assert_eq!(games[0].0, "Valorant");
        assert_eq!(games[1].0, "Fortnite");
        assert_eq!(games[2].0, "Overwatch 2");
    }

    #[test]
    fn batch_input_deduplicates_games() {
        let (games, warnings) = parse_game_batch_input("20, Valorant, 20");
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
}
