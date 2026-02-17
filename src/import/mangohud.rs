//! MangoHud log parser
//!
//! MangoHud is the most popular Linux FPS overlay.
//! It can log frame times to CSV files.
//!
//! To enable logging in MangoHud:
//!   MANGOHUD_LOG=1 mangohud game
//! Or add to ~/.config/MangoHud/MangoHud.conf:
//!   log_duration=60
//!   output_folder=/path/to/logs
//!
//! Log format:
//!   fps,frametime,cpu_load,gpu_load,...
//! Where frametime is in milliseconds

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::common::FrameData;

fn parse_valid_frametime(value: &str) -> Option<f64> {
    let ms = value.trim().parse::<f64>().ok()?;
    if ms > 0.0 && ms <= 10_000.0 {
        Some(ms)
    } else {
        None
    }
}

/// Heuristically detect whether a file is a MangoHud-style capture/log.
pub fn looks_like_mangohud_capture_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if !path.exists() {
        return false;
    }

    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let reader = BufReader::new(file);
    for line_result in reader.lines().take(25) {
        let line = match line_result {
            Ok(line) => line,
            Err(_) => return false,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("fps,") || lower.contains("frametime") {
            return true;
        }

        let fields: Vec<&str> = trimmed.split(',').collect();
        if fields
            .get(1)
            .and_then(|value| parse_valid_frametime(value))
            .is_some()
        {
            return true;
        }
        if fields
            .first()
            .and_then(|value| parse_valid_frametime(value))
            .is_some()
        {
            return true;
        }
    }

    false
}

/// Parse a MangoHud log file
pub fn parse_mangohud_log<P: AsRef<Path>>(path: P) -> Result<FrameData> {
    let file = File::open(path.as_ref())
        .with_context(|| format!("Failed to open file: {:?}", path.as_ref()))?;

    let reader = BufReader::new(file);
    let lines = reader.lines();

    // Find header row
    let mut frametime_col: Option<usize> = None;
    let mut header_found = false;

    let mut frame_times: Vec<f64> = Vec::new();

    for line_result in lines {
        let line = line_result?;

        // Skip empty lines and comments
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();

        // Look for header row
        if !header_found {
            for (i, field) in fields.iter().enumerate() {
                let field_lower = field.trim().to_lowercase();
                if field_lower == "frametime" || field_lower == "frametime_ms" {
                    frametime_col = Some(i);
                }
            }

            if frametime_col.is_some() {
                header_found = true;
                continue;
            }

            // MangoHud default data rows are fps,frametime,...
            // If headers are missing but a second numeric column exists, treat it as frametime.
            if let Some(ms) = fields.get(1).and_then(|value| parse_valid_frametime(value)) {
                frame_times.push(ms);
                frametime_col = Some(1);
                header_found = true;
                continue;
            }

            // Some configs output just one column of frametimes.
            if let Some(ms) = fields
                .first()
                .and_then(|value| parse_valid_frametime(value))
            {
                frame_times.push(ms);
                frametime_col = Some(0);
                header_found = true;
                continue;
            }
        }

        // Parse data rows
        if let Some(col) = frametime_col {
            if let Some(value) = fields.get(col) {
                if let Some(ms) = parse_valid_frametime(value) {
                    frame_times.push(ms);
                }
            }
        }
    }

    if frame_times.is_empty() {
        anyhow::bail!("No frame time data found in MangoHud log");
    }

    // Calculate duration
    let total_ms: f64 = frame_times.iter().sum();
    let duration_secs = total_ms / 1000.0;

    // Try to extract game name from filename
    let application = path
        .as_ref()
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            // MangoHud logs often named like "game_2024-01-15_12-30-00.csv"
            s.split('_')
                .take_while(|part| !part.chars().all(|c| c.is_numeric() || c == '-'))
                .collect::<Vec<_>>()
                .join("_")
        })
        .filter(|s| !s.is_empty());

    Ok(FrameData {
        frame_times_ms: frame_times,
        application,
        duration_secs,
        source: "MangoHud".to_string(),
    })
}

/// Find the most recent MangoHud log in common locations
pub fn find_latest_mangohud_log() -> Option<std::path::PathBuf> {
    let mut possible_paths = Vec::new();
    if let Some(base_dirs) = directories::BaseDirs::new() {
        // Default MangoHud log location
        possible_paths.push(base_dirs.home_dir().join(".local/share/MangoHud"));
        // Config-specified location
        possible_paths.push(base_dirs.config_dir().join("MangoHud/logs"));
    }

    // XDG data home
    if let Ok(path) = std::env::var("XDG_DATA_HOME") {
        possible_paths.push(std::path::PathBuf::from(path).join("MangoHud"));
    }

    // Current directory fallback is intentionally test-only to avoid accidental
    // false positives from unrelated CSV/log files in production.
    #[cfg(test)]
    {
        possible_paths.push(std::path::PathBuf::from("."));
    }

    for path in possible_paths {
        if path.exists() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut log_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let path = e.path();
                        path.extension()
                            .map(|ext| ext == "csv" || ext == "log")
                            .unwrap_or(false)
                            && looks_like_mangohud_capture_file(&path)
                    })
                    .collect();

                // Sort by modification time (newest first)
                log_files.sort_by(|a, b| {
                    let a_time = a.metadata().and_then(|m| m.modified()).ok();
                    let b_time = b.metadata().and_then(|m| m.modified()).ok();
                    b_time.cmp(&a_time)
                });

                if let Some(newest) = log_files.first() {
                    return Some(newest.path());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_mangohud_log_with_header() {
        let log_content = r#"fps,frametime,cpu_load,gpu_load
60,16.67,50,80
59,16.95,52,82
61,16.39,48,78
30,33.33,60,90
60,16.67,50,80
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", log_content).unwrap();

        let result = parse_mangohud_log(file.path()).unwrap();

        assert_eq!(result.frame_times_ms.len(), 5);
        assert_eq!(result.source, "MangoHud");
    }

    #[test]
    fn test_parse_mangohud_simple_format() {
        // Some MangoHud configs output just frame times
        let log_content = r#"16.67
16.95
16.39
33.33
16.67
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", log_content).unwrap();

        let result = parse_mangohud_log(file.path()).unwrap();

        assert_eq!(result.frame_times_ms.len(), 5);
    }

    #[test]
    fn test_parse_mangohud_headerless_fps_and_frametime_rows() {
        let log_content = r#"120,8.33
60,16.67
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", log_content).unwrap();

        let result = parse_mangohud_log(file.path()).unwrap();

        assert_eq!(result.frame_times_ms, vec![8.33, 16.67]);
    }

    #[test]
    fn test_detect_mangohud_log_shape() {
        let log_content = "fps,frametime,cpu_load\n60,16.67,50\n";
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", log_content).unwrap();

        assert!(looks_like_mangohud_capture_file(file.path()));
    }
}
