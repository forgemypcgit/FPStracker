//! Common data structures for parsed benchmark data

use serde::{Deserialize, Serialize};

/// Raw frame timing data from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    /// Frame times in milliseconds
    pub frame_times_ms: Vec<f64>,
    /// Application/game name (if detected)
    pub application: Option<String>,
    /// Capture duration in seconds
    pub duration_secs: f64,
    /// Source tool name
    pub source: String,
}

/// Calculated benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Average FPS
    pub avg_fps: f64,
    /// 1% low FPS (99th percentile frame time)
    pub fps_1_low: f64,
    /// 0.1% low FPS (99.9th percentile frame time)
    pub fps_01_low: Option<f64>,
    /// Minimum FPS
    pub min_fps: f64,
    /// Maximum FPS
    pub max_fps: f64,
    /// Total frames captured
    pub frame_count: usize,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Average frame time in ms
    pub avg_frame_time_ms: f64,
    /// Application name
    pub application: Option<String>,
    /// Source tool
    pub source: String,
}

impl FrameData {
    /// Calculate benchmark statistics from frame data
    pub fn calculate_stats(&self) -> Option<BenchmarkResult> {
        self.calculate_stats_with_max_frame_time(1000.0)
    }

    /// Calculate benchmark statistics with a custom frame-time validation ceiling.
    pub fn calculate_stats_with_max_frame_time(
        &self,
        max_frame_time_ms: f64,
    ) -> Option<BenchmarkResult> {
        let valid_times = self.sanitized_frame_times(max_frame_time_ms);
        if valid_times.is_empty() {
            return None;
        }

        let frame_count = valid_times.len();
        let fps_values: Vec<f64> = valid_times.iter().map(|&ft| 1000.0 / ft).collect();
        if fps_values.is_empty() {
            return None;
        }

        let avg_frame_time: f64 = valid_times.iter().sum::<f64>() / frame_count as f64;
        let avg_fps = 1000.0 / avg_frame_time;

        let mut sorted_times = valid_times.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx_99 = ((frame_count as f64 * 0.99) as usize).min(frame_count - 1);
        let p99_frame_time = sorted_times[idx_99];
        let fps_1_low = 1000.0 / p99_frame_time;

        let fps_01_low = if frame_count >= 1000 {
            let idx_999 = ((frame_count as f64 * 0.999) as usize).min(frame_count - 1);
            let p999_frame_time = sorted_times[idx_999];
            Some(1000.0 / p999_frame_time)
        } else {
            None
        };

        let min_fps = fps_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_fps = fps_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        Some(BenchmarkResult {
            avg_fps,
            fps_1_low,
            fps_01_low,
            min_fps,
            max_fps,
            frame_count,
            duration_secs: self.duration_secs,
            avg_frame_time_ms: avg_frame_time,
            application: self.application.clone(),
            source: self.source.clone(),
        })
    }

    /// Return finite, positive frame times under `max_frame_time_ms`.
    pub fn sanitized_frame_times(&self, max_frame_time_ms: f64) -> Vec<f64> {
        let ceiling = if max_frame_time_ms.is_finite() && max_frame_time_ms > 0.0 {
            max_frame_time_ms
        } else {
            1000.0
        };

        self.frame_times_ms
            .iter()
            .copied()
            .filter(|ft| ft.is_finite() && *ft > 0.0 && *ft <= ceiling)
            .collect()
    }

    /// Median frame time for sanitized samples.
    pub fn median_frame_time_ms(&self, max_frame_time_ms: f64) -> Option<f64> {
        let mut values = self.sanitized_frame_times(max_frame_time_ms);
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = values.len() / 2;
        if values.len().is_multiple_of(2) {
            Some((values[mid - 1] + values[mid]) / 2.0)
        } else {
            Some(values[mid])
        }
    }

    /// Count stutter spikes using a threshold derived from median frame time.
    pub fn stutter_spike_count(&self, max_frame_time_ms: f64) -> usize {
        let values = self.sanitized_frame_times(max_frame_time_ms);
        if values.is_empty() {
            return 0;
        }
        let median = match self.median_frame_time_ms(max_frame_time_ms) {
            Some(value) => value,
            None => return 0,
        };
        let spike_threshold = (median * 1.75).max(33.3);
        values.iter().filter(|ft| **ft >= spike_threshold).count()
    }

    /// Ratio of stutter spikes to total sanitized samples.
    pub fn stutter_spike_ratio(&self, max_frame_time_ms: f64) -> f64 {
        let values = self.sanitized_frame_times(max_frame_time_ms);
        if values.is_empty() {
            return 0.0;
        }
        self.stutter_spike_count(max_frame_time_ms) as f64 / values.len() as f64
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const WIDTH: usize = 44;

        writeln!(f, "┌{}┐", "─".repeat(WIDTH))?;
        writeln!(f, "│{:^WIDTH$}│", "BENCHMARK RESULTS")?;
        writeln!(f, "├{}┤", "─".repeat(WIDTH))?;

        if let Some(ref app) = self.application {
            writeln!(f, "│ Application: {:<WIDTH$}│", app)?;
        }

        writeln!(f, "│ Source:      {:<WIDTH$}│", self.source)?;
        writeln!(f, "├{}┤", "─".repeat(WIDTH))?;
        writeln!(f, "│ Average FPS:     {:>8.1}              │", self.avg_fps)?;
        writeln!(
            f,
            "│ 1% Low FPS:      {:>8.1}              │",
            self.fps_1_low
        )?;

        if let Some(fps_01) = self.fps_01_low {
            writeln!(f, "│ 0.1% Low FPS:    {:>8.1}              │", fps_01)?;
        }

        writeln!(f, "│ Min FPS:         {:>8.1}              │", self.min_fps)?;
        writeln!(f, "│ Max FPS:         {:>8.1}              │", self.max_fps)?;
        writeln!(f, "├{}┤", "─".repeat(WIDTH))?;
        writeln!(
            f,
            "│ Frame Count:     {:>8}               │",
            self.frame_count
        )?;
        writeln!(
            f,
            "│ Duration:        {:>8.1}s             │",
            self.duration_secs
        )?;
        writeln!(
            f,
            "│ Avg Frame Time:  {:>8.2}ms            │",
            self.avg_frame_time_ms
        )?;
        writeln!(f, "└{}┘", "─".repeat(WIDTH))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FrameData;

    #[test]
    fn spike_metrics_detect_tail_events() {
        let data = FrameData {
            frame_times_ms: vec![10.0, 10.5, 9.8, 11.0, 40.0, 41.0],
            application: Some("test-game".to_string()),
            duration_secs: 1.0,
            source: "test".to_string(),
        };

        assert_eq!(data.stutter_spike_count(1000.0), 2);
        let ratio = data.stutter_spike_ratio(1000.0);
        assert!(ratio > 0.30 && ratio < 0.35);
    }
}
