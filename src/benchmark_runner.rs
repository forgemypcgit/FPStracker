//! Benchmark runner module
//!
//! Runs optional synthetic benchmarks to measure hardware performance
//! Uses only open-source and built-in legal tools.

use anyhow::{Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use serde_json::Value;
#[cfg(target_os = "windows")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::mpsc;
#[cfg(target_os = "windows")]
use sysinfo::System;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_NO_BUFFERING,
    FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Version of the synthetic suite semantics.
///
/// Bump this when score meanings change (normalization, new tools, different weightings), so the
/// backend can compare like-for-like runs safely.
pub const SYNTHETIC_SUITE_VERSION: &str = "1";

/// Benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Synthetic benchmark suite version (internal schema version for score semantics).
    pub synthetic_suite_version: String,
    /// CPU benchmark score (if run)
    pub cpu_score: Option<u64>,
    /// GPU benchmark score (if run)
    pub gpu_score: Option<u64>,
    /// RAM benchmark score (if run)
    pub ram_score: Option<u64>,
    /// Storage benchmark score (if run)
    pub disk_score: Option<u64>,
    /// Source label for `cpu_score` (e.g., winsat, 7z, sysbench, internal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_score_source: Option<String>,
    /// Source label for `gpu_score` (e.g., winsat, glmark2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_score_source: Option<String>,
    /// Source label for `ram_score` (e.g., winsat, internal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_score_source: Option<String>,
    /// Source label for `disk_score` (e.g., winsat, diskspd, internal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_score_source: Option<String>,
    /// Note about WinSAT (e.g., skipped due to missing elevation).
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winsat_note: Option<String>,
    /// Optional: 7-Zip single-thread benchmark rating (MIPS) when available.
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_7z_single_mips: Option<u64>,
    /// Optional: 7-Zip multi-thread benchmark rating (MIPS) when available.
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_7z_multi_mips: Option<u64>,
    /// Optional: DiskSpd sequential read throughput when available (MiB/s, rounded).
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diskspd_read_mb_s: Option<u64>,
    /// Optional: DiskSpd sequential write throughput when available (MiB/s, rounded).
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diskspd_write_mb_s: Option<u64>,
    /// Optional: Blender CPU render time (ms) when Blender is installed.
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blender_cpu_render_ms: Option<u64>,
    /// Optional: Blender CPU render settings used (resolution/samples).
    #[cfg(target_os = "windows")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blender_cpu_render_settings: Option<String>,
    /// sysbench CPU throughput (events/sec) single-thread (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sysbench_cpu_1t_events_s: Option<u64>,
    /// sysbench CPU throughput (events/sec) multi-thread (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sysbench_cpu_mt_events_s: Option<u64>,
    /// sysbench memory throughput (MiB/s) (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sysbench_memory_mib_s: Option<u64>,
    /// fio sequential read throughput (MiB/s) (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fio_seq_read_mib_s: Option<u64>,
    /// fio sequential write throughput (MiB/s) (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fio_seq_write_mib_s: Option<u64>,
    /// fio random read IOPS (4k, QD1) (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fio_randread_iops: Option<u64>,
    /// fio random write IOPS (4k, QD1) (best-effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fio_randwrite_iops: Option<u64>,
    /// Peak CPU frequency observed during test (MHz)
    pub cpu_peak_clock_mhz: Option<u64>,
    /// Peak GPU frequency observed during test (MHz)
    pub gpu_peak_clock_mhz: Option<u64>,
    /// Peak GPU memory clock observed (MHz)
    pub gpu_memory_peak_clock_mhz: Option<u64>,
    /// Test duration in seconds
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkProgressUpdate {
    pub completed_steps: usize,
    pub total_steps: usize,
    pub status: String,
}

#[derive(Clone, Default)]
pub struct BenchmarkRunOptions {
    /// Suppress all stdout printing from the runner.
    ///
    /// This is useful when benchmarks are triggered from the embedded Web UI server or the fullscreen
    /// TUI, where printing would corrupt the UI.
    pub quiet: bool,
    /// Optional callback for progress updates.
    ///
    /// This receives progress events even when `quiet` is true.
    pub progress: Option<Arc<dyn Fn(BenchmarkProgressUpdate) + Send + Sync + 'static>>,
}

/// Available benchmark tools
#[derive(Debug, Clone, Copy)]
pub enum BenchmarkType {
    /// Quick 30-second test
    Quick,
    /// Standard 2-minute test
    Standard,
    /// Extended 5-minute test
    Extended,
}

impl BenchmarkType {
    pub fn duration(&self) -> Duration {
        match self {
            BenchmarkType::Quick => Duration::from_secs(30),
            BenchmarkType::Standard => Duration::from_secs(120),
            BenchmarkType::Extended => Duration::from_secs(300),
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            BenchmarkType::Quick => "30 seconds - Quick check",
            BenchmarkType::Standard => "2 minutes - Recommended",
            BenchmarkType::Extended => "5 minutes - Most accurate",
        }
    }

    pub fn profile_key(&self) -> &'static str {
        match self {
            BenchmarkType::Quick => "quick",
            BenchmarkType::Standard => "standard",
            BenchmarkType::Extended => "extended",
        }
    }
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
struct PdhCpuFreqSampler {
    query: isize,
    counter: isize,
}

#[cfg(target_os = "windows")]
impl PdhCpuFreqSampler {
    fn new() -> Option<Self> {
        unsafe {
            let mut query: isize = 0;
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                return None;
            }

            let mut counter: isize = 0;
            let path = wide_null(r"\\Processor Information(_Total)\\Processor Frequency");
            if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 {
                let _ = PdhCloseQuery(query);
                return None;
            }

            let _ = PdhCollectQueryData(query);
            Some(Self { query, counter })
        }
    }

    fn sample_mhz(&mut self) -> Option<u64> {
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }

            let mut value: PDH_FMT_COUNTERVALUE = std::mem::zeroed();
            let mut ty: u32 = 0;
            if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, &mut ty, &mut value) != 0 {
                return None;
            }

            let mhz = value.Anonymous.doubleValue;
            if mhz.is_finite() && (100.0..=20_000.0).contains(&mhz) {
                Some(mhz.round() as u64)
            } else {
                None
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for PdhCpuFreqSampler {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

#[cfg(target_os = "windows")]
struct WindowsCpuClockSampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "windows")]
impl WindowsCpuClockSampler {
    fn new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(0));
        let stop_for_thread = Arc::clone(&stop);
        let peak_for_thread = Arc::clone(&peak);
        let handle = std::thread::spawn(move || {
            let mut pdh = PdhCpuFreqSampler::new();
            let mut sys = System::new();
            while !stop_for_thread.load(Ordering::Relaxed) {
                let sampled = pdh.as_mut().and_then(|sampler| sampler.sample_mhz());
                if let Some(mhz) = sampled {
                    peak_for_thread.fetch_max(mhz, Ordering::Relaxed);
                } else {
                    // Fallback if PDH is unavailable.
                    sys.refresh_cpu_all();
                    for cpu in sys.cpus() {
                        let mhz = cpu.frequency();
                        if mhz > 0 {
                            peak_for_thread.fetch_max(mhz, Ordering::Relaxed);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });

        Self {
            stop,
            peak,
            handle: Some(handle),
        }
    }

    fn finish(&mut self) -> u64 {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.peak.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsCpuClockSampler {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Print warning screen before benchmarks
pub fn print_benchmark_warning(bench_type: BenchmarkType) {
    println!(
        "\n{}",
        "╔══════════════════════════════════════════════════════════════╗".bright_yellow()
    );
    println!(
        "{}",
        "║                                                              ║".bright_yellow()
    );
    println!(
        "{}",
        "║              ⚠️  BENCHMARK WARNING ⚠️                        ║"
            .bright_yellow()
            .bold()
    );
    println!(
        "{}",
        "║                                                              ║".bright_yellow()
    );
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════╣".bright_yellow()
    );
    println!(
        "{}",
        "║                                                              ║".bright_yellow()
    );
    println!(
        "{}",
        "║  Before starting:                                            ║".bright_yellow()
    );
    println!(
        "{}",
        "║                                                              ║".bright_yellow()
    );
    println!(
        "{}",
        "║  • Fans will spin loudly (normal under load)                 ║".bright_white()
    );
    println!(
        "{}",
        format!(
            "║  • Expected duration target: ~{}                             ║",
            format_duration(bench_type.duration())
        )
        .bright_white()
    );
    println!(
        "{}",
        "║  • Close Chrome, Discord, game launchers first               ║".bright_white()
    );
    println!(
        "{}",
        "║  • Keep laptop plugged in and well ventilated                ║".bright_white()
    );
    println!(
        "{}",
        "║                                                              ║".bright_yellow()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════╝".bright_yellow()
    );
    println!();
}

/// Display benchmark menu and get user choice
pub fn show_benchmark_menu() -> Option<BenchmarkType> {
    println!("{}", "Choose benchmark duration:".bright_cyan().bold());
    println!();
    println!(
        "  {} {}",
        "[1]".bright_green(),
        "Run Quick Test (30 seconds)".bright_white()
    );
    println!(
        "  {} {} {}",
        "[2]".bright_green(),
        "Run Standard Test (2 minutes)".bright_white(),
        "← Recommended".bright_cyan()
    );
    println!(
        "  {} {}",
        "[3]".bright_green(),
        "Run Extended Test (5 minutes)".bright_white()
    );
    println!();
    println!(
        "  {} {}",
        "[4]".bright_red(),
        "Skip Benchmarks (manual data only)".bright_white()
    );
    println!();
    print!("Enter choice [1-4]: ");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();

    match input.trim() {
        "1" => Some(BenchmarkType::Quick),
        "2" => Some(BenchmarkType::Standard),
        "3" => Some(BenchmarkType::Extended),
        "4" => None,
        _ => {
            println!(
                "{}",
                "Invalid choice, defaulting to Standard test.".bright_yellow()
            );
            Some(BenchmarkType::Standard)
        }
    }
}

/// Check if benchmark tools are available (platform-specific)
pub fn check_benchmark_tools() -> Vec<(String, bool)> {
    let mut tools = vec![];

    #[cfg(target_os = "linux")]
    {
        tools.push(("glmark2".to_string(), is_tool_available("glmark2")));
        tools.push(("sysbench".to_string(), is_tool_available("sysbench")));
        tools.push(("fio".to_string(), is_tool_available("fio")));
        tools.push(("stress-ng".to_string(), is_tool_available("stress-ng")));
    }

    #[cfg(target_os = "windows")]
    {
        tools.push(("winsat".to_string(), is_windows_command_available("winsat")));
        tools.push(("powershell".to_string(), is_windows_powershell_available()));
        tools.push(("7z".to_string(), is_tool_available("7z")));
        tools.push((
            "diskspd".to_string(),
            crate::deps::locate_diskspd_executable().is_some(),
        ));
        tools.push((
            "blender".to_string(),
            crate::deps::locate_blender_executable().is_some(),
        ));
    }

    tools
}

fn is_tool_available(tool: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("where")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[cfg(target_os = "windows")]
fn is_windows_command_available(cmd: &str) -> bool {
    Command::new("cmd")
        .args(["/C", "where", cmd])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_windows_powershell_available() -> bool {
    is_windows_command_available("pwsh") || is_windows_command_available("powershell")
}

/// Run benchmarks and return results
pub fn run_benchmarks(bench_type: BenchmarkType) -> Result<BenchmarkResults> {
    run_benchmarks_with_options(bench_type, BenchmarkRunOptions::default())
}

/// Run benchmarks and return results, with additional runner options.
pub fn run_benchmarks_with_options(
    bench_type: BenchmarkType,
    options: BenchmarkRunOptions,
) -> Result<BenchmarkResults> {
    let start_time = Instant::now();

    #[cfg(target_os = "windows")]
    let mut clock_sampler = WindowsCpuClockSampler::new();

    if !options.quiet {
        println!(
            "\n{}",
            "⚡ Starting synthetic benchmarks...".bright_cyan().bold()
        );
        println!(
            "   {} {}",
            "Target profile:".bright_white(),
            format_duration(bench_type.duration()).bright_yellow()
        );
    }

    #[cfg(target_os = "windows")]
    {
        if !options.quiet {
            println!(
                "   {}",
                "Windows mode uses WinSAT formal scoring for CPU/RAM/SSD/GPU precision."
                    .bright_white()
            );
        }
    }

    if !options.quiet {
        println!();
    }

    let mut results = BenchmarkResults {
        synthetic_suite_version: SYNTHETIC_SUITE_VERSION.to_string(),
        cpu_score: None,
        gpu_score: None,
        ram_score: None,
        disk_score: None,
        cpu_score_source: None,
        gpu_score_source: None,
        ram_score_source: None,
        disk_score_source: None,
        #[cfg(target_os = "windows")]
        winsat_note: None,
        #[cfg(target_os = "windows")]
        cpu_7z_single_mips: None,
        #[cfg(target_os = "windows")]
        cpu_7z_multi_mips: None,
        #[cfg(target_os = "windows")]
        diskspd_read_mb_s: None,
        #[cfg(target_os = "windows")]
        diskspd_write_mb_s: None,
        #[cfg(target_os = "windows")]
        blender_cpu_render_ms: None,
        #[cfg(target_os = "windows")]
        blender_cpu_render_settings: None,
        sysbench_cpu_1t_events_s: None,
        sysbench_cpu_mt_events_s: None,
        sysbench_memory_mib_s: None,
        fio_seq_read_mib_s: None,
        fio_seq_write_mib_s: None,
        fio_randread_iops: None,
        fio_randwrite_iops: None,
        cpu_peak_clock_mhz: None,
        gpu_peak_clock_mhz: None,
        gpu_memory_peak_clock_mhz: None,
        duration_secs: 0.0,
    };

    #[cfg(target_os = "windows")]
    {
        run_windows_benchmarks(bench_type, &mut results, &options)?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        run_non_windows_benchmarks(bench_type, &mut results, &options);
    }

    // Try to get peak clocks if nvidia-smi is available
    if let Ok((gpu_clock, mem_clock)) = get_peak_gpu_clocks() {
        results.gpu_peak_clock_mhz = gpu_clock;
        results.gpu_memory_peak_clock_mhz = mem_clock;
        if !options.quiet {
            if let Some(clock) = gpu_clock {
                println!(
                    "   {} Peak GPU clock detected: {} MHz",
                    "✓".bright_green(),
                    clock.to_string().bright_cyan()
                );
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let cpu_peak = clock_sampler.finish();
        if cpu_peak >= 100 {
            results.cpu_peak_clock_mhz = Some(cpu_peak);
        }
    }

    results.duration_secs = start_time.elapsed().as_secs_f64();

    if !options.quiet {
        println!(
            "\n{}",
            format!(
                "✅ Benchmarks complete! Duration: {:.1} seconds",
                results.duration_secs
            )
            .bright_green()
            .bold()
        );
        println!();
    }

    Ok(results)
}

#[cfg(not(target_os = "windows"))]
fn run_non_windows_benchmarks(
    bench_type: BenchmarkType,
    results: &mut BenchmarkResults,
    options: &BenchmarkRunOptions,
) {
    #[cfg(target_os = "linux")]
    {
        run_linux_benchmarks(bench_type, results, options);
    }

    #[cfg(not(target_os = "linux"))]
    {
        let total_steps = 2;
        render_progress(0, total_steps, "Starting CPU benchmark", options);

        match run_cpu_benchmark(bench_type) {
            Ok(score) => {
                results.cpu_score = Some(score);
                render_progress(1, total_steps, "CPU benchmark complete", options);
                if !options.quiet {
                    println!(
                        "   {} CPU benchmark score: {}",
                        "✓".bright_green(),
                        score.to_string().bright_cyan()
                    );
                }
            }
            Err(e) => {
                render_progress(1, total_steps, "CPU benchmark unavailable", options);
                if !options.quiet {
                    println!(
                        "   {} CPU benchmark failed: {}",
                        "⚠".bright_yellow(),
                        e.to_string().bright_red()
                    );
                }
            }
        }

        render_progress(1, total_steps, "Starting GPU benchmark", options);
        match run_gpu_benchmark(bench_type) {
            Ok(score) => {
                results.gpu_score = Some(score);
                render_progress(2, total_steps, "GPU benchmark complete", options);
                if !options.quiet {
                    println!(
                        "   {} GPU benchmark score: {}",
                        "✓".bright_green(),
                        score.to_string().bright_cyan()
                    );
                }
            }
            Err(e) => {
                render_progress(2, total_steps, "GPU benchmark unavailable", options);
                if !options.quiet {
                    println!(
                        "   {} GPU benchmark failed: {}",
                        "⚠".bright_yellow(),
                        e.to_string().bright_red()
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn run_linux_benchmarks(
    bench_type: BenchmarkType,
    results: &mut BenchmarkResults,
    options: &BenchmarkRunOptions,
) {
    let (cpu_budget, ram_budget, disk_budget, gpu_budget, disk_mb) = internal_budgets(bench_type);
    let total_steps = 4;

    render_progress(0, total_steps, "CPU benchmark", options);
    if let Ok((score, one_t, mt)) = sysbench_cpu_score(cpu_budget) {
        results.cpu_score = Some(score);
        results.cpu_score_source = Some("sysbench".to_string());
        results.sysbench_cpu_1t_events_s = one_t;
        results.sysbench_cpu_mt_events_s = mt;
    } else if let Ok(score) = internal_cpu_score(cpu_budget) {
        results.cpu_score = Some(score);
        results.cpu_score_source = Some("internal".to_string());
    }

    render_progress(1, total_steps, "RAM benchmark", options);
    if let Ok((score, mib_s)) = sysbench_memory_score(ram_budget) {
        results.ram_score = Some(score);
        results.ram_score_source = Some("sysbench memory".to_string());
        results.sysbench_memory_mib_s = Some(mib_s);
    } else if let Ok(score) = internal_ram_score(ram_budget) {
        results.ram_score = Some(score);
        results.ram_score_source = Some("internal".to_string());
    }

    render_progress(2, total_steps, "Disk benchmark", options);
    if let Ok(metrics) = fio_disk_metrics(disk_budget, disk_mb) {
        results.disk_score = Some(metrics.disk_score);
        results.disk_score_source = Some("fio".to_string());
        results.fio_seq_read_mib_s = metrics.seq_read_mib_s;
        results.fio_seq_write_mib_s = metrics.seq_write_mib_s;
        results.fio_randread_iops = metrics.randread_iops;
        results.fio_randwrite_iops = metrics.randwrite_iops;
    } else if let Ok(score) = internal_disk_score(disk_budget, disk_mb) {
        results.disk_score = Some(score);
        results.disk_score_source = Some("internal".to_string());
    }

    render_progress(3, total_steps, "GPU benchmark (optional)", options);
    if let Ok(score) = linux_gpu_score(gpu_budget) {
        results.gpu_score = Some(score);
        results.gpu_score_source = Some("glmark2".to_string());
    }
    render_progress(4, total_steps, "Linux synthetic complete", options);
}

#[cfg(target_os = "linux")]
fn sysbench_cpu_score(budget: Duration) -> Result<(u64, Option<u64>, Option<u64>)> {
    if !is_tool_available("sysbench") {
        anyhow::bail!("sysbench not available");
    }

    let secs = budget.as_secs().clamp(3, 30);
    let mt_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 64);

    let one_t = sysbench_cpu_events_per_second(1, secs)?;
    let mt = sysbench_cpu_events_per_second(mt_threads, secs)?;

    // Stable mapping: keep output in the same WinSAT-ish 1..10_000 range.
    // `sysbench cpu` events/sec scales roughly with CPU single+multi performance.
    let composite = (one_t * 0.35) + (mt * 0.65);
    let score = (composite / 20.0).round() as u64;
    Ok((
        score.clamp(1, 10_000),
        Some(one_t.round() as u64),
        Some(mt.round() as u64),
    ))
}

#[cfg(target_os = "linux")]
fn sysbench_cpu_events_per_second(threads: usize, secs: u64) -> Result<f64> {
    let mut cmd = Command::new("sysbench");
    cmd.args([
        "cpu",
        "--cpu-max-prime=20000",
        &format!("--threads={threads}"),
        &format!("--time={secs}"),
        "run",
    ]);
    let timeout = Duration::from_secs(secs.saturating_add(30));
    let output = run_command_capture_with_timeout(&mut cmd, timeout, "sysbench cpu")
        .context("Failed to run sysbench cpu")?;

    if !output.status.success() {
        anyhow::bail!("sysbench cpu failed ({})", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.split("events per second:").nth(1) {
            let value = rest.trim();
            if let Ok(parsed) = value.parse::<f64>() {
                if parsed.is_finite() && parsed > 0.0 {
                    return Ok(parsed);
                }
            }
        }
    }

    anyhow::bail!("Could not parse sysbench cpu events/sec from output");
}

#[cfg(target_os = "linux")]
fn sysbench_memory_score(budget: Duration) -> Result<(u64, u64)> {
    if !is_tool_available("sysbench") {
        anyhow::bail!("sysbench not available");
    }

    let secs = budget.as_secs().clamp(3, 30);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 64);

    let mut cmd = Command::new("sysbench");
    cmd.args([
        "memory",
        "--memory-block-size=1M",
        "--memory-total-size=100T",
        &format!("--threads={threads}"),
        &format!("--time={secs}"),
        "run",
    ]);
    let timeout = Duration::from_secs(secs.saturating_add(30));
    let output = run_command_capture_with_timeout(&mut cmd, timeout, "sysbench memory")
        .context("Failed to run sysbench memory")?;

    if !output.status.success() {
        anyhow::bail!("sysbench memory failed ({})", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        // Example: "    transferred (MiB/sec):  38851.19"
        if let Some(rest) = line.split("transferred (MiB/sec):").nth(1) {
            if let Ok(parsed) = rest.trim().parse::<f64>() {
                if parsed.is_finite() && parsed > 0.0 {
                    let mib_s = parsed.round().max(1.0) as u64;
                    let score = (parsed / 20.0).round() as u64;
                    return Ok((score.clamp(1, 10_000), mib_s));
                }
            }
        }
    }

    anyhow::bail!("Could not parse sysbench memory MiB/sec from output");
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct FioDiskMetrics {
    disk_score: u64,
    seq_read_mib_s: Option<u64>,
    seq_write_mib_s: Option<u64>,
    randread_iops: Option<u64>,
    randwrite_iops: Option<u64>,
}

#[cfg(target_os = "linux")]
fn fio_disk_metrics(budget: Duration, file_mb: u64) -> Result<FioDiskMetrics> {
    if !is_tool_available("fio") {
        anyhow::bail!("fio not available");
    }

    let total_runtime = budget.as_secs().clamp(3, 30);
    let size_mb = file_mb.clamp(64, 1024);
    let filename =
        std::env::temp_dir().join(format!("fps-tracker-fio-{}.dat", uuid::Uuid::new_v4()));
    let filename_str = filename
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("fio temp filename is not valid UTF-8"))?
        .to_string();

    // Try direct=1 first (less cache skew). If it fails, fall back to buffered I/O.
    let direct_variants = ["1", "0"];
    let mut last_err: Option<anyhow::Error> = None;
    for direct in direct_variants {
        // Split the time budget: always run a short sequential profile. If we have enough time,
        // also run a 4k QD1 random profile so the reported IOPS are meaningful.
        let seq_runtime = (total_runtime / 2).clamp(2, 20);
        let rand_runtime = total_runtime.saturating_sub(seq_runtime).clamp(0, 20);

        let seq = fio_run_profile(
            &filename_str,
            size_mb,
            seq_runtime.max(2),
            direct,
            FioProfile::SequentialMixed,
        );
        let rand = if rand_runtime >= 2 {
            fio_run_profile(
                &filename_str,
                size_mb,
                rand_runtime,
                direct,
                FioProfile::Random4kQd1Mixed,
            )
            .map(Some)
        } else {
            Ok(None)
        };

        match (seq, rand) {
            (Ok(seq_value), Ok(rand_value)) => {
                let _ = std::fs::remove_file(&filename);
                return parse_fio_metrics(seq_value, rand_value);
            }
            (Err(err), _) | (_, Err(err)) => {
                last_err = Some(err);
                continue;
            }
        }
    }

    let _ = std::fs::remove_file(&filename);
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fio failed")))
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
enum FioProfile {
    SequentialMixed,
    Random4kQd1Mixed,
}

#[cfg(target_os = "linux")]
fn fio_run_profile(
    filename: &str,
    size_mb: u64,
    runtime_secs: u64,
    direct: &str,
    profile: FioProfile,
) -> Result<serde_json::Value> {
    let (rw, bs, mix_read) = match profile {
        // Sequential-ish: 1 MiB transfers, 50/50 read+write.
        FioProfile::SequentialMixed => ("readwrite", "1m", "50"),
        // Random: 4 KiB, QD1, 50/50 read+write.
        FioProfile::Random4kQd1Mixed => ("randrw", "4k", "50"),
    };

    let mut cmd = Command::new("fio");
    cmd.args([
        "--output-format=json",
        "--name=fps-tracker",
        &format!("--filename={filename}"),
        &format!("--size={size_mb}m"),
        "--ioengine=sync",
        "--iodepth=1",
        "--numjobs=1",
        "--group_reporting=1",
        &format!("--runtime={runtime_secs}"),
        "--time_based=1",
        &format!("--direct={direct}"),
        &format!("--rw={rw}"),
        &format!("--bs={bs}"),
        &format!("--rwmixread={mix_read}"),
    ]);
    let timeout = Duration::from_secs(runtime_secs.saturating_add(60));
    let output =
        run_command_capture_with_timeout(&mut cmd, timeout, "fio").context("Failed to run fio")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("fio failed ({}): {}", output.status, stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<serde_json::Value>(&stdout).context("Failed to parse fio JSON output")
}

#[cfg(target_os = "linux")]
fn parse_fio_metrics(
    sequential: serde_json::Value,
    random: Option<serde_json::Value>,
) -> Result<FioDiskMetrics> {
    // fio JSON structure: { "jobs": [ { "read": {...}, "write": {...} } ] }
    let seq_job = sequential
        .get("jobs")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| anyhow::anyhow!("fio JSON missing jobs[0]"))?;

    let (seq_read_mib_s, _seq_read_iops) = fio_parse_rw(seq_job.get("read"));
    let (seq_write_mib_s, _seq_write_iops) = fio_parse_rw(seq_job.get("write"));

    let (randread_iops, randwrite_iops) = if let Some(random) = random {
        let rand_job = random
            .get("jobs")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow::anyhow!("fio JSON missing jobs[0] (random profile)"))?;
        let (_rand_read_mib_s, rand_read_iops) = fio_parse_rw(rand_job.get("read"));
        let (_rand_write_mib_s, rand_write_iops) = fio_parse_rw(rand_job.get("write"));
        (rand_read_iops, rand_write_iops)
    } else {
        (None, None)
    };

    // One composite score for disk: weight sequential bandwidth and QD1 IOPS if present.
    let bw_component =
        seq_read_mib_s.unwrap_or(0) as f64 * 0.5 + seq_write_mib_s.unwrap_or(0) as f64 * 0.5;
    let iops_component =
        (randread_iops.unwrap_or(0) as f64 + randwrite_iops.unwrap_or(0) as f64) / 2.0;
    let score = ((bw_component / 5.0) + (iops_component / 100.0)).round() as u64;

    Ok(FioDiskMetrics {
        disk_score: score.clamp(1, 10_000),
        seq_read_mib_s,
        seq_write_mib_s,
        randread_iops,
        randwrite_iops,
    })
}

#[cfg(target_os = "linux")]
fn fio_parse_rw(section: Option<&serde_json::Value>) -> (Option<u64>, Option<u64>) {
    let Some(section) = section else {
        return (None, None);
    };
    let bw_kib_s = section.get("bw").and_then(|v| v.as_f64());
    let iops = section.get("iops").and_then(|v| v.as_f64());

    let mib_s = bw_kib_s.map(|bw| (bw / 1024.0).round().max(0.0) as u64);
    let iops = iops.map(|v| v.round().max(0.0) as u64);
    (mib_s.filter(|v| *v > 0), iops.filter(|v| *v > 0))
}

fn internal_budgets(bench_type: BenchmarkType) -> (Duration, Duration, Duration, Duration, u64) {
    match bench_type {
        BenchmarkType::Quick => (
            Duration::from_secs(2),
            Duration::from_secs(1),
            Duration::from_secs(3),
            Duration::from_secs(3),
            256,
        ),
        BenchmarkType::Standard => (
            Duration::from_secs(5),
            Duration::from_secs(3),
            Duration::from_secs(6),
            Duration::from_secs(6),
            256,
        ),
        BenchmarkType::Extended => (
            Duration::from_secs(10),
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_secs(10),
            256,
        ),
    }
}

fn internal_cpu_score(budget: Duration) -> Result<u64> {
    let start = Instant::now();
    let mut x: u64 = 0x1234_5678_9abc_def0;
    let mut iters: u64 = 0;

    while start.elapsed() < budget {
        // Mixed integer ops + branches to approximate typical game/engine workloads.
        x ^= x >> 12;
        x = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        x ^= x << 25;
        x = x.rotate_left(17);
        if x & 1 == 0 {
            x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        } else {
            x = x.wrapping_sub(0x517c_c1b7_2722_0a95);
        }
        iters = iters.wrapping_add(1);
    }

    black_box(x);

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    let iters_per_sec = (iters as f64 / elapsed).max(1.0);

    // Calibrate to a WinSAT-like range (~200-2000) across typical desktops.
    let score = (iters_per_sec / 50_000.0).round() as u64;
    Ok(score.clamp(1, 10_000))
}

fn internal_ram_score(budget: Duration) -> Result<u64> {
    let size_bytes: usize = 128 * 1024 * 1024; // 128 MiB working set
    let mut src = vec![0u8; size_bytes];
    let mut dst = vec![0u8; size_bytes];
    for (i, b) in src.iter_mut().enumerate().step_by(4096) {
        *b = (i as u8).wrapping_mul(31).wrapping_add(7);
    }

    let start = Instant::now();
    let mut bytes: u64 = 0;
    let mut checksum: u64 = 0;
    while start.elapsed() < budget {
        dst.copy_from_slice(&src);
        checksum ^= dst[0] as u64;
        bytes = bytes.wrapping_add(size_bytes as u64);
    }
    black_box(checksum);

    let mib_s = (bytes as f64 / (1024.0 * 1024.0)) / start.elapsed().as_secs_f64().max(0.001);
    let score = (mib_s / 20.0).round() as u64;
    Ok(score.clamp(1, 10_000))
}

fn internal_disk_score(_budget: Duration, file_mb: u64) -> Result<u64> {
    let path = std::env::temp_dir().join(format!(
        "fps-tracker-disk-bench-{}.bin",
        uuid::Uuid::new_v4()
    ));

    let file_size = (file_mb as usize) * 1024 * 1024;
    let chunk = vec![0xa5u8; 1024 * 1024];

    // Write a fixed-size file to measure sequential read throughput.
    {
        let mut file =
            std::fs::File::create(&path).context("Failed to create disk benchmark file")?;
        let mut written: usize = 0;
        while written < file_size {
            let remaining = file_size - written;
            let to_write = remaining.min(chunk.len());
            file.write_all(&chunk[..to_write])
                .context("Failed writing disk benchmark file")?;
            written += to_write;
        }
        file.sync_data().ok();
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(mib_s) = windows_unbuffered_seq_read_mib_s(&path, file_size as u64) {
            let _ = std::fs::remove_file(&path);
            let score = (mib_s as f64 / 5.0).round() as u64;
            return Ok(score.clamp(1, 10_000));
        }
    }

    let read_start = Instant::now();
    let mut file = std::fs::File::open(&path).context("Failed to open disk benchmark file")?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut read_total: u64 = 0;
    loop {
        let n = file.read(&mut buf).context("Disk benchmark read failed")?;
        if n == 0 {
            break;
        }
        read_total += n as u64;
    }
    let elapsed = read_start.elapsed().as_secs_f64().max(0.001);
    let mib_s = (read_total as f64 / (1024.0 * 1024.0)) / elapsed;

    let _ = std::fs::remove_file(&path);

    let score = (mib_s / 5.0).round() as u64;
    Ok(score.clamp(1, 10_000))
}

#[cfg(target_os = "windows")]
fn windows_unbuffered_seq_read_mib_s(path: &std::path::Path, file_bytes: u64) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        anyhow::bail!("CreateFileW failed for unbuffered sequential read");
    }

    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = HandleGuard(handle);

    struct AlignedBuf {
        ptr: *mut u8,
        len: usize,
        align: usize,
    }
    impl AlignedBuf {
        fn new(len: usize, align: usize) -> Result<Self> {
            use std::alloc::{alloc_zeroed, Layout};

            let layout = Layout::from_size_align(len, align)
                .map_err(|_| anyhow::anyhow!("invalid alignment layout"))?;
            let ptr = unsafe { alloc_zeroed(layout) };
            if ptr.is_null() {
                anyhow::bail!("aligned allocation failed");
            }
            Ok(Self { ptr, len, align })
        }

        fn as_mut_ptr(&mut self) -> *mut u8 {
            self.ptr
        }
    }
    impl Drop for AlignedBuf {
        fn drop(&mut self) {
            use std::alloc::{dealloc, Layout};

            if self.ptr.is_null() {
                return;
            }
            if let Ok(layout) = Layout::from_size_align(self.len, self.align) {
                unsafe { dealloc(self.ptr, layout) };
            }
        }
    }

    let chunk: u32 = 256 * 1024; // 4 KiB aligned chunk for FILE_FLAG_NO_BUFFERING.
    let mut buf = AlignedBuf::new(chunk as usize, 4096)?;

    let start = Instant::now();
    let mut total: u64 = 0;
    while total < file_bytes {
        let mut read: u32 = 0;
        let ok = unsafe {
            ReadFile(
                handle,
                buf.as_mut_ptr() as _,
                chunk,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if read < chunk {
            break;
        }
    }

    if total == 0 {
        anyhow::bail!("unbuffered disk read returned no data");
    }

    black_box(total);
    let elapsed = start.elapsed().as_secs_f64().max(0.000_001);
    let mib_s = ((total as f64) / (1024.0 * 1024.0) / elapsed)
        .round()
        .max(1.0) as u64;
    Ok(mib_s)
}

fn run_command_capture_with_timeout(
    command: &mut Command,
    timeout: Duration,
    label: &str,
) -> Result<std::process::Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to launch {label}"))?;

    let stdout_handle = child.stdout.take().map(|mut stream| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stream, &mut buf);
            buf
        })
    });
    let stderr_handle = child.stderr.take().map(|mut stream| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stream, &mut buf);
            buf
        })
    });

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("Failed while waiting on {label}"))?
        {
            break status;
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(handle) = stdout_handle {
                let _ = handle.join();
            }
            if let Some(handle) = stderr_handle {
                let _ = handle.join();
            }
            anyhow::bail!("{label} timed out after {}s", timeout.as_secs());
        }

        std::thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr = stderr_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "linux")]
fn linux_gpu_score(budget: Duration) -> Result<u64> {
    if !is_tool_available("glmark2") {
        anyhow::bail!("glmark2 is not installed (optional GPU synthetic).");
    }

    // Use a deterministic scene. The timeout is a hang safety guard only; it is set generously
    // so we don't truncate "normal" runs (slow shader compilation / first-run caches).
    let timeout = Duration::from_secs(240).max(budget + Duration::from_secs(180));

    let primary_args = ["--off-screen", "--benchmark", "build"];
    let fallback_args = ["--benchmark", "build"];

    let run = |args: &[&str], label: &str| -> Result<std::process::Output> {
        let mut cmd = Command::new("glmark2");
        cmd.args(args);
        run_command_capture_with_timeout(&mut cmd, timeout, label).context("Failed to run glmark2")
    };

    let mut output = run(&primary_args, "glmark2")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_ascii_lowercase();

        // Some distro builds don't ship --off-screen. If that's the only issue, retry once without it.
        let offscreen_unsupported = stderr_lower.contains("off-screen")
            && (stderr_lower.contains("unknown option")
                || stderr_lower.contains("unrecognized option")
                || stderr_lower.contains("invalid option"));
        if offscreen_unsupported {
            output = run(&fallback_args, "glmark2 (fallback)")?;
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("glmark2 failed ({}): {}", output.status, stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let score = parse_glmark2_score(&stdout)
        .ok_or_else(|| anyhow::anyhow!("Could not parse glmark2 score from output"))?;

    Ok(score.clamp(1, 1_000_000))
}

#[cfg(any(test, target_os = "linux"))]
fn parse_glmark2_score(stdout: &str) -> Option<u64> {
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("glmark2 Score:") {
            if let Ok(raw) = rest.trim().parse::<u64>() {
                if raw > 0 {
                    return Some(raw);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn run_windows_benchmarks(
    bench_type: BenchmarkType,
    results: &mut BenchmarkResults,
    options: &BenchmarkRunOptions,
) -> Result<()> {
    let elevated = is_windows_process_elevated();
    let winsat_available = is_windows_command_available("winsat");
    let powershell_available = is_windows_powershell_available();

    // WinSAT is best-effort. When it can't run, we still return partial synthetic metrics
    // (and avoid inventing component scores).
    if !elevated {
        results.winsat_note = Some(
            "WinSAT skipped: fps-tracker is not running as Administrator (run elevated to capture WinSAT scores)."
                .to_string(),
        );
    } else if !winsat_available {
        results.winsat_note = Some(
            "WinSAT skipped: winsat command not found (Windows system assessment tool missing/unavailable)."
                .to_string(),
        );
    } else if !powershell_available {
        results.winsat_note = Some(
            "WinSAT skipped: PowerShell not available in PATH (required to query Win32_WinSAT scores)."
                .to_string(),
        );
    } else {
        let winsat_outcome: Result<()> = (|| {
            let total_steps = 4;
            render_progress(
                0,
                total_steps,
                "Launching WinSAT formal run (CPU/RAM/SSD/GPU)",
                options,
            );

            let spawn_result = Command::new("winsat")
                .arg("formal")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            let mut child = match spawn_result {
                Ok(child) => Some(child),
                Err(err) => {
                    if err.raw_os_error() == Some(740) {
                        results.winsat_note = Some(
                            "WinSAT skipped: Windows required elevation for winsat on this system (run fps-tracker as Administrator)."
                                .to_string(),
                        );
                        None
                    } else {
                        results.winsat_note =
                            Some(format!("WinSAT skipped: failed to launch winsat: {err}"));
                        None
                    }
                }
            };

            let Some(ref mut child) = child else {
                return Ok(());
            };
            let (stdout_rx, stdout_handle) = if let Some(stdout) = child.stdout.take() {
                let (tx, rx) = mpsc::channel::<String>();
                let handle = std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(std::result::Result::ok) {
                        let _ = tx.send(line);
                    }
                });
                (Some(rx), Some(handle))
            } else {
                (None, None)
            };
            let (stderr_rx, stderr_handle) = if let Some(stderr) = child.stderr.take() {
                let (tx, rx) = mpsc::channel::<String>();
                let handle = std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(std::result::Result::ok) {
                        let _ = tx.send(line);
                    }
                });
                (Some(rx), Some(handle))
            } else {
                (None, None)
            };

            let mut completed: [bool; 4] = [false, false, false, false];
            let mut winsat_stdout = String::new();
            let mut winsat_stderr = String::new();
            let winsat_timeout = Duration::from_secs(1_200);
            let wait_start = Instant::now();
            let status = loop {
                if let Some(rx) = stdout_rx.as_ref() {
                    while let Ok(line) = rx.try_recv() {
                        winsat_stdout.push_str(&line);
                        winsat_stdout.push('\n');
                        if let Some(index) = detect_windows_stage_index(&line) {
                            if !completed[index] {
                                completed[index] = true;
                                let stage_name = windows_stage_name(index);
                                render_progress(
                                    index + 1,
                                    total_steps,
                                    &format!("Testing {stage_name}"),
                                    options,
                                );
                            }
                        }
                    }
                }
                if let Some(rx) = stderr_rx.as_ref() {
                    while let Ok(line) = rx.try_recv() {
                        winsat_stderr.push_str(&line);
                        winsat_stderr.push('\n');
                        if let Some(index) = detect_windows_stage_index(&line) {
                            if !completed[index] {
                                completed[index] = true;
                                let stage_name = windows_stage_name(index);
                                render_progress(
                                    index + 1,
                                    total_steps,
                                    &format!("Testing {stage_name}"),
                                    options,
                                );
                            }
                        }
                    }
                }

                if let Some(status) = child
                    .try_wait()
                    .context("Failed while waiting for WinSAT completion")?
                {
                    break status;
                }

                if wait_start.elapsed() >= winsat_timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    results.winsat_note = Some(format!(
                        "WinSAT formal timed out after {}s. Continuing with other synthetic metrics.",
                        winsat_timeout.as_secs()
                    ));
                    return Ok(());
                }

                std::thread::sleep(Duration::from_millis(200));
            };

            if let Some(handle) = stdout_handle {
                let _ = handle.join();
            }
            if let Some(handle) = stderr_handle {
                let _ = handle.join();
            }
            if let Some(rx) = stdout_rx {
                while let Ok(line) = rx.try_recv() {
                    winsat_stdout.push_str(&line);
                    winsat_stdout.push('\n');
                    if let Some(index) = detect_windows_stage_index(&line) {
                        if !completed[index] {
                            completed[index] = true;
                            let stage_name = windows_stage_name(index);
                            render_progress(
                                index + 1,
                                total_steps,
                                &format!("Testing {stage_name}"),
                                options,
                            );
                        }
                    }
                }
            }
            if let Some(rx) = stderr_rx {
                while let Ok(line) = rx.try_recv() {
                    winsat_stderr.push_str(&line);
                    winsat_stderr.push('\n');
                    if let Some(index) = detect_windows_stage_index(&line) {
                        if !completed[index] {
                            completed[index] = true;
                            let stage_name = windows_stage_name(index);
                            render_progress(
                                index + 1,
                                total_steps,
                                &format!("Testing {stage_name}"),
                                options,
                            );
                        }
                    }
                }
            }

            if !status.success() {
                results.winsat_note = Some(format!(
                    "WinSAT formal failed ({}). {}",
                    status,
                    if winsat_stderr.trim().is_empty() {
                        "Continuing with other synthetic metrics."
                    } else {
                        winsat_stderr.trim()
                    }
                ));
                return Ok(());
            }

            if !winsat_stdout.trim().is_empty() {
                for line in winsat_stdout.lines() {
                    if let Some(index) = detect_windows_stage_index(line) {
                        if !completed[index] {
                            completed[index] = true;
                            let stage_name = windows_stage_name(index);
                            render_progress(
                                index + 1,
                                total_steps,
                                &format!("Testing {stage_name}"),
                                options,
                            );
                        }
                    }
                }
            }

            let scores = match query_windows_winsat_scores() {
                Ok(scores) => scores,
                Err(err) => {
                    results.winsat_note = Some(format!(
                        "WinSAT completed but score retrieval failed: {err}"
                    ));
                    return Ok(());
                }
            };

            results.cpu_score = scores.cpu_score;
            results.ram_score = scores.memory_score;
            results.disk_score = scores.disk_score;
            results.gpu_score = scores.graphics_score.or(scores.d3d_score);
            if results.cpu_score.is_some() {
                results.cpu_score_source = Some("winsat".to_string());
            }
            if results.ram_score.is_some() {
                results.ram_score_source = Some("winsat".to_string());
            }
            if results.disk_score.is_some() {
                results.disk_score_source = Some("winsat".to_string());
            }
            if results.gpu_score.is_some() {
                results.gpu_score_source = Some("winsat".to_string());
            }

            if !options.quiet {
                print_component_score("CPU", results.cpu_score);
                print_component_score("RAM", results.ram_score);
                print_component_score("SSD/Disk", results.disk_score);
                print_component_score("GPU", results.gpu_score);
            }

            if results.cpu_score.is_none()
                && results.ram_score.is_none()
                && results.disk_score.is_none()
                && results.gpu_score.is_none()
            {
                results.winsat_note = Some(
                    "WinSAT finished but no component scores were available. Continuing with other synthetic metrics."
                        .to_string(),
                );
            }

            Ok(())
        })();

        if let Err(err) = winsat_outcome {
            results.winsat_note = Some(format!(
                "WinSAT failed: {err}. Continuing with other synthetic metrics."
            ));
        }
    }

    // Optional Windows-only tools (best-effort).
    if !matches!(bench_type, BenchmarkType::Quick) {
        if is_tool_available("7z") {
            match run_windows_7zip_benchmark_mips(Some(1)) {
                Ok(mips) => results.cpu_7z_single_mips = Some(mips),
                Err(err) => warn(options, format!("7-Zip benchmark failed: {err}")),
            }
            match run_windows_7zip_benchmark_mips(None) {
                Ok(mips) => results.cpu_7z_multi_mips = Some(mips),
                Err(err) => warn(options, format!("7-Zip benchmark failed: {err}")),
            }
        }

        if let Some(diskspd) = crate::deps::locate_diskspd_executable() {
            match run_windows_diskspd_seq_mb_s(&diskspd) {
                Ok((read_mb_s, write_mb_s)) => {
                    results.diskspd_read_mb_s = read_mb_s;
                    results.diskspd_write_mb_s = write_mb_s;
                }
                Err(err) => warn(options, format!("DiskSpd benchmark failed: {err}")),
            }
        }

        if let Some(blender) = crate::deps::locate_blender_executable() {
            match run_windows_blender_cpu_render(&blender, bench_type) {
                Ok((render_ms, settings)) => {
                    results.blender_cpu_render_ms = Some(render_ms);
                    results.blender_cpu_render_settings = Some(settings);
                }
                Err(err) => warn(options, format!("Blender render benchmark failed: {err}")),
            }
        }
    }

    // Fill missing component scores (never fabricate values).
    let (cpu_budget, ram_budget, disk_budget, _gpu_budget, disk_mb) = internal_budgets(bench_type);

    if results.cpu_score.is_none() {
        if let Some(mips) = results.cpu_7z_multi_mips.or(results.cpu_7z_single_mips) {
            results.cpu_score = Some(mips);
            results.cpu_score_source = Some("7z_mips".to_string());
        } else if let Ok(score) = internal_cpu_score(cpu_budget) {
            results.cpu_score = Some(score);
            results.cpu_score_source = Some("internal".to_string());
        }
    }

    if results.ram_score.is_none() {
        if let Ok(score) = internal_ram_score(ram_budget) {
            results.ram_score = Some(score);
            results.ram_score_source = Some("internal".to_string());
        }
    }

    if results.disk_score.is_none() {
        if let Some(read) = results.diskspd_read_mb_s {
            results.disk_score = Some(read);
            results.disk_score_source = Some("diskspd_read_mib_s".to_string());
        } else if let Ok(score) = internal_disk_score(disk_budget, disk_mb) {
            results.disk_score = Some(score);
            results.disk_score_source = Some("internal".to_string());
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct WinSatScores {
    cpu_score: Option<u64>,
    memory_score: Option<u64>,
    disk_score: Option<u64>,
    d3d_score: Option<u64>,
    graphics_score: Option<u64>,
}

#[cfg(target_os = "windows")]
fn query_windows_winsat_scores() -> Result<WinSatScores> {
    let script = "$row = Get-CimInstance -ClassName Win32_WinSAT -ErrorAction Stop | Select-Object CPUScore,MemoryScore,DiskScore,D3DScore,GraphicsScore,WinSATAssessmentState; $row | ConvertTo-Json -Compress";

    let output = run_windows_powershell(script)
        .context("Failed to execute PowerShell Win32_WinSAT query")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("PowerShell Win32_WinSAT query failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(stdout.trim())
        .context("Failed to parse Win32_WinSAT JSON response")?;

    let object = if let Some(array) = value.as_array() {
        array
            .first()
            .ok_or_else(|| anyhow::anyhow!("Win32_WinSAT query returned no rows"))?
    } else {
        &value
    };

    if let Some(state) = object
        .get("WinSATAssessmentState")
        .and_then(|value| value.as_u64())
    {
        if state != 1 {
            anyhow::bail!(
                "WinSAT assessment state is {state} (expected 1 = Valid). Run `winsat formal` as Administrator and retry."
            );
        }
    }

    Ok(WinSatScores {
        cpu_score: parse_winsat_score(object, "CPUScore"),
        memory_score: parse_winsat_score(object, "MemoryScore"),
        disk_score: parse_winsat_score(object, "DiskScore"),
        d3d_score: parse_winsat_score(object, "D3DScore"),
        graphics_score: parse_winsat_score(object, "GraphicsScore"),
    })
}

#[cfg(target_os = "windows")]
fn parse_winsat_score(value: &Value, key: &str) -> Option<u64> {
    let raw = value.get(key)?.as_f64()?;
    if !raw.is_finite() || raw <= 0.0 {
        return None;
    }
    Some((raw * 100.0).round() as u64)
}

#[cfg(target_os = "windows")]
fn run_windows_powershell(script: &str) -> Result<std::process::Output> {
    for shell in ["pwsh", "powershell"] {
        let mut cmd = Command::new(shell);
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        let result = run_command_capture_with_timeout(
            &mut cmd,
            Duration::from_secs(45),
            &format!("{shell} command"),
        );
        if let Ok(output) = result {
            return Ok(output);
        }
    }

    anyhow::bail!("Neither pwsh nor powershell is available in PATH")
}

#[cfg(any(test, target_os = "windows"))]
fn detect_windows_stage_index(line: &str) -> Option<usize> {
    let lower = line.to_ascii_lowercase();

    if lower.contains("cpu") {
        return Some(0);
    }
    if lower.contains("memory") || lower.contains(" mem") || lower.starts_with("mem") {
        return Some(1);
    }
    if lower.contains("disk") || lower.contains("drive") || lower.contains("storage") {
        return Some(2);
    }
    if lower.contains("d3d")
        || lower.contains("graphics")
        || lower.contains("direct3d")
        || lower.contains("video")
    {
        return Some(3);
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_stage_name(index: usize) -> &'static str {
    match index {
        0 => "CPU",
        1 => "RAM",
        2 => "SSD/Disk",
        3 => "GPU",
        _ => "Unknown",
    }
}

#[cfg(target_os = "windows")]
fn warn(options: &BenchmarkRunOptions, message: String) {
    if options.quiet {
        return;
    }
    eprintln!("WARN: {message}");
}

#[cfg(target_os = "windows")]
fn is_windows_process_elevated() -> bool {
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
fn run_windows_7zip_benchmark_mips(threads: Option<u32>) -> Result<u64> {
    let mmt_arg = match threads {
        Some(count) => format!("-mmt={count}"),
        None => "-mmt=on".to_string(),
    };

    let mut cmd = Command::new("7z");
    cmd.args(["b", mmt_arg.as_str()]);
    let output = run_command_capture_with_timeout(&mut cmd, Duration::from_secs(240), "7z b")
        .context("Failed to run 7z benchmark (7z b)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("7z benchmark failed ({}): {}", output.status, stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_7z_total_mips(&stdout).ok_or_else(|| {
        anyhow::anyhow!("Unable to parse 7-Zip benchmark output (missing Tot/Total Rating)")
    })
}

#[cfg(any(test, target_os = "windows"))]
fn parse_7z_total_mips(output: &str) -> Option<u64> {
    let mut best: Option<u64> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("tot:") {
            // Typical: "Tot:  7645  7653  7649  100%"
            let rest = trimmed.split_once(':')?.1.trim();
            let nums: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|token| token.parse::<u64>().ok())
                .collect();
            if nums.len() >= 3 {
                best = Some(nums[2]);
                continue;
            }
            if let Some(last) = nums.last().copied() {
                best = Some(last);
                continue;
            }
        }

        if lower.contains("total") && lower.contains("rating") {
            // Some builds print: "Total Rating: 12345"
            if let Some(num) = trimmed
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<u64>().ok())
                .next_back()
            {
                best = Some(num);
            }
        }
    }

    best
}

#[cfg(target_os = "windows")]
fn run_windows_diskspd_seq_mb_s(diskspd: &std::path::Path) -> Result<(Option<u64>, Option<u64>)> {
    let temp_file = std::env::temp_dir().join("fps-tracker-diskspd.dat");
    let temp_file_str = temp_file.to_string_lossy().to_string();

    let mut read: Option<u64> = None;
    let mut write: Option<u64> = None;

    if let Ok(mb_s) = run_diskspd_once(diskspd, &temp_file_str, 0) {
        read = Some(mb_s);
    }
    let _ = std::fs::remove_file(&temp_file);

    if let Ok(mb_s) = run_diskspd_once(diskspd, &temp_file_str, 100) {
        write = Some(mb_s);
    }

    let _ = std::fs::remove_file(&temp_file);

    if read.is_none() && write.is_none() {
        anyhow::bail!("DiskSpd ran but no totals could be parsed");
    }

    Ok((read, write))
}

#[cfg(target_os = "windows")]
fn run_diskspd_once(diskspd: &std::path::Path, path: &str, write_percent: u32) -> Result<u64> {
    let write_arg = format!("-w{write_percent}");
    let mut cmd = Command::new(diskspd);
    cmd.args([
        "-c256M",
        "-d8",
        "-W2",
        "-b1M",
        "-t1",
        "-o1",
        "-Sh",
        write_arg.as_str(),
        path,
    ]);
    let output = run_command_capture_with_timeout(&mut cmd, Duration::from_secs(180), "diskspd")
        .context("Failed to run diskspd")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("diskspd failed ({}): {}", output.status, stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_diskspd_total_mb_s(&stdout)
        .ok_or_else(|| anyhow::anyhow!("Unable to parse diskspd total MB/s from output"))
}

#[cfg(any(test, target_os = "windows"))]
fn parse_diskspd_total_mb_s(output: &str) -> Option<u64> {
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("total:") {
            continue;
        }

        let cols: Vec<&str> = trimmed.split('|').collect();
        if cols.len() < 3 {
            continue;
        }
        let mb_s_text = cols[2].trim();
        let number_text = mb_s_text.split_whitespace().next().unwrap_or("").trim();
        if let Ok(value) = number_text.parse::<f64>() {
            if value.is_finite() && value > 0.0 {
                return Some(value.round() as u64);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn run_windows_blender_cpu_render(
    blender: &std::path::Path,
    bench_type: BenchmarkType,
) -> Result<(u64, String)> {
    let (width, height, samples) = match bench_type {
        BenchmarkType::Quick => (1280u32, 720u32, 32u32),
        BenchmarkType::Standard => (1280u32, 720u32, 64u32),
        BenchmarkType::Extended => (1920u32, 1080u32, 64u32),
    };

    let output_path = std::env::temp_dir().join("fps-tracker-blender-render.png");
    let output_path = output_path.to_string_lossy();

    let script = format!(
        r#"
import bpy, time, json, random

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = {samples}
scene.render.resolution_x = {width}
scene.render.resolution_y = {height}
scene.render.resolution_percentage = 100
scene.render.filepath = r"{output_path}"

cam_data = bpy.data.cameras.new("Camera")
cam = bpy.data.objects.new("Camera", cam_data)
scene.collection.objects.link(cam)
scene.camera = cam
cam.location = (8, -8, 6)
cam.rotation_euler = (1.1, 0.0, 0.8)

light_data = bpy.data.lights.new(name="Light", type='AREA')
light = bpy.data.objects.new(name="Light", object_data=light_data)
scene.collection.objects.link(light)
light.location = (4, -4, 6)
light_data.energy = 1500.0

bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, 0))

random.seed(1337)
for i in range(220):
    x = random.uniform(-8.0, 8.0)
    y = random.uniform(-8.0, 8.0)
    z = random.uniform(0.2, 4.0)
    if i % 3 == 0:
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=random.uniform(0.15, 0.6), location=(x,y,z))
    elif i % 3 == 1:
        bpy.ops.mesh.primitive_torus_add(major_radius=random.uniform(0.25, 0.9), minor_radius=random.uniform(0.05, 0.25), location=(x,y,z))
    else:
        bpy.ops.mesh.primitive_cone_add(radius1=random.uniform(0.2, 0.7), depth=random.uniform(0.4, 1.6), location=(x,y,z))

t0 = time.perf_counter()
bpy.ops.render.render(write_still=True)
t1 = time.perf_counter()

payload = {{
  "render_ms": int(round((t1 - t0) * 1000.0)),
  "width": {width},
  "height": {height},
  "samples": {samples},
  "device": "CPU"
}}
print("FPS_TRACKER_BLENDER_JSON:" + json.dumps(payload))
"#
    );

    let mut cmd = Command::new(blender);
    cmd.args([
        "--background",
        "--factory-startup",
        "--python-expr",
        &script,
    ]);
    let timeout = match bench_type {
        BenchmarkType::Quick => Duration::from_secs(240),
        BenchmarkType::Standard => Duration::from_secs(360),
        BenchmarkType::Extended => Duration::from_secs(540),
    };
    let output = run_command_capture_with_timeout(&mut cmd, timeout, "blender render")
        .context("Failed to execute Blender CLI render")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Blender render failed ({}): {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_blender_json_line(&stdout).ok_or_else(|| {
        anyhow::anyhow!("Blender ran but no FPS_TRACKER_BLENDER_JSON line was found")
    })?;

    let render_ms = parsed
        .get("render_ms")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Blender JSON missing render_ms"))?;
    let width = parsed
        .get("width")
        .and_then(|v| v.as_u64())
        .unwrap_or(width as u64);
    let height = parsed
        .get("height")
        .and_then(|v| v.as_u64())
        .unwrap_or(height as u64);
    let samples = parsed
        .get("samples")
        .and_then(|v| v.as_u64())
        .unwrap_or(samples as u64);

    let settings = format!("Blender CPU render: {width}x{height}, {samples} samples");
    Ok((render_ms, settings))
}

#[cfg(any(test, target_os = "windows"))]
fn parse_blender_json_line(stdout: &str) -> Option<serde_json::Value> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        let prefix = "FPS_TRACKER_BLENDER_JSON:";
        if let Some(json_text) = trimmed.strip_prefix(prefix) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_text.trim()) {
                return Some(value);
            }
        }
    }
    None
}

fn render_progress(
    completed_steps: usize,
    total_steps: usize,
    status: &str,
    options: &BenchmarkRunOptions,
) {
    if let Some(on_progress) = options.progress.as_ref() {
        on_progress(BenchmarkProgressUpdate {
            completed_steps: completed_steps.min(total_steps),
            total_steps,
            status: status.to_string(),
        });
    }

    if options.quiet {
        return;
    }
    if total_steps == 0 {
        println!("   {}", status.bright_white());
        return;
    }

    let width = 26usize;
    let completed = completed_steps.min(total_steps);
    let filled = ((completed as f64 / total_steps as f64) * width as f64).round() as usize;
    let bar = format!(
        "{}{}",
        "#".repeat(filled),
        "-".repeat(width.saturating_sub(filled))
    );
    let percent = ((completed as f64 / total_steps as f64) * 100.0).round() as u64;

    println!(
        "   [{}] {:>3}% {}",
        bar.bright_cyan(),
        percent,
        status.bright_white()
    );
}

#[cfg(target_os = "windows")]
fn print_component_score(label: &str, score: Option<u64>) {
    match score {
        Some(value) => println!(
            "   {} {} score: {}",
            "✓".bright_green(),
            label.bright_white(),
            value.to_string().bright_cyan()
        ),
        None => println!(
            "   {} {} score unavailable",
            "⚠".bright_yellow(),
            label.bright_white()
        ),
    }
}

/// Run CPU benchmark using platform-specific tools (macOS and other non-Windows, non-Linux).
#[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
fn run_cpu_benchmark(bench_type: BenchmarkType) -> Result<u64> {
    if !is_tool_available("sysbench") {
        anyhow::bail!("sysbench is not installed (optional CPU synthetic).");
    }

    let secs = bench_type.duration().as_secs().clamp(5, 30);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 64);

    let output = Command::new("sysbench")
        .args([
            "cpu",
            "--cpu-max-prime=20000",
            &format!("--threads={threads}"),
            &format!("--time={secs}"),
            "run",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run sysbench cpu")?;

    if !output.status.success() {
        anyhow::bail!("sysbench cpu failed ({})", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.split("events per second:").nth(1) {
            if let Ok(parsed) = rest.trim().parse::<f64>() {
                if parsed.is_finite() && parsed > 0.0 {
                    // Map to the same WinSAT-ish range used elsewhere.
                    let score = (parsed / 20.0).round() as u64;
                    return Ok(score.clamp(1, 10_000));
                }
            }
        }
    }

    anyhow::bail!("Could not parse sysbench cpu events/sec from output");
}

/// Run GPU benchmark using platform-specific tools (macOS and other non-Windows, non-Linux).
#[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
fn run_gpu_benchmark(_bench_type: BenchmarkType) -> Result<u64> {
    if !is_tool_available("glmark2") {
        anyhow::bail!("glmark2 is not installed (optional GPU synthetic).");
    }

    let output = Command::new("glmark2")
        .arg("--off-screen")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run glmark2")?;

    if !output.status.success() {
        anyhow::bail!("glmark2 failed ({})", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("glmark2 Score:") {
            if let Ok(raw) = rest.trim().parse::<u64>() {
                let score = (raw / 5).max(1);
                return Ok(score.clamp(1, 10_000));
            }
        }
    }

    anyhow::bail!("Could not parse glmark2 score from output");
}

/// Get peak GPU clocks during benchmark
fn get_peak_gpu_clocks() -> Result<(Option<u64>, Option<u64>)> {
    let mut max_gpu_clock: Option<u64> = None;
    let mut max_mem_clock: Option<u64> = None;

    // Sample GPU clocks multiple times and return the peak
    for _ in 0..5 {
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("nvidia-smi")
                .args([
                    "--query-gpu=clocks.gr,clocks.mem",
                    "--format=csv,noheader,nounits",
                ])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = stdout.lines().next() {
                        let parts: Vec<&str> = line.split(", ").collect();
                        if parts.len() >= 2 {
                            if let Ok(gpu) = parts[0].trim().replace(" MHz", "").parse::<u64>() {
                                max_gpu_clock = Some(max_gpu_clock.map_or(gpu, |m| m.max(gpu)));
                            }
                            if let Ok(mem) = parts[1].trim().replace(" MHz", "").parse::<u64>() {
                                max_mem_clock = Some(max_mem_clock.map_or(mem, |m| m.max(mem)));
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("nvidia-smi")
                .args([
                    "--query-gpu=clocks.gr,clocks.mem",
                    "--format=csv,noheader,nounits",
                ])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = stdout.lines().next() {
                        let parts: Vec<&str> = line.split(", ").collect();
                        if parts.len() >= 2 {
                            if let Ok(gpu) = parts[0].trim().replace(" MHz", "").parse::<u64>() {
                                max_gpu_clock = Some(max_gpu_clock.map_or(gpu, |m| m.max(gpu)));
                            }
                            if let Ok(mem) = parts[1].trim().replace(" MHz", "").parse::<u64>() {
                                max_mem_clock = Some(max_mem_clock.map_or(mem, |m| m.max(mem)));
                            }
                        }
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    Ok((max_gpu_clock, max_mem_clock))
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{} seconds", secs)
    } else {
        format!("{} minutes", secs / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_tool_availability() {
        let tools = check_benchmark_tools();
        // Just make sure it doesn't panic
        assert!(!tools.is_empty());
    }

    #[test]
    fn progress_callback_emits_events_in_quiet_mode() {
        let events: Arc<Mutex<Vec<BenchmarkProgressUpdate>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let options = BenchmarkRunOptions {
            quiet: true,
            progress: Some(Arc::new(move |update| {
                let mut guard = sink.lock().expect("progress sink lock poisoned");
                guard.push(update);
            })),
        };

        render_progress(2, 4, "Disk benchmark", &options);

        let guard = events.lock().expect("progress sink lock poisoned");
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].completed_steps, 2);
        assert_eq!(guard[0].total_steps, 4);
        assert_eq!(guard[0].status, "Disk benchmark");
    }

    #[test]
    fn windows_stage_detection_maps_known_lines() {
        assert_eq!(
            detect_windows_stage_index("Running CPU assessment"),
            Some(0)
        );
        assert_eq!(
            detect_windows_stage_index("Memory performance test"),
            Some(1)
        );
        assert_eq!(detect_windows_stage_index("Disk sequential test"), Some(2));
        assert_eq!(detect_windows_stage_index("Direct3D graphics"), Some(3));
        assert_eq!(detect_windows_stage_index("some unrelated line"), None);
    }

    #[test]
    fn parse_7z_total_prefers_tot_line() {
        let sample = r#"
Some header
Tot:  7645  7653  7649  100%
Tail
"#;
        assert_eq!(parse_7z_total_mips(sample), Some(7649));
    }

    #[test]
    fn parse_diskspd_total_mb_s_extracts_total_row() {
        let sample = r#"
total:     |     0.00 |   1234.56 |     789.00 |
"#;
        assert_eq!(parse_diskspd_total_mb_s(sample), Some(1235));
    }

    #[test]
    fn parse_blender_json_line_extracts_payload() {
        let sample = r#"
log line
FPS_TRACKER_BLENDER_JSON:{"render_ms":1234,"width":1280,"height":720}
more log
"#;
        let parsed = parse_blender_json_line(sample).expect("blender json line should parse");
        assert_eq!(parsed.get("render_ms").and_then(|v| v.as_u64()), Some(1234));
    }

    #[test]
    fn parse_glmark2_score_extracts_raw_value() {
        let sample = r#"
glmark2 2023.07
=======================================================
    glmark2 Score: 5168
"#;
        assert_eq!(parse_glmark2_score(sample), Some(5168));
    }
}
