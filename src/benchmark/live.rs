//! Live benchmark capture (preview)
//!
//! This module captures real frametime samples from external tools while the
//! game is running. It does not inject into games, but anti-cheat compatibility
//! still depends on each game's policy and may change over time.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use uuid::Uuid;

use crate::benchmark::focus;
#[cfg(target_os = "windows")]
use crate::deps;
use crate::import;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSource {
    Auto,
    MangoHud,
    PresentMon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    Strict,
    Lenient,
}

impl fmt::Display for FocusPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FocusPolicy::Strict => write!(f, "strict"),
            FocusPolicy::Lenient => write!(f, "lenient"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiveCaptureOptions {
    pub source: CaptureSource,
    pub duration_secs: u64,
    pub file: Option<PathBuf>,
    pub game_hint: Option<String>,
    pub process_name: Option<String>,
    pub focus_policy: FocusPolicy,
    pub pause_on_unfocus: bool,
    pub poll_ms: u64,
    pub process_validation: bool,
    pub max_frame_time_ms: f64,
    pub strict_unfocus_grace_ms: u64,
}

#[derive(Debug, Clone)]
pub struct LiveCaptureResult {
    pub source: String,
    pub capture_path: Option<PathBuf>,
    pub game_hint: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: f64,
    pub frame_count: usize,
    pub avg_fps: f64,
    pub fps_1_low: f64,
    pub fps_01_low: Option<f64>,
    pub min_fps: f64,
    pub max_fps: f64,
    pub target_process: Option<String>,
    pub focus_pauses: u32,
    pub samples_dropped_unfocused: usize,
    pub capture_quality_score: u8,
    pub longest_unfocused_ms: u64,
    pub total_unfocused_ms: u64,
    pub dropped_sample_ratio: f64,
    pub stutter_spike_count: usize,
    pub stutter_spike_ratio: f64,
    pub unstable_capture: bool,
}

impl fmt::Display for LiveCaptureResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Live Benchmark Result")?;
        writeln!(f, "  Source: {}", self.source)?;
        if let Some(path) = &self.capture_path {
            writeln!(f, "  Capture File: {}", path.display())?;
        }
        if let Some(game) = &self.game_hint {
            writeln!(f, "  Game: {}", game)?;
        }
        if let Some(process) = &self.target_process {
            writeln!(f, "  Target Process: {}", process)?;
        }
        writeln!(f, "  Started: {}", self.started_at.to_rfc3339())?;
        writeln!(f, "  Ended: {}", self.ended_at.to_rfc3339())?;
        writeln!(f, "  Duration: {:.1}s", self.duration_secs)?;
        writeln!(f, "  Samples: {}", self.frame_count)?;
        writeln!(f, "  Avg FPS: {:.1}", self.avg_fps)?;
        writeln!(f, "  1% Low: {:.1}", self.fps_1_low)?;
        if let Some(fps_01) = self.fps_01_low {
            writeln!(f, "  0.1% Low: {:.1}", fps_01)?;
        }
        writeln!(
            f,
            "  Min / Max FPS: {:.1} / {:.1}",
            self.min_fps, self.max_fps
        )?;
        writeln!(
            f,
            "  Focus pauses / dropped samples: {} / {}",
            self.focus_pauses, self.samples_dropped_unfocused
        )?;
        writeln!(
            f,
            "  Unfocused (longest/total): {}ms / {}ms",
            self.longest_unfocused_ms, self.total_unfocused_ms
        )?;
        writeln!(f, "  Capture Quality Score: {}", self.capture_quality_score)?;
        writeln!(
            f,
            "  Dropped sample ratio: {:.2}%",
            self.dropped_sample_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Stutter spikes: {} ({:.2}%)",
            self.stutter_spike_count,
            self.stutter_spike_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Unstable capture: {}",
            if self.unstable_capture { "yes" } else { "no" }
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct CaptureDiagnostics {
    target_process: Option<String>,
    focus_pauses: u32,
    samples_dropped_unfocused: usize,
    process_validation_enabled: bool,
    process_mismatch_detected: bool,
    longest_unfocused_ms: u64,
    total_unfocused_ms: u64,
}

pub fn run_live_capture(options: &LiveCaptureOptions) -> Result<LiveCaptureResult> {
    if options.duration_secs < 10 {
        anyhow::bail!("Duration must be at least 10 seconds");
    }
    if options.duration_secs > 900 {
        anyhow::bail!("Duration must be <= 900 seconds");
    }
    if !(50..=500).contains(&options.poll_ms) {
        anyhow::bail!("poll-ms must be between 50 and 500");
    }
    if !(100..=10_000).contains(&options.strict_unfocus_grace_ms) {
        anyhow::bail!("strict-unfocus-grace-ms must be between 100 and 10000");
    }
    if !options.max_frame_time_ms.is_finite() || options.max_frame_time_ms <= 0.0 {
        anyhow::bail!("max-frame-time-ms must be a positive number");
    }

    let source = resolve_source(options)?;
    match source {
        CaptureSource::MangoHud => capture_from_mangohud(options),
        CaptureSource::PresentMon => capture_from_presentmon(options),
        CaptureSource::Auto => unreachable!("resolve_source must not return Auto"),
    }
}

fn resolve_source(options: &LiveCaptureOptions) -> Result<CaptureSource> {
    if options.source != CaptureSource::Auto {
        return Ok(options.source);
    }

    if let Some(path) = options.file.as_deref() {
        if import::mangohud::looks_like_mangohud_capture_file(path) {
            return Ok(CaptureSource::MangoHud);
        }

        #[cfg(target_os = "windows")]
        {
            return Ok(CaptureSource::PresentMon);
        }

        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!(
                "Provided --file is not recognized as a MangoHud capture. \
On Linux/macOS, --source auto requires a MangoHud log path, or omit --file."
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        if deps::locate_presentmon_executable().is_some() {
            return Ok(CaptureSource::PresentMon);
        }
    }

    if import::mangohud::find_latest_mangohud_log().is_some() {
        return Ok(CaptureSource::MangoHud);
    }

    anyhow::bail!(
        "No live capture source found. Start MangoHud logging or use --source presentmon (Windows)."
    )
}

fn capture_from_mangohud(options: &LiveCaptureOptions) -> Result<LiveCaptureResult> {
    let capture_path = if let Some(path) = options.file.clone() {
        path
    } else {
        import::mangohud::find_latest_mangohud_log().ok_or_else(|| {
            anyhow::anyhow!(
                "No MangoHud log found. Enable logging: MANGOHUD=1 MANGOHUD_LOG=1 <game>"
            )
        })?
    };

    if !capture_path.exists() {
        anyhow::bail!("Capture file does not exist: {}", capture_path.display());
    }

    let mut parser = MangoHudStreamParser::default();
    prime_mangohud_parser_with_existing_header(&capture_path, &mut parser)?;
    let mut partial_line = String::new();
    let mut frame_times_ms: VecDeque<f64> = VecDeque::new();
    let mut read_offset = File::open(&capture_path)
        .with_context(|| format!("Failed to open {}", capture_path.display()))?
        .metadata()
        .context("Failed to read capture file metadata")?
        .len();

    let started_at = Utc::now();
    let start_instant = Instant::now();
    let deadline = start_instant + Duration::from_secs(options.duration_secs);
    let mut last_printed_sec = u64::MAX;
    let mut dynamic_poll_ms = options.poll_ms.clamp(50, 500);

    let mut diagnostics = init_capture_diagnostics(options);
    let monitor_focus = monitor_focus_enabled(options, &diagnostics);

    let mut focus_tracker = FocusTracker::new(
        options.process_name.clone(),
        options.focus_policy,
        monitor_focus,
        options.pause_on_unfocus,
        options.strict_unfocus_grace_ms,
    );

    println!(
        "Live capture started ({}) from {}",
        options.duration_secs,
        capture_path.display()
    );
    println!(
        "Run your game benchmark path now. Capturing fresh samples only (adaptive poll, focus-aware)...\n"
    );

    while Instant::now() < deadline {
        let elapsed = start_instant.elapsed().as_secs();
        if elapsed != last_printed_sec {
            let remaining = options.duration_secs.saturating_sub(elapsed);
            print!(
                "\rCapturing... {:>3}s left | samples: {} | dropped: {} | poll: {}ms",
                remaining,
                frame_times_ms.len(),
                diagnostics.samples_dropped_unfocused,
                dynamic_poll_ms
            );
            let _ = std::io::stdout().flush();
            last_printed_sec = elapsed;
        }

        let mut file = File::open(&capture_path)
            .with_context(|| format!("Failed to open {}", capture_path.display()))?;
        file.seek(SeekFrom::Start(read_offset))?;

        let mut chunk = String::new();
        file.read_to_string(&mut chunk)?;
        read_offset = read_offset.saturating_add(chunk.len() as u64);

        let new_samples = parse_chunk_frametimes(&mut parser, &chunk, &mut partial_line);

        let collecting = focus_tracker.should_collect();
        if collecting {
            frame_times_ms.extend(new_samples.iter().copied());
        } else {
            diagnostics.samples_dropped_unfocused = diagnostics
                .samples_dropped_unfocused
                .saturating_add(new_samples.len());
        }

        update_diagnostics_from_focus_tracker(&mut diagnostics, &focus_tracker);

        dynamic_poll_ms = adjust_poll_interval(dynamic_poll_ms, options.poll_ms, new_samples.len());
        thread::sleep(Duration::from_millis(dynamic_poll_ms));
    }
    println!();

    if !partial_line.is_empty() {
        let trailing = partial_line.trim();
        if let Some(ft) = parser.parse_line(trailing) {
            if focus_tracker.should_collect() {
                frame_times_ms.push_back(ft);
            } else {
                diagnostics.samples_dropped_unfocused =
                    diagnostics.samples_dropped_unfocused.saturating_add(1);
            }
        }
    }

    focus_tracker.finalize_unfocused_tracking();
    update_diagnostics_from_focus_tracker(&mut diagnostics, &focus_tracker);

    if diagnostics.process_validation_enabled && diagnostics.process_mismatch_detected {
        anyhow::bail!(
            "Capture process validation failed: active foreground process did not consistently match target '{}'.",
            diagnostics.target_process.as_deref().unwrap_or("(unknown)")
        );
    }
    if focus_tracker.strict_focus_violation() {
        anyhow::bail!(strict_focus_violation_message(
            options.strict_unfocus_grace_ms
        ));
    }

    let ended_at = Utc::now();
    let duration_secs = start_instant.elapsed().as_secs_f64();
    build_result(
        frame_times_ms.into_iter().collect(),
        "MangoHud Live",
        Some(capture_path),
        options.game_hint.clone(),
        started_at,
        ended_at,
        duration_secs,
        options.max_frame_time_ms,
        diagnostics,
    )
}

fn capture_from_presentmon(options: &LiveCaptureOptions) -> Result<LiveCaptureResult> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = options;
        anyhow::bail!("PresentMon capture is only available on Windows");
    }

    #[cfg(target_os = "windows")]
    {
        let presentmon_path = deps::locate_presentmon_executable().ok_or_else(|| {
            anyhow::anyhow!(
                "presentmon is not available. Run `fps-tracker doctor --fix` \
to install Intel.PresentMon.Console or bootstrap a local fallback."
            )
        })?;

        if options.pause_on_unfocus
            && options.focus_policy == FocusPolicy::Strict
            && options.process_name.is_none()
        {
            anyhow::bail!("Strict focus policy requires --process-name for PresentMon capture.");
        }

        let output_path = if let Some(path) = options.file.clone() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "Failed to create PresentMon output directory: {}",
                        parent.display()
                    )
                })?;
            }
            path
        } else {
            let storage = crate::storage::init_storage()
                .context("Failed to initialize local storage for PresentMon capture")?;
            let captures_dir = storage.data_dir().join("captures");
            std::fs::create_dir_all(&captures_dir).with_context(|| {
                format!(
                    "Failed to create PresentMon capture directory: {}",
                    captures_dir.display()
                )
            })?;
            let file_name = format!(
                "presentmon_capture_{}_{}.csv",
                Utc::now().timestamp(),
                Uuid::new_v4().simple()
            );
            captures_dir.join(file_name)
        };

        let mut diagnostics = init_capture_diagnostics(options);
        let monitor_focus = monitor_focus_enabled(options, &diagnostics);

        let started_at = Utc::now();
        let start_instant = Instant::now();

        let mut cmd = Command::new(&presentmon_path);
        cmd.arg("-timed")
            .arg(options.duration_secs.to_string())
            .arg("-output_file")
            .arg(&output_path);

        if let Some(process_name) = options.process_name.as_deref() {
            cmd.args(["-process_name", process_name]);
        }

        let mut child = cmd.spawn().map_err(|err| {
            anyhow::anyhow!(
                "Failed to run presentmon: {err}. \
Ensure PresentMon is installed and runnable (`presentmon --help`). \
If it still fails, repair the Microsoft Visual C++ Redistributable and try again."
            )
        })?;

        let mut focus_tracker = FocusTracker::new(
            options.process_name.clone(),
            options.focus_policy,
            monitor_focus,
            options.pause_on_unfocus,
            options.strict_unfocus_grace_ms,
        );

        while child
            .try_wait()
            .context("Failed waiting for presentmon")?
            .is_none()
        {
            let _ = focus_tracker.should_collect();
            thread::sleep(Duration::from_millis(500));
        }

        let status = child
            .wait()
            .context("Failed to collect PresentMon process status")?;

        focus_tracker.finalize_unfocused_tracking();
        update_diagnostics_from_focus_tracker(&mut diagnostics, &focus_tracker);

        if !status.success() {
            anyhow::bail!(
                "PresentMon failed with status {}. \
Confirm it runs directly (`presentmon --help`), then retry with `fps-tracker doctor --fix` if needed.",
                status
            );
        }

        if options.pause_on_unfocus && diagnostics.focus_pauses > 0 {
            anyhow::bail!(
                "Capture paused due to focus loss while using PresentMon. \
PresentMon output cannot be safely trimmed by focus windows; re-run while keeping the target window focused, \
or set --pause-on-unfocus false."
            );
        }

        if options.focus_policy == FocusPolicy::Strict && focus_tracker.strict_focus_violation() {
            anyhow::bail!(
                "{} Re-run while keeping target window focused, increase --strict-unfocus-grace-ms, or use --focus-policy lenient.",
                strict_focus_violation_message(options.strict_unfocus_grace_ms)
            );
        }

        let frame_data = import::capframex::parse_capframex_csv_for_process(
            &output_path,
            if diagnostics.process_validation_enabled {
                options.process_name.as_deref()
            } else {
                None
            },
        )
        .with_context(|| format!("Failed to parse {}", output_path.display()))?;

        if let (Some(target), Some(observed)) = (
            options.process_name.as_deref(),
            frame_data.application.as_deref(),
        ) {
            if !focus::process_name_matches(observed, target) {
                diagnostics.process_mismatch_detected = true;
            }
        }

        if diagnostics.process_validation_enabled && diagnostics.process_mismatch_detected {
            anyhow::bail!(
                "PresentMon process validation failed: capture did not match target process '{}'.",
                options
                    .process_name
                    .as_deref()
                    .unwrap_or("(unknown target process)")
            );
        }

        let ended_at = Utc::now();
        build_result(
            frame_data.frame_times_ms,
            "PresentMon",
            Some(output_path),
            options.game_hint.clone().or(frame_data.application),
            started_at,
            ended_at,
            start_instant
                .elapsed()
                .as_secs_f64()
                .max(frame_data.duration_secs),
            options.max_frame_time_ms,
            diagnostics,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn build_result(
    frame_times_ms: Vec<f64>,
    source: &str,
    capture_path: Option<PathBuf>,
    game_hint: Option<String>,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    duration_secs: f64,
    max_frame_time_ms: f64,
    diagnostics: CaptureDiagnostics,
) -> Result<LiveCaptureResult> {
    let valid_frame_times: Vec<f64> = frame_times_ms
        .into_iter()
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= max_frame_time_ms)
        .collect();

    if valid_frame_times.len() < 60 {
        anyhow::bail!(
            "Not enough live frame samples captured ({}). Keep benchmark scene active while capture runs.",
            valid_frame_times.len()
        );
    }

    let frame_count = valid_frame_times.len();
    let avg_frame_time_ms = valid_frame_times.iter().sum::<f64>() / frame_count as f64;
    let avg_fps = 1000.0 / avg_frame_time_ms;

    let mut sorted = valid_frame_times.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let idx_99 = ((frame_count as f64 * 0.99) as usize).min(frame_count - 1);
    let p99_ft = sorted[idx_99];
    let fps_1_low = 1000.0 / p99_ft;

    let low_0_1_fps = if frame_count >= 1000 {
        let idx_999 = ((frame_count as f64 * 0.999) as usize).min(frame_count - 1);
        Some(1000.0 / sorted[idx_999])
    } else {
        None
    };

    let fps_values: Vec<f64> = valid_frame_times.iter().map(|ft| 1000.0 / ft).collect();
    let min_fps = fps_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_fps = fps_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let quality_data = import::FrameData {
        frame_times_ms: valid_frame_times.clone(),
        application: game_hint.clone(),
        duration_secs,
        source: source.to_string(),
    };
    let stutter_spike_count = quality_data.stutter_spike_count(max_frame_time_ms);
    let stutter_spike_ratio = quality_data.stutter_spike_ratio(max_frame_time_ms);
    let dropped_sample_ratio =
        compute_dropped_sample_ratio(frame_count, diagnostics.samples_dropped_unfocused);
    let capture_quality_score = compute_capture_quality_score(
        frame_count,
        diagnostics.focus_pauses,
        diagnostics.samples_dropped_unfocused,
        diagnostics.process_mismatch_detected,
        diagnostics.longest_unfocused_ms,
        stutter_spike_ratio,
    );
    let unstable_capture = capture_quality_score < 70
        || dropped_sample_ratio > 0.10
        || stutter_spike_ratio > 0.08
        || diagnostics.longest_unfocused_ms > 5_000;

    Ok(LiveCaptureResult {
        source: source.to_string(),
        capture_path,
        game_hint,
        started_at,
        ended_at,
        duration_secs,
        frame_count,
        avg_fps,
        fps_1_low,
        fps_01_low: low_0_1_fps,
        min_fps,
        max_fps,
        target_process: diagnostics.target_process,
        focus_pauses: diagnostics.focus_pauses,
        samples_dropped_unfocused: diagnostics.samples_dropped_unfocused,
        capture_quality_score,
        longest_unfocused_ms: diagnostics.longest_unfocused_ms,
        total_unfocused_ms: diagnostics.total_unfocused_ms,
        dropped_sample_ratio,
        stutter_spike_count,
        stutter_spike_ratio,
        unstable_capture,
    })
}

