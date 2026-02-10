//! Benchmark session tracking
//!
//! Tracks FPS data during a gaming session for later submission.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A gaming benchmark session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSession {
    /// Unique session ID
    pub id: Uuid,
    /// Session start time
    pub started_at: DateTime<Utc>,
    /// Session end time (if finished)
    pub ended_at: Option<DateTime<Utc>>,
    /// Game being played
    pub game: String,
    /// Resolution (e.g., "1440p")
    pub resolution: String,
    /// Graphics preset
    pub preset: String,
    /// Ray tracing enabled
    pub ray_tracing: bool,
    /// Upscaling mode (DLSS/FSR)
    pub upscaling: Option<String>,
    /// FPS samples collected during session
    pub fps_samples: Vec<FpsSample>,
}

/// A single FPS sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsSample {
    /// Timestamp when sample was taken
    pub timestamp: DateTime<Utc>,
    /// Current FPS
    pub fps: f64,
    /// Frame time in ms
    pub frame_time_ms: f64,
}

impl BenchmarkSession {
    /// Start a new benchmark session
    #[allow(dead_code)]
    pub fn start(
        game: String,
        resolution: String,
        preset: String,
        ray_tracing: bool,
        upscaling: Option<String>,
    ) -> Self {
        BenchmarkSession {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            ended_at: None,
            game,
            resolution,
            preset,
            ray_tracing,
            upscaling,
            fps_samples: Vec::new(),
        }
    }

    /// Add an FPS sample
    #[allow(dead_code)]
    pub fn add_sample(&mut self, fps: f64) {
        if !fps.is_finite() || fps <= 0.0 {
            return;
        }
        self.fps_samples.push(FpsSample {
            timestamp: Utc::now(),
            fps,
            frame_time_ms: 1000.0 / fps,
        });
    }

    /// End the session
    #[allow(dead_code)]
    pub fn end(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Calculate average FPS
    pub fn average_fps(&self) -> Option<f64> {
        if self.fps_samples.is_empty() {
            return None;
        }
        let sum: f64 = self.fps_samples.iter().map(|s| s.fps).sum();
        Some(sum / self.fps_samples.len() as f64)
    }

    /// Calculate 1% low FPS (99th percentile of frame times)
    pub fn fps_1_low(&self) -> Option<f64> {
        if self.fps_samples.len() < 100 {
            return None; // Need at least 100 samples for meaningful percentile
        }

        let mut frame_times: Vec<f64> = self.fps_samples.iter().map(|s| s.frame_time_ms).collect();
        frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // 99th percentile frame time
        let idx = (frame_times.len() as f64 * 0.99) as usize;
        let p99_frame_time = frame_times.get(idx)?;

        // Convert back to FPS
        Some(1000.0 / p99_frame_time)
    }

    /// Calculate 0.1% low FPS (99.9th percentile of frame times)
    #[allow(dead_code)]
    pub fn fps_01_low(&self) -> Option<f64> {
        if self.fps_samples.len() < 1000 {
            return None; // Need at least 1000 samples
        }

        let mut frame_times: Vec<f64> = self.fps_samples.iter().map(|s| s.frame_time_ms).collect();
        frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx = (frame_times.len() as f64 * 0.999) as usize;
        let p999_frame_time = frame_times.get(idx)?;

        Some(1000.0 / p999_frame_time)
    }

    /// Session duration in seconds
    pub fn duration_secs(&self) -> f64 {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        (end - self.started_at).num_milliseconds() as f64 / 1000.0
    }

    /// Is session valid for submission? (at least 30 seconds of data)
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.fps_samples.len() >= 30 && self.duration_secs() >= 30.0
    }
}
