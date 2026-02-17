//! CapFrameX CSV parser
//!
//! CapFrameX is a popular Windows FPS capture tool that uses PresentMon.
//! It exports CSV files with frame timing data.
//!
//! CSV Format (simplified):
//! - Header row with metadata
//! - "MsBetweenPresents" column contains frame times in milliseconds
//! - "Application" column contains the game/app name

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use std::collections::HashSet;
use std::path::Path;

use crate::benchmark::focus;

use super::common::FrameData;

/// Parse a CapFrameX capture CSV file
pub fn parse_capframex_csv<P: AsRef<Path>>(path: P) -> Result<FrameData> {
    parse_capframex_csv_for_process(path, None)
}

/// Parse a CapFrameX/PresentMon CSV file and optionally filter rows to a target process.
pub fn parse_capframex_csv_for_process<P: AsRef<Path>>(
    path: P,
    process_filter: Option<&str>,
) -> Result<FrameData> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path.as_ref())
        .with_context(|| format!("Failed to open file: {:?}", path.as_ref()))?;

    // Find header row and locate important columns
    let mut frametime_col: Option<usize> = None;
    let mut application_col: Option<usize> = None;
    let mut process_name_col: Option<usize> = None;
    let mut header_found = false;

    let mut frame_times: Vec<f64> = Vec::new();
    let mut application: Option<String> = None;
    let mut seen_processes: HashSet<String> = HashSet::new();
    let normalized_filter = process_filter.map(focus::normalize_process_name);

    for record_result in reader.records() {
        let record = record_result.with_context(|| {
            format!("Failed to parse CSV record from file: {:?}", path.as_ref())
        })?;

        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }

        // Look for header row
        if !header_found {
            for (i, field) in record.iter().enumerate() {
                let field_lower = field.trim().to_ascii_lowercase();
                if field_lower.contains("msbetweenpresents")
                    || field_lower.contains("frametime")
                    || field_lower.contains("msbetween")
                {
                    frametime_col = Some(i);
                }
                if field_lower.contains("application") {
                    application_col = Some(i);
                }
                if field_lower == "processname"
                    || field_lower.contains("process_name")
                    || field_lower == "process"
                {
                    process_name_col = Some(i);
                }
            }

            if frametime_col.is_some() {
                header_found = true;
                continue;
            }
        }

        // Parse data rows
        if header_found {
            let process_value = process_name_col
                .and_then(|col| record.get(col))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    application_col
                        .and_then(|col| record.get(col))
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                });

            if let Some(proc_name) = process_value.as_deref() {
                seen_processes.insert(proc_name.to_string());
            }

            let include_row = if let Some(filter) = normalized_filter.as_deref() {
                process_value
                    .as_deref()
                    .map(focus::normalize_process_name)
                    .map(|candidate| candidate == filter)
                    .unwrap_or(false)
            } else {
                true
            };

            if !include_row {
                continue;
            }

            if let Some(col) = frametime_col {
                if let Some(value) = record.get(col) {
                    if let Ok(ms) = value.trim().parse::<f64>() {
                        if ms.is_finite() && ms > 0.0 && ms <= 10_000.0 {
                            frame_times.push(ms);
                        }
                    }
                }
            }

            // Get application name from first data row
            if application.is_none() {
                application = process_value;
            }
        }
    }

    if frame_times.is_empty() {
        if process_filter.is_some() && !seen_processes.is_empty() {
            let mut observed: Vec<String> = seen_processes.into_iter().collect();
            observed.sort_unstable();
            anyhow::bail!(
                "No frame time data found for requested process. Observed processes: {}",
                observed.join(", ")
            );
        }
        anyhow::bail!("No frame time data found in CSV file");
    }

    // Calculate duration
    let total_ms: f64 = frame_times.iter().sum();
    let duration_secs = total_ms / 1000.0;

    Ok(FrameData {
        frame_times_ms: frame_times,
        application,
        duration_secs,
        source: "CapFrameX".to_string(),
    })
}

/// Find the most recent CapFrameX capture in the default directory
pub fn find_latest_capframex_capture() -> Option<std::path::PathBuf> {
    // CapFrameX default paths
    let possible_paths: [Option<std::path::PathBuf>; 2] = [
        // Windows default
        directories::UserDirs::new().and_then(|d| {
            d.document_dir()
                .map(|p| p.join("CapFrameX").join("Captures"))
        }),
        // Alternative location
        directories::BaseDirs::new().map(|d| d.data_local_dir().join("CapFrameX").join("Captures")),
    ];

    for path_opt in possible_paths.iter().flatten() {
        if path_opt.exists() {
            // Find most recent CSV file
            if let Ok(entries) = std::fs::read_dir(path_opt) {
                let mut csv_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "csv")
                            .unwrap_or(false)
                    })
                    .collect();

                // Sort by modification time (newest first)
                csv_files.sort_by(|a, b| {
                    let a_time = a.metadata().and_then(|m| m.modified()).ok();
                    let b_time = b.metadata().and_then(|m| m.modified()).ok();
                    b_time.cmp(&a_time)
                });

                if let Some(newest) = csv_files.first() {
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
    fn test_parse_capframex_csv() {
        let csv_content = r#"Application,MsBetweenPresents,TimeInSeconds
game.exe,16.67,0.01667
game.exe,16.65,0.03332
game.exe,16.70,0.05002
game.exe,33.33,0.08335
game.exe,16.68,0.10003
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();

        let result = parse_capframex_csv(file.path()).unwrap();

        assert_eq!(result.frame_times_ms.len(), 5);
        assert_eq!(result.application, Some("game.exe".to_string()));
        assert_eq!(result.source, "CapFrameX");

        // Calculate stats
        let stats = result.calculate_stats().unwrap();
        assert!(stats.avg_fps > 0.0);
        assert!(stats.fps_1_low > 0.0);
    }

    #[test]
    fn test_parse_capframex_csv_with_process_filter() {
        let csv_content = r#"ProcessName,MsBetweenPresents,TimeInSeconds
game.exe,16.67,0.01667
other.exe,33.33,0.05000
game.exe,16.65,0.06665
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();

        let result = parse_capframex_csv_for_process(file.path(), Some("game.exe")).unwrap();
        assert_eq!(result.frame_times_ms.len(), 2);
        assert_eq!(result.application, Some("game.exe".to_string()));
    }

    #[test]
    fn test_parse_capframex_csv_with_uppercase_process_extension_filter() {
        let csv_content = r#"ProcessName,MsBetweenPresents,TimeInSeconds
GAME.EXE,16.67,0.01667
OTHER.EXE,33.33,0.05000
GAME.EXE,16.65,0.06665
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();

        let result = parse_capframex_csv_for_process(file.path(), Some("game.exe")).unwrap();
        assert_eq!(result.frame_times_ms.len(), 2);
    }

    #[test]
    fn test_parse_capframex_csv_handles_quoted_commas_with_process_filter() {
        let csv_content = r#"Application,ProcessName,MsBetweenPresents,TimeInSeconds
"My, Game",game.exe,16.67,0.01667
"My, Game",game.exe,16.65,0.03332
"My, Game",other.exe,33.33,0.06665
"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();

        let result = parse_capframex_csv_for_process(file.path(), Some("game.exe")).unwrap();
        assert_eq!(result.frame_times_ms, vec![16.67, 16.65]);
        assert_eq!(result.application, Some("game.exe".to_string()));
    }
}