fn compute_capture_quality_score(
    frame_count: usize,
    focus_pauses: u32,
    samples_dropped_unfocused: usize,
    process_mismatch_detected: bool,
    longest_unfocused_ms: u64,
    stutter_spike_ratio: f64,
) -> u8 {
    let mut score: i32 = 100;

    if frame_count < 120 {
        score -= 10;
    }

    score -= ((focus_pauses as i32) * 5).min(35);

    if samples_dropped_unfocused > 0 {
        score -= ((samples_dropped_unfocused as i32) / 50 + 5).min(30);
    }

    if process_mismatch_detected {
        score -= 40;
    }

    if longest_unfocused_ms > 0 {
        score -= ((longest_unfocused_ms as i32) / 1000 * 5).min(20);
    }

    if stutter_spike_ratio > 0.0 {
        score -= (stutter_spike_ratio * 100.0).round() as i32;
    }

    score.clamp(0, 100) as u8
}

fn compute_dropped_sample_ratio(frame_count: usize, dropped: usize) -> f64 {
    let total = frame_count.saturating_add(dropped);
    if total == 0 {
        return 0.0;
    }
    dropped as f64 / total as f64
}

#[derive(Debug, Default)]
struct MangoHudStreamParser {
    frametime_col: Option<usize>,
    header_seen: bool,
}

