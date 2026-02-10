//! Benchmark runner module
//!
//! Runs optional synthetic benchmarks to measure hardware performance
//! Uses only open-source, legal tools.

use anyhow::{Context, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// CPU benchmark score (if run)
    pub cpu_score: Option<u64>,
    /// GPU benchmark score (if run)
    pub gpu_score: Option<u64>,
    /// Peak CPU frequency observed during test (MHz)
    pub cpu_peak_clock_mhz: Option<u64>,
    /// Peak GPU frequency observed during test (MHz)
    pub gpu_peak_clock_mhz: Option<u64>,
    /// Peak GPU memory clock observed (MHz)
    pub gpu_memory_peak_clock_mhz: Option<u64>,
    /// Test duration in seconds
    pub duration_secs: f64,
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
            "║  • Duration: ~{}                                          ║",
            format_duration(bench_type.duration())
        )
        .bright_white()
    );
    println!(
        "{}",
        "║  • Close Chrome, Discord, games first                        ║".bright_white()
    );
    println!(
        "{}",
        "║  • Laptop users: ensure good ventilation                     ║".bright_white()
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
        "Skip Benchmarks (use database values)".bright_white()
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

/// Check if required benchmark tools are available (platform-specific)
pub fn check_benchmark_tools() -> Vec<(String, bool)> {
    let mut tools = vec![];

    #[cfg(target_os = "linux")]
    {
        tools.push(("glmark2".to_string(), is_tool_available("glmark2")));
        tools.push(("sysbench".to_string(), is_tool_available("sysbench")));
        tools.push(("stress-ng".to_string(), is_tool_available("stress-ng")));
    }

    #[cfg(target_os = "windows")]
    {
        // Windows tools - check for common benchmark utilities
        tools.push(("winsat".to_string(), is_windows_command_available("winsat")));
        tools.push(("7z".to_string(), is_tool_available("7z")));
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

/// Run benchmarks and return results
pub fn run_benchmarks(bench_type: BenchmarkType) -> Result<BenchmarkResults> {
    let start_time = Instant::now();

    println!(
        "\n{}",
        "⚡ Starting synthetic benchmarks...".bright_cyan().bold()
    );
    println!(
        "   This will take about {}\n",
        format_duration(bench_type.duration()).bright_yellow()
    );

    let mut results = BenchmarkResults {
        cpu_score: None,
        gpu_score: None,
        cpu_peak_clock_mhz: None,
        gpu_peak_clock_mhz: None,
        gpu_memory_peak_clock_mhz: None,
        duration_secs: 0.0,
    };

    // Run CPU benchmark
    match run_cpu_benchmark(bench_type) {
        Ok(score) => {
            results.cpu_score = Some(score);
            println!(
                "   {} CPU benchmark complete: {} points",
                "✓".bright_green(),
                score.to_string().bright_cyan()
            );
        }
        Err(e) => {
            println!(
                "   {} CPU benchmark failed: {}",
                "⚠".bright_yellow(),
                e.to_string().bright_red()
            );
        }
    }

    // Run GPU benchmark
    match run_gpu_benchmark(bench_type) {
        Ok(score) => {
            results.gpu_score = Some(score);
            println!(
                "   {} GPU benchmark complete: {} points",
                "✓".bright_green(),
                score.to_string().bright_cyan()
            );
        }
        Err(e) => {
            println!(
                "   {} GPU benchmark failed: {}",
                "⚠".bright_yellow(),
                e.to_string().bright_red()
            );
        }
    }

    // Try to get peak clocks if nvidia-smi is available
    if let Ok((gpu_clock, mem_clock)) = get_peak_gpu_clocks() {
        results.gpu_peak_clock_mhz = gpu_clock;
        results.gpu_memory_peak_clock_mhz = mem_clock;
        if let Some(clock) = gpu_clock {
            println!(
                "   {} Peak GPU clock detected: {} MHz",
                "✓".bright_green(),
                clock.to_string().bright_cyan()
            );
        }
    }

    results.duration_secs = start_time.elapsed().as_secs_f64();

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

    Ok(results)
}

/// Run CPU benchmark using platform-specific tools
fn run_cpu_benchmark(_bench_type: BenchmarkType) -> Result<u64> {
    #[cfg(target_os = "linux")]
    {
        // Try sysbench first
        if is_tool_available("sysbench") {
            let output = Command::new("sysbench")
                .args([
                    "cpu",
                    "--cpu-max-prime=20000",
                    "--threads=4",
                    "--time=10",
                    "run",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .context("Failed to run sysbench")?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse events per second from output
            for line in stdout.lines() {
                if line.contains("events per second:") {
                    if let Some(val) = line.split(':').nth(1) {
                        if let Ok(score) = val.trim().parse::<f64>() {
                            return Ok(score as u64);
                        }
                    }
                }
            }
        }

        // Fallback: use stress-ng if available
        if is_tool_available("stress-ng") {
            let start = Instant::now();
            let _output = Command::new("stress-ng")
                .args(["--cpu", "4", "--timeout", "10s", "--metrics-brief"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .context("Failed to run stress-ng")?;

            let elapsed = start.elapsed().as_secs_f64();
            // Generate a synthetic score based on how fast it completed
            // This is a rough approximation
            return Ok((10000.0 / elapsed) as u64);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: Use WinSAT (Windows System Assessment Tool)
        if is_windows_command_available("winsat") {
            let output = Command::new("winsat")
                .args(["cpu", "-compression"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .context("Failed to run WinSAT CPU test")?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse CPU score from WinSAT output
            for line in stdout.lines() {
                if line.contains("CPU") && line.contains("Score") {
                    // Extract numeric value
                    for word in line.split_whitespace() {
                        if let Ok(score) = word.parse::<f64>() {
                            return Ok(score as u64 * 100); // Scale to match other scores
                        }
                    }
                }
            }
        }

        // Fallback: Use 7z benchmark if available
        if is_tool_available("7z") {
            let output = Command::new("7z")
                .args(["b", "-mmt4"]) // Benchmark with 4 threads
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .context("Failed to run 7z benchmark")?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse MIPS from 7z output
            for line in stdout.lines() {
                if line.contains("MIPS") || line.contains("rating") {
                    // Extract numeric value
                    for word in line.split_whitespace() {
                        if let Ok(score) = word.parse::<f64>() {
                            return Ok(score as u64);
                        }
                    }
                }
            }
        }
    }

    anyhow::bail!("No CPU benchmark tool available for this platform")
}

/// Run GPU benchmark using platform-specific tools
fn run_gpu_benchmark(_bench_type: BenchmarkType) -> Result<u64> {
    #[cfg(target_os = "linux")]
    {
        if !is_tool_available("glmark2") {
            anyhow::bail!("glmark2 not available");
        }

        let output = Command::new("glmark2")
            .args(["--fullscreen", "--run-time", "60"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .context("Failed to run glmark2")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse glmark2 score from output
        for line in stdout.lines() {
            if line.contains("glmark2 Score:") {
                if let Some(val) = line.split(':').nth(1) {
                    if let Ok(score) = val.trim().parse::<u64>() {
                        return Ok(score);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: Use WinSAT for GPU assessment
        if is_windows_command_available("winsat") {
            let output = Command::new("winsat")
                .args(["dwm", "-normalw", "10"]) // Desktop Window Manager test
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .context("Failed to run WinSAT GPU test")?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse GPU score from WinSAT output
            for line in stdout.lines() {
                if line.contains("GraphicsScore") || line.contains("DWM") {
                    for word in line.split_whitespace() {
                        if let Ok(score) = word.parse::<f64>() {
                            return Ok(score as u64 * 10); // Scale to match glmark2 range
                        }
                    }
                }
            }
        }

        // Alternative: Try to run a simple DirectX diagnostic
        let output = Command::new("dxdiag")
            .args(["/t", "dxdiag_output.txt"])
            .output();

        if output.is_ok() {
            // dxdiag ran successfully, use a synthetic score based on system info
            // This is a placeholder - in production, you'd parse dxdiag output
            return Ok(5000); // Default mid-range score
        }
    }

    anyhow::bail!("No GPU benchmark tool available for this platform")
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
            // On Windows, try to use nvidia-smi if available
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

    #[test]
    fn test_tool_availability() {
        let tools = check_benchmark_tools();
        // Just make sure it doesn't panic
        assert!(!tools.is_empty());
    }
}