impl MangoHudStreamParser {
    fn parse_line(&mut self, line: &str) -> Option<f64> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let fields: Vec<&str> = trimmed.split(',').collect();

        if !self.header_seen {
            if let Some(idx) = find_frametime_column(&fields) {
                self.frametime_col = Some(idx);
                self.header_seen = true;
                return None;
            }
            self.header_seen = true;
        }

        if let Some(idx) = self.frametime_col {
            return fields.get(idx).and_then(|value| parse_frametime(value));
        }

        if fields.len() == 1 {
            return parse_frametime(fields[0]);
        }

        if fields.len() >= 2 {
            // MangoHud data lines are usually fps,frametime,...
            if let Some(ft) = parse_frametime(fields[1]) {
                return Some(ft);
            }
        }

        parse_frametime(fields[0])
    }
}

fn parse_chunk_frametimes(
    parser: &mut MangoHudStreamParser,
    chunk: &str,
    partial_line: &mut String,
) -> Vec<f64> {
    if chunk.is_empty() {
        return Vec::new();
    }

    let mut combined = String::with_capacity(partial_line.len() + chunk.len());
    combined.push_str(partial_line);
    combined.push_str(chunk);
    partial_line.clear();

    let mut frame_times = Vec::new();
    for segment in combined.split_inclusive('\n') {
        if segment.ends_with('\n') {
            let line = segment.trim_end_matches('\n').trim_end_matches('\r');
            if let Some(ft) = parser.parse_line(line) {
                frame_times.push(ft);
            }
        } else {
            partial_line.push_str(segment);
        }
    }

    frame_times
}

fn find_frametime_column(fields: &[&str]) -> Option<usize> {
    fields.iter().position(|field| {
        let normalized = field.trim().to_ascii_lowercase();
        normalized == "frametime" || normalized == "frametime_ms"
    })
}

fn parse_frametime(value: &str) -> Option<f64> {
    let parsed = value.trim().parse::<f64>().ok()?;
    if parsed.is_finite() && parsed > 0.0 && parsed <= 10_000.0 {
        Some(parsed)
    } else {
        None
    }
}

fn prime_mangohud_parser_with_existing_header(
    capture_path: &std::path::Path,
    parser: &mut MangoHudStreamParser,
) -> Result<()> {
    let file = File::open(capture_path)
        .with_context(|| format!("Failed to open {}", capture_path.display()))?;
    let mut prefix = String::new();
    file.take(64 * 1024)
        .read_to_string(&mut prefix)
        .with_context(|| format!("Failed to read {}", capture_path.display()))?;

    for line in prefix.lines() {
        if parser.frametime_col.is_some() {
            break;
        }
        let _ = parser.parse_line(line);
    }

    Ok(())
}

fn adjust_poll_interval(current_ms: u64, base_ms: u64, new_samples: usize) -> u64 {
    let base_ms = base_ms.clamp(50, 500);
    let current_ms = current_ms.clamp(50, 500);

    if new_samples == 0 {
        return (current_ms + 25).min(500);
    }

    if new_samples >= 350 {
        return 50;
    }

    if new_samples >= 180 {
        return base_ms.min(75);
    }

    if new_samples <= 10 {
        return (base_ms + 25).min(250);
    }

    base_ms
}

fn init_capture_diagnostics(options: &LiveCaptureOptions) -> CaptureDiagnostics {
    CaptureDiagnostics {
        target_process: options.process_name.clone(),
        process_validation_enabled: options.process_validation && options.process_name.is_some(),
        ..CaptureDiagnostics::default()
    }
}

fn monitor_focus_enabled(options: &LiveCaptureOptions, diagnostics: &CaptureDiagnostics) -> bool {
    options.pause_on_unfocus
        || diagnostics.process_validation_enabled
        || options.focus_policy == FocusPolicy::Strict
}

fn update_diagnostics_from_focus_tracker(
    diagnostics: &mut CaptureDiagnostics,
    focus_tracker: &FocusTracker,
) {
    diagnostics.focus_pauses = focus_tracker.focus_pauses();
    diagnostics.longest_unfocused_ms = focus_tracker.longest_unfocused_ms();
    diagnostics.total_unfocused_ms = focus_tracker.total_unfocused_ms();
    diagnostics.process_mismatch_detected =
        diagnostics.process_mismatch_detected || focus_tracker.mismatch_detected();
}

fn strict_focus_violation_message(strict_unfocus_grace_ms: u64) -> String {
    format!(
        "Strict focus policy blocked this capture due to sustained focus loss (> {}ms).",
        strict_unfocus_grace_ms
    )
}

struct FocusTracker {
    target_process_normalized: Option<String>,
    focus_policy: FocusPolicy,
    monitor_enabled: bool,
    pause_collection: bool,
    cached_collect: bool,
    focus_pauses: u32,
    mismatch_detected: bool,
    last_probe: Instant,
    strict_unfocus_grace_ms: u64,
    unfocused_since: Option<Instant>,
    total_unfocused_ms: u64,
    longest_unfocused_ms: u64,
    strict_focus_violation: bool,
}

fn should_collect_for_target_process(
    target_process_normalized: Option<&str>,
    active_process: Option<&str>,
) -> bool {
    match target_process_normalized {
        Some(target) => match active_process {
            Some(active) => focus::process_name_matches(active, target),
            None => true,
        },
        None => true,
    }
}

impl FocusTracker {
    fn new(
        target_process: Option<String>,
        focus_policy: FocusPolicy,
        monitor_enabled: bool,
        pause_collection: bool,
        strict_unfocus_grace_ms: u64,
    ) -> Self {
        Self {
            target_process_normalized: target_process.as_deref().map(focus::normalize_process_name),
            focus_policy,
            monitor_enabled,
            pause_collection,
            cached_collect: true,
            focus_pauses: 0,
            mismatch_detected: false,
            last_probe: Instant::now() - Duration::from_secs(1),
            strict_unfocus_grace_ms,
            unfocused_since: None,
            total_unfocused_ms: 0,
            longest_unfocused_ms: 0,
            strict_focus_violation: false,
        }
    }

    fn should_collect(&mut self) -> bool {
        if !self.monitor_enabled {
            self.cached_collect = true;
            return true;
        }

        if self.last_probe.elapsed() < Duration::from_millis(400) {
            return self.effective_collect(self.cached_collect);
        }

        self.last_probe = Instant::now();

        let active_process = focus::foreground_process_name();
        let collect_now = should_collect_for_target_process(
            self.target_process_normalized.as_deref(),
            active_process.as_deref(),
        );
        if let (Some(target), Some(active)) = (
            self.target_process_normalized.as_deref(),
            active_process.as_deref(),
        ) {
            if !focus::process_name_matches(active, target) {
                self.mismatch_detected = true;
            }
        }

        self.apply_collect_state(collect_now);
        self.effective_collect(collect_now)
    }

    fn effective_collect(&self, collect_now: bool) -> bool {
        if self.pause_collection {
            collect_now
        } else {
            true
        }
    }

    fn apply_collect_state(&mut self, collect_now: bool) {
        if self.cached_collect && !collect_now {
            self.focus_pauses = self.focus_pauses.saturating_add(1);
            self.unfocused_since = Some(Instant::now());
        }

        if collect_now {
            self.finalize_unfocused_segment();
        } else if let Some(started) = self.unfocused_since {
            let unfocused_ms = started.elapsed().as_millis() as u64;
            self.longest_unfocused_ms = self.longest_unfocused_ms.max(unfocused_ms);
            if self.focus_policy == FocusPolicy::Strict
                && unfocused_ms > self.strict_unfocus_grace_ms
            {
                self.strict_focus_violation = true;
            }
        }

        self.cached_collect = collect_now;
    }

    fn focus_pauses(&self) -> u32 {
        self.focus_pauses
    }

    fn mismatch_detected(&self) -> bool {
        self.mismatch_detected
    }

    fn finalize_unfocused_segment(&mut self) {
        let Some(started) = self.unfocused_since.take() else {
            return;
        };
        let unfocused_ms = started.elapsed().as_millis() as u64;
        self.total_unfocused_ms = self.total_unfocused_ms.saturating_add(unfocused_ms);
        self.longest_unfocused_ms = self.longest_unfocused_ms.max(unfocused_ms);
        if self.focus_policy == FocusPolicy::Strict && unfocused_ms > self.strict_unfocus_grace_ms {
            self.strict_focus_violation = true;
        }
    }

    fn finalize_unfocused_tracking(&mut self) {
        self.finalize_unfocused_segment();
    }

    fn longest_unfocused_ms(&self) -> u64 {
        self.longest_unfocused_ms
    }

    fn total_unfocused_ms(&self) -> u64 {
        self.total_unfocused_ms
    }

    fn strict_focus_violation(&self) -> bool {
        self.strict_focus_violation
    }
}

#[cfg(test)]
mod tests {
    use super::{
        adjust_poll_interval, build_result, compute_capture_quality_score,
        compute_dropped_sample_ratio, find_frametime_column, parse_chunk_frametimes,
        parse_frametime, resolve_source, should_collect_for_target_process, CaptureDiagnostics,
        CaptureSource, FocusPolicy, FocusTracker, LiveCaptureOptions, MangoHudStreamParser,
    };
    use crate::import::FrameData;
    use chrono::Utc;
    use std::io::Write;
    use std::time::{Duration, Instant};
    use tempfile::NamedTempFile;

    #[test]
    fn parse_frametime_values() {
        assert_eq!(parse_frametime("16.67"), Some(16.67));
        assert_eq!(parse_frametime("0"), None);
        assert_eq!(parse_frametime("nan"), None);
    }

    #[test]
    fn finds_frametime_column() {
        let fields = vec!["fps", "frametime", "cpu_load"];
        assert_eq!(find_frametime_column(&fields), Some(1));
    }

    #[test]
    fn stream_parser_handles_header_and_data() {
        let mut parser = MangoHudStreamParser::default();
        assert_eq!(parser.parse_line("fps,frametime,cpu_load"), None);
        assert_eq!(parser.parse_line("120,8.33,40"), Some(8.33));
    }

    #[test]
    fn chunk_parser_keeps_partial_lines_between_reads() {
        let mut parser = MangoHudStreamParser::default();
        let mut partial = String::new();

        let first = parse_chunk_frametimes(&mut parser, "fps,frametime\n120,8.3", &mut partial);
        assert!(first.is_empty());
        assert_eq!(partial, "120,8.3");

        let second = parse_chunk_frametimes(&mut parser, "3\n100,10.0\n", &mut partial);
        assert_eq!(second, vec![8.33, 10.0]);
        assert!(partial.is_empty());
    }

    #[test]
    fn adaptive_polling_reacts_to_sample_volume() {
        assert_eq!(adjust_poll_interval(100, 100, 0), 125);
        assert_eq!(adjust_poll_interval(100, 100, 500), 50);
        assert_eq!(adjust_poll_interval(100, 100, 5), 125);
        assert_eq!(adjust_poll_interval(100, 100, 50), 100);
    }

    #[test]
    fn quality_score_penalizes_focus_and_mismatch() {
        let clean = compute_capture_quality_score(300, 0, 0, false, 0, 0.0);
        let noisy = compute_capture_quality_score(100, 3, 250, true, 4_000, 0.15);
        assert!(clean > noisy);
    }

    #[test]
    fn stutter_spike_metrics_are_deterministic() {
        let frame_data = FrameData {
            frame_times_ms: vec![10.0, 11.0, 9.5, 40.0, 39.0, 10.2],
            application: None,
            duration_secs: 1.0,
            source: "test".to_string(),
        };
        let count = frame_data.stutter_spike_count(1000.0);
        let ratio = frame_data.stutter_spike_ratio(1000.0);
        assert_eq!(count, 2);
        assert!(ratio > 0.30 && ratio < 0.35);
    }

    #[test]
    fn dropped_ratio_computes_expected_value() {
        let ratio = compute_dropped_sample_ratio(900, 100);
        assert!((ratio - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_focus_grace_allows_brief_focus_flaps() {
        let mut tracker = FocusTracker::new(
            Some("game.exe".to_string()),
            FocusPolicy::Strict,
            true,
            true,
            1_000,
        );

        tracker.apply_collect_state(false);
        tracker.unfocused_since = Some(Instant::now() - Duration::from_millis(250));
        tracker.apply_collect_state(true);

        tracker.apply_collect_state(false);
        tracker.unfocused_since = Some(Instant::now() - Duration::from_millis(350));
        tracker.apply_collect_state(true);

        assert_eq!(tracker.focus_pauses(), 2);
        assert!(!tracker.strict_focus_violation());
        assert!(tracker.total_unfocused_ms() >= 500);
    }

    #[test]
    fn strict_focus_grace_rejects_sustained_focus_loss() {
        let mut tracker = FocusTracker::new(
            Some("game.exe".to_string()),
            FocusPolicy::Strict,
            true,
            true,
            1_000,
        );

        tracker.apply_collect_state(false);
        tracker.unfocused_since = Some(Instant::now() - Duration::from_millis(1_500));
        tracker.apply_collect_state(false);

        assert!(tracker.strict_focus_violation());
        assert!(tracker.longest_unfocused_ms() >= 1_500);
    }

    #[test]
    fn strict_focus_tolerates_unknown_foreground_process() {
        assert!(should_collect_for_target_process(Some("game.exe"), None));
    }

    #[test]
    fn monitor_only_mode_tracks_mismatch_without_dropping_samples() {
        let mut tracker = FocusTracker::new(
            Some("game.exe".to_string()),
            FocusPolicy::Strict,
            true,
            false,
            1_000,
        );

        tracker.apply_collect_state(false);
        assert!(tracker.effective_collect(false));
        assert_eq!(tracker.focus_pauses(), 1);
    }

    #[test]
    fn high_fps_capture_stays_stable_when_clean() {
        let result = build_result(
            vec![4.0; 2_000],
            "test",
            None,
            Some("test-game".to_string()),
            Utc::now(),
            Utc::now(),
            8.0,
            1000.0,
            CaptureDiagnostics::default(),
        )
        .expect("expected clean result");

        assert!(result.avg_fps > 200.0);
        assert!(!result.unstable_capture);
        assert!(result.capture_quality_score >= 90);
    }

    #[test]
    fn capture_marked_unstable_when_dropped_ratio_is_high() {
        let result = build_result(
            vec![8.0; 2_000],
            "test",
            None,
            Some("test-game".to_string()),
            Utc::now(),
            Utc::now(),
            16.0,
            1000.0,
            CaptureDiagnostics {
                samples_dropped_unfocused: 350,
                ..CaptureDiagnostics::default()
            },
        )
        .expect("expected parsed result");

        assert!(result.dropped_sample_ratio > 0.10);
        assert!(result.unstable_capture);
    }

    #[test]
    fn capture_marked_unstable_when_spike_ratio_is_high() {
        let mut frame_times = vec![8.0; 1_700];
        frame_times.extend(vec![45.0; 300]);

        let result = build_result(
            frame_times,
            "test",
            None,
            Some("test-game".to_string()),
            Utc::now(),
            Utc::now(),
            16.0,
            1000.0,
            CaptureDiagnostics::default(),
        )
        .expect("expected parsed result");

        assert!(result.stutter_spike_ratio > 0.08);
        assert!(result.unstable_capture);
    }

    #[test]
    fn resolve_source_prefers_explicit_mangohud_file_in_auto_mode() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "fps,frametime,cpu_load").expect("write header");
        writeln!(file, "120,8.33,40").expect("write row");

        let options = LiveCaptureOptions {
            source: CaptureSource::Auto,
            duration_secs: 30,
            file: Some(file.path().to_path_buf()),
            game_hint: None,
            process_name: None,
            focus_policy: FocusPolicy::Lenient,
            pause_on_unfocus: false,
            poll_ms: 100,
            process_validation: false,
            max_frame_time_ms: 1000.0,
            strict_unfocus_grace_ms: 1000,
        };

        assert_eq!(
            resolve_source(&options).expect("source resolution"),
            CaptureSource::MangoHud
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_source_treats_non_mangohud_file_as_presentmon_on_windows() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "this,is,not,mangohud").expect("write row");

        let options = LiveCaptureOptions {
            source: CaptureSource::Auto,
            duration_secs: 30,
            file: Some(file.path().to_path_buf()),
            game_hint: None,
            process_name: None,
            focus_policy: FocusPolicy::Lenient,
            pause_on_unfocus: false,
            poll_ms: 100,
            process_validation: false,
            max_frame_time_ms: 1000.0,
            strict_unfocus_grace_ms: 1000,
        };

        assert_eq!(
            resolve_source(&options).expect("source resolution"),
            CaptureSource::PresentMon
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn resolve_source_rejects_non_mangohud_file_in_auto_mode_on_non_windows() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "this,is,not,mangohud").expect("write row");

        let options = LiveCaptureOptions {
            source: CaptureSource::Auto,
            duration_secs: 30,
            file: Some(file.path().to_path_buf()),
            game_hint: None,
            process_name: None,
            focus_policy: FocusPolicy::Lenient,
            pause_on_unfocus: false,
            poll_ms: 100,
            process_validation: false,
            max_frame_time_ms: 1000.0,
            strict_unfocus_grace_ms: 1000,
        };

        assert!(resolve_source(&options).is_err());
    }
}
