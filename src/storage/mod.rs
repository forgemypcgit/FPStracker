//! Local storage for offline benchmarks and build configurations
//!
//! Stores data in:
//! - Linux: ~/.local/share/fps-tracker/
//! - macOS: ~/Library/Application Support/fps-tracker/
//! - Windows: %APPDATA%/fps-tracker/

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::PathBuf;
use sysinfo::{Pid, System};
use uuid::Uuid;

use crate::benchmark::BenchmarkSubmission;
use crate::feedback::FeedbackSubmission;
use crate::idempotency;

/// Local storage manager for fps-tracker data
pub struct LocalStorage {
    data_dir: PathBuf,
}

pub struct PendingSyncLock {
    path: PathBuf,
    file: Option<std::fs::File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBenchmark {
    pub id: String,
    pub submission: BenchmarkSubmission,
    /// Idempotency key for this submission. Must be reused for all retries.
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingFeedback {
    pub id: String,
    pub feedback: FeedbackSubmission,
    /// Idempotency key for this feedback. Must be reused for all retries.
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingBenchmarkRecord {
    submission: BenchmarkSubmission,
    idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingFeedbackRecord {
    feedback: FeedbackSubmission,
    idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum PendingBenchmarkFile {
    Record(PendingBenchmarkRecord),
    Legacy(BenchmarkSubmission),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum PendingFeedbackFile {
    Record(PendingFeedbackRecord),
    Legacy(FeedbackSubmission),
}

impl Drop for PendingSyncLock {
    fn drop(&mut self) {
        // Close handle before removing lock file (required on Windows).
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

impl LocalStorage {
    /// Initialize local storage, creating directories if needed
    pub fn new() -> Result<Self> {
        let primary = ProjectDirs::from("com", "forgemypc", "fps-tracker")
            .context("Could not determine project directories")?;
        let data_dir = primary.data_dir().to_path_buf();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        // Create subdirectories
        fs::create_dir_all(data_dir.join("pending"))?;
        fs::create_dir_all(data_dir.join("uploaded"))?;
        fs::create_dir_all(data_dir.join("pending_feedback"))?;
        fs::create_dir_all(data_dir.join("uploaded_feedback"))?;
        fs::create_dir_all(data_dir.join("builds"))?;
        fs::create_dir_all(data_dir.join("captures"))?;

        Ok(Self { data_dir })
    }

    /// Save a benchmark submission for later upload.
    pub fn save_pending_benchmark_with_idempotency_key(
        &self,
        submission: &BenchmarkSubmission,
        idempotency_key: &str,
    ) -> Result<String> {
        let key = idempotency_key.trim();
        if key.is_empty() {
            anyhow::bail!("Idempotency key cannot be empty");
        }

        let id = format!(
            "pending_{}_{}",
            Utc::now().timestamp_millis(),
            Uuid::new_v4().simple()
        );
        let path = self.data_dir.join("pending").join(format!("{id}.json"));
        let record = PendingBenchmarkRecord {
            submission: submission.clone(),
            idempotency_key: key.to_string(),
        };
        let json = serde_json::to_string_pretty(&record)
            .context("Failed to serialize pending benchmark")?;

        let mut file = open_private_file_new(&path)
            .with_context(|| format!("Failed to create benchmark at {}", path.display()))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("Failed to write benchmark to {}", path.display()))?;

        Ok(id)
    }

    /// Load all pending benchmarks
    pub fn load_pending_benchmarks(&self) -> Result<Vec<PendingBenchmark>> {
        let pending_dir = self.data_dir.join("pending");
        let uploaded_dir = self.data_dir.join("uploaded");
        let mut benchmarks = Vec::new();

        if !pending_dir.exists() {
            return Ok(benchmarks);
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&pending_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                entries.push(entry);
            }
        }

        // Oldest pending submissions first for deterministic retry ordering.
        entries.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());

        for entry in entries {
            let path = entry.path();
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // If the record is already finalized as uploaded, skip it from retry queue.
            if uploaded_dir.join(format!("{id}.json")).exists() {
                let _ = fs::remove_file(&path);
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_benchmark(&path);
                    continue;
                }
            };

            let record: PendingBenchmarkFile = match serde_json::from_str(&content) {
                Ok(record) => record,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_benchmark(&path);
                    continue;
                }
            };

            let (submission, idempotency_key) = match record {
                PendingBenchmarkFile::Record(record) => (record.submission, record.idempotency_key),
                PendingBenchmarkFile::Legacy(submission) => {
                    (submission, idempotency::legacy_pending_key(&id))
                }
            };

            benchmarks.push(PendingBenchmark {
                id,
                submission,
                idempotency_key,
            });
        }

        Ok(benchmarks)
    }

    /// Save feedback for later upload.
    pub fn save_pending_feedback_with_idempotency_key(
        &self,
        feedback: &FeedbackSubmission,
        idempotency_key: &str,
    ) -> Result<String> {
        let key = idempotency_key.trim();
        if key.is_empty() {
            anyhow::bail!("Idempotency key cannot be empty");
        }

        let id = format!(
            "pending_feedback_{}_{}",
            Utc::now().timestamp_millis(),
            Uuid::new_v4().simple()
        );
        let path = self
            .data_dir
            .join("pending_feedback")
            .join(format!("{id}.json"));
        let record = PendingFeedbackRecord {
            feedback: feedback.clone(),
            idempotency_key: key.to_string(),
        };
        let json = serde_json::to_string_pretty(&record)
            .context("Failed to serialize pending feedback")?;

        let mut file = open_private_file_new(&path)
            .with_context(|| format!("Failed to create feedback at {}", path.display()))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("Failed to write feedback to {}", path.display()))?;

        Ok(id)
    }

    /// Load all pending feedback.
    pub fn load_pending_feedback(&self) -> Result<Vec<PendingFeedback>> {
        let pending_dir = self.data_dir.join("pending_feedback");
        let uploaded_dir = self.data_dir.join("uploaded_feedback");
        let mut feedbacks = Vec::new();

        if !pending_dir.exists() {
            return Ok(feedbacks);
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&pending_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());

        for entry in entries {
            let path = entry.path();
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            if uploaded_dir.join(format!("{id}.json")).exists() {
                let _ = fs::remove_file(&path);
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_feedback(&path);
                    continue;
                }
            };

            let record: PendingFeedbackFile = match serde_json::from_str(&content) {
                Ok(record) => record,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_feedback(&path);
                    continue;
                }
            };

            let (feedback, idempotency_key) = match record {
                PendingFeedbackFile::Record(record) => (record.feedback, record.idempotency_key),
                PendingFeedbackFile::Legacy(feedback) => {
                    (feedback, idempotency::legacy_pending_feedback_key(&id))
                }
            };

            feedbacks.push(PendingFeedback {
                id,
                feedback,
                idempotency_key,
            });
        }

        Ok(feedbacks)
    }

    pub fn remove_pending_feedback(&self, id: &str) -> Result<()> {
        if !is_valid_pending_id(id) {
            anyhow::bail!("Invalid pending feedback ID");
        }

        let path = self
            .data_dir
            .join("pending_feedback")
            .join(format!("{id}.json"));
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("Failed to remove pending feedback: {}", path.display())
            })?;
        }
        Ok(())
    }

    pub fn mark_pending_feedback_uploaded(&self, id: &str) -> Result<()> {
        if !is_valid_pending_id(id) {
            anyhow::bail!("Invalid pending feedback ID");
        }

        let pending_path = self
            .data_dir
            .join("pending_feedback")
            .join(format!("{id}.json"));
        if !pending_path.exists() {
            return Ok(());
        }

        let uploaded_dir = self.data_dir.join("uploaded_feedback");
        fs::create_dir_all(&uploaded_dir).with_context(|| {
            format!(
                "Failed to create uploaded feedback directory: {}",
                uploaded_dir.display()
            )
        })?;

        let uploaded_path = uploaded_dir.join(format!("{id}.json"));
        if uploaded_path.exists() {
            fs::remove_file(&uploaded_path).with_context(|| {
                format!(
                    "Failed to replace existing uploaded feedback: {}",
                    uploaded_path.display()
                )
            })?;
        }

        if fs::rename(&pending_path, &uploaded_path).is_err() {
            fs::copy(&pending_path, &uploaded_path).with_context(|| {
                format!(
                    "Failed to copy uploaded feedback out of pending queue: {} -> {}",
                    pending_path.display(),
                    uploaded_path.display()
                )
            })?;
            let _ = fs::remove_file(&pending_path);
        }

        Ok(())
    }

    pub fn try_acquire_feedback_sync_lock(&self) -> Result<Option<PendingSyncLock>> {
        let lock_path = self.data_dir.join("pending_feedback").join(".sync.lock");
        match create_feedback_sync_lock(&lock_path) {
            Ok(lock) => Ok(Some(lock)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                if pending_sync_lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    match create_feedback_sync_lock(&lock_path) {
                        Ok(lock) => return Ok(Some(lock)),
                        Err(retry_err) if retry_err.kind() == ErrorKind::AlreadyExists => {
                            return Ok(None);
                        }
                        Err(retry_err) => {
                            return Err(retry_err).with_context(|| {
                                format!(
                                    "Failed to recover feedback sync lock at {}",
                                    lock_path.display()
                                )
                            });
                        }
                    }
                }
                Ok(None)
            }
            Err(err) => Err(err).with_context(|| {
                format!(
                    "Failed to create feedback sync lock at {}",
                    lock_path.display()
                )
            }),
        }
    }

    /// Remove a pending benchmark after successful upload
    pub fn remove_pending_benchmark(&self, id: &str) -> Result<()> {
        if !is_valid_pending_id(id) {
            anyhow::bail!("Invalid pending benchmark ID");
        }

        let path = self.data_dir.join("pending").join(format!("{id}.json"));
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("Failed to remove pending benchmark: {}", path.display())
            })?;
        }
        Ok(())
    }

    /// Move a pending benchmark out of the retry queue after a successful upload.
    pub fn mark_pending_benchmark_uploaded(&self, id: &str) -> Result<()> {
        if !is_valid_pending_id(id) {
            anyhow::bail!("Invalid pending benchmark ID");
        }

        let pending_path = self.data_dir.join("pending").join(format!("{id}.json"));
        if !pending_path.exists() {
            return Ok(());
        }

        let uploaded_dir = self.data_dir.join("uploaded");
        fs::create_dir_all(&uploaded_dir).with_context(|| {
            format!(
                "Failed to create uploaded benchmark directory: {}",
                uploaded_dir.display()
            )
        })?;

        let uploaded_path = uploaded_dir.join(format!("{id}.json"));
        if uploaded_path.exists() {
            fs::remove_file(&uploaded_path).with_context(|| {
                format!(
                    "Failed to replace existing uploaded benchmark: {}",
                    uploaded_path.display()
                )
            })?;
        }

        if fs::rename(&pending_path, &uploaded_path).is_err() {
            fs::copy(&pending_path, &uploaded_path).with_context(|| {
                format!(
                    "Failed to copy uploaded benchmark out of pending queue: {} -> {}",
                    pending_path.display(),
                    uploaded_path.display()
                )
            })?;
            let _ = fs::remove_file(&pending_path);
        }

        Ok(())
    }

    pub fn try_acquire_pending_sync_lock(&self) -> Result<Option<PendingSyncLock>> {
        let lock_path = self.data_dir.join("pending").join(".sync.lock");
        match create_pending_sync_lock(&lock_path) {
            Ok(lock) => Ok(Some(lock)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                // Recover from stale locks left behind by hard crashes or forced termination.
                if pending_sync_lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    match create_pending_sync_lock(&lock_path) {
                        Ok(lock) => return Ok(Some(lock)),
                        Err(retry_err) if retry_err.kind() == ErrorKind::AlreadyExists => {
                            return Ok(None);
                        }
                        Err(retry_err) => {
                            return Err(retry_err).with_context(|| {
                                format!(
                                    "Failed to recover pending sync lock at {}",
                                    lock_path.display()
                                )
                            });
                        }
                    }
                }

                Ok(None)
            }
            Err(err) => Err(err).with_context(|| {
                format!(
                    "Failed to create pending sync lock at {}",
                    lock_path.display()
                )
            }),
        }
    }

    /// Count pending benchmarks
    pub fn pending_count(&self) -> Result<usize> {
        let pending_dir = self.data_dir.join("pending");
        if !pending_dir.exists() {
            return Ok(0);
        }

        let count = fs::read_dir(&pending_dir)?
            .filter(|e| {
                e.as_ref()
                    .map(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .count();

        Ok(count)
    }

    /// Save a build configuration
    pub fn save_build(&self, name: &str, build: &BuildConfig) -> Result<()> {
        let safe_name = sanitize_build_name(name)?;
        let path = self
            .data_dir
            .join("builds")
            .join(format!("{safe_name}.json"));

        let json = serde_json::to_string_pretty(build).context("Failed to serialize build")?;

        let mut file = open_private_file_overwrite(&path)
            .with_context(|| format!("Failed to create build file at {}", path.display()))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("Failed to write build to {}", path.display()))?;

        Ok(())
    }

    /// Load a build configuration
    pub fn load_build(&self, name: &str) -> Result<BuildConfig> {
        let safe_name = sanitize_build_name(name)?;
        let path = self
            .data_dir
            .join("builds")
            .join(format!("{safe_name}.json"));

        let content =
            fs::read_to_string(&path).with_context(|| format!("Build '{}' not found", name))?;

        let build: BuildConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse build: {}", path.display()))?;

        Ok(build)
    }

    /// List all saved builds
    pub fn list_builds(&self) -> Result<Vec<String>> {
        let builds_dir = self.data_dir.join("builds");
        let mut builds = Vec::new();

        if !builds_dir.exists() {
            return Ok(builds);
        }

        for entry in fs::read_dir(&builds_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    builds.push(name.to_string());
                }
            }
        }

        builds.sort_unstable();

        Ok(builds)
    }

    /// Delete a saved build
    pub fn delete_build(&self, name: &str) -> Result<()> {
        let safe_name = sanitize_build_name(name)?;
        let path = self
            .data_dir
            .join("builds")
            .join(format!("{safe_name}.json"));
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete build: {}", path.display()))?;
        }
        Ok(())
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }
}

/// Build configuration for compatibility checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub components: BuildComponents,
    pub notes: Option<String>,
}

/// Component specifications for a build
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildComponents {
    pub cpu: Option<ComponentSpec>,
    pub gpu: Option<ComponentSpec>,
    pub motherboard: Option<ComponentSpec>,
    pub ram: Option<ComponentSpec>,
    pub psu: Option<ComponentSpec>,
    pub case: Option<ComponentSpec>,
    pub cooler: Option<ComponentSpec>,
    pub storage: Vec<ComponentSpec>,
}

/// Individual component specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub name: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub specs: HashMap<String, serde_json::Value>,
}

impl ComponentSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            brand: None,
            model: None,
            specs: HashMap::new(),
        }
    }

    pub fn with_brand(mut self, brand: impl Into<String>) -> Self {
        self.brand = Some(brand.into());
        self
    }

    pub fn with_spec(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.specs.insert(key.into(), value.into());
        self
    }
}

/// Initialize storage and return instance
pub fn init_storage() -> Result<LocalStorage> {
    LocalStorage::new()
}

fn is_valid_pending_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn quarantine_corrupt_pending_benchmark(path: &std::path::Path) -> std::io::Result<()> {
    let invalid_path = path.with_extension("invalid");

    // Try to preserve the file for troubleshooting. If we can't rename, fall back to removing it
    // so one bad file doesn't block all future uploads.
    fs::rename(path, &invalid_path).or_else(|_| fs::remove_file(path))
}

fn quarantine_corrupt_pending_feedback(path: &std::path::Path) -> std::io::Result<()> {
    let invalid_path = path.with_extension("invalid");
    fs::rename(path, &invalid_path).or_else(|_| fs::remove_file(path))
}

fn open_private_file_new(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    open_private_file(path, true, false)
}

fn open_private_file_overwrite(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    open_private_file(path, false, true)
}

fn open_private_file(
    path: &std::path::Path,
    create_new: bool,
    truncate: bool,
) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true);
    }
    if truncate {
        options.truncate(true);
    }

    // Best-effort: restrict permissions at creation time on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)
}

const PENDING_SYNC_LOCK_STALE_SECS: u64 = 6 * 60 * 60;

fn pending_sync_lock_is_stale(path: &std::path::Path) -> bool {
    let lock_timestamp = read_pending_sync_lock_timestamp(path);
    let stale_by_age = lock_age_secs(path) > PENDING_SYNC_LOCK_STALE_SECS;

    if let Some(pid) = read_pending_sync_lock_pid(path) {
        if !process_is_running(pid) {
            return true;
        }
        if let Some(lock_time) = lock_timestamp {
            return process_started_after(pid, lock_time);
        }
        return false;
    }

    stale_by_age
}

fn create_pending_sync_lock(path: &std::path::Path) -> std::io::Result<PendingSyncLock> {
    let mut file = open_private_file_new(path)?;
    let _ = writeln!(
        file,
        "pid={} at={}",
        std::process::id(),
        Utc::now().to_rfc3339()
    );
    Ok(PendingSyncLock {
        path: path.to_path_buf(),
        file: Some(file),
    })
}

fn create_feedback_sync_lock(path: &std::path::Path) -> std::io::Result<PendingSyncLock> {
    // Same format as the benchmark sync lock so staleness checks can be shared.
    let mut file = open_private_file_new(path)?;
    let _ = writeln!(
        file,
        "pid={} at={}",
        std::process::id(),
        Utc::now().to_rfc3339()
    );
    Ok(PendingSyncLock {
        path: path.to_path_buf(),
        file: Some(file),
    })
}

fn read_pending_sync_lock_pid(path: &std::path::Path) -> Option<u32> {
    let contents = fs::read_to_string(path).ok()?;
    let pid_fragment = contents
        .lines()
        .find_map(|line| line.strip_prefix("pid="))?
        .split_whitespace()
        .next()?;
    pid_fragment.parse::<u32>().ok()
}

fn process_is_running(pid: u32) -> bool {
    let system = System::new_all();
    system.process(Pid::from_u32(pid)).is_some()
}

fn process_started_after(pid: u32, when: DateTime<Utc>) -> bool {
    let system = System::new_all();
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return true;
    };

    // `start_time()` is documented as epoch seconds, but we defensively support
    // boot-relative seconds as well to avoid platform-specific ambiguity.
    let raw_start = process.start_time();
    let boot_time = System::boot_time();
    let start_epoch_secs = if raw_start < boot_time {
        boot_time.saturating_add(raw_start)
    } else {
        raw_start
    };

    let Some(started_at) = DateTime::<Utc>::from_timestamp(start_epoch_secs as i64, 0) else {
        return false;
    };

    started_at > when + ChronoDuration::seconds(5)
}

fn lock_age_secs(path: &std::path::Path) -> u64 {
    if let Some(lock_time) = read_pending_sync_lock_timestamp(path) {
        return Utc::now()
            .signed_duration_since(lock_time)
            .to_std()
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
    }

    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn read_pending_sync_lock_timestamp(path: &std::path::Path) -> Option<DateTime<Utc>> {
    let contents = fs::read_to_string(path).ok()?;
    let at_fragment = contents
        .split_whitespace()
        .find_map(|part| part.strip_prefix("at="))?;
    DateTime::parse_from_rfc3339(at_fragment)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn sanitize_build_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Build name cannot be empty");
    }
    if trimmed.len() > 120 {
        anyhow::bail!("Build name is too long (max 120 characters)");
    }
    if trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || trimmed.contains("..")
    {
        anyhow::bail!("Build name contains invalid path characters");
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackCategory, FeedbackSubmission, FeedbackSurface};
    use crate::hardware::cpu::CpuInfo;
    use crate::hardware::gpu::{GpuInfo, GpuVendor};
    use crate::hardware::ram::RamInfo;
    use crate::hardware::SystemInfo;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_build() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };

        // Create subdirectories manually for test
        fs::create_dir_all(storage.data_dir.join("builds")).unwrap();

        let build = BuildConfig {
            name: "test-build".to_string(),
            created_at: Utc::now(),
            components: BuildComponents {
                cpu: Some(ComponentSpec::new("AMD Ryzen 5 7600X")),
                gpu: Some(ComponentSpec::new("RTX 4070")),
                ..Default::default()
            },
            notes: Some("Test build".to_string()),
        };

        storage.save_build("test-build", &build).unwrap();
        let loaded = storage.load_build("test-build").unwrap();

        assert_eq!(loaded.name, "test-build");
        assert!(loaded.components.cpu.is_some());
        assert_eq!(loaded.components.gpu.as_ref().unwrap().name, "RTX 4070");
    }

    #[test]
    fn test_rejects_invalid_build_name() {
        let result = sanitize_build_name("../escape");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_pending_benchmarks_skips_invalid_entries() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };

        fs::create_dir_all(storage.data_dir.join("pending")).unwrap();

        let system_info = SystemInfo {
            gpu: GpuInfo {
                name: "Test GPU".to_string(),
                vendor: GpuVendor::Unknown,
                pci_id: None,
                vram_mb: Some(8192),
                gpu_clock_mhz: None,
                memory_clock_mhz: None,
                temperature_c: None,
                utilization_percent: None,
                driver_version: None,
            },
            cpu: CpuInfo {
                name: "Test CPU".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: Some(4200),
                max_frequency_mhz: None,
                architecture: Some("x86_64".to_string()),
                vendor: "Unknown".to_string(),
            },
            ram: RamInfo {
                installed_mb: Some(16_384),
                usable_mb: 16_000,
                speed_mhz: Some(3200),
                ram_type: None,
                stick_count: None,
                model: None,
            },
            os: "Linux".to_string(),
            os_version: None,
        };

        let submission = BenchmarkSubmission::new(
            system_info,
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            60.0,
            None,
            false,
            None,
        );

        let pending_id = storage
            .save_pending_benchmark_with_idempotency_key(
                &submission,
                "fps-tracker-submit-test-invalid-entry",
            )
            .unwrap();

        let bad_path = storage.data_dir.join("pending").join("bad.json");
        fs::write(&bad_path, "{not valid json").unwrap();

        let loaded = storage.load_pending_benchmarks().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, pending_id);
        assert!(loaded[0].idempotency_key.starts_with("fps-tracker-submit-"));

        assert!(!bad_path.exists());
        assert!(storage
            .data_dir
            .join("pending")
            .join("bad.invalid")
            .exists());
    }

    #[test]
    fn test_load_pending_feedback_skips_invalid_entries() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };

        fs::create_dir_all(storage.data_dir.join("pending_feedback")).unwrap();

        let feedback = FeedbackSubmission {
            surface: FeedbackSurface::TerminalUi,
            category: FeedbackCategory::Other,
            issue_code: "other".to_string(),
            message: "Something went wrong.".to_string(),
            diagnostics: None,
        };

        let pending_id = storage
            .save_pending_feedback_with_idempotency_key(&feedback, "fps-tracker-feedback-test")
            .unwrap();

        let bad_path = storage.data_dir.join("pending_feedback").join("bad.json");
        fs::write(&bad_path, "{not valid json").unwrap();

        let loaded = storage.load_pending_feedback().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, pending_id);
        assert_eq!(loaded[0].feedback.issue_code, "other");
        assert_eq!(loaded[0].idempotency_key, "fps-tracker-feedback-test");

        assert!(!bad_path.exists());
        assert!(storage
            .data_dir
            .join("pending_feedback")
            .join("bad.invalid")
            .exists());
    }

    #[test]
    fn test_remove_pending_benchmark_rejects_invalid_id() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };

        let result = storage.remove_pending_benchmark("../escape");
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_pending_benchmark_uploaded_moves_file_out_of_pending_queue() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };
        fs::create_dir_all(storage.data_dir.join("pending")).unwrap();
        fs::create_dir_all(storage.data_dir.join("uploaded")).unwrap();

        let system_info = SystemInfo {
            gpu: GpuInfo {
                name: "Test GPU".to_string(),
                vendor: GpuVendor::Unknown,
                pci_id: None,
                vram_mb: Some(8192),
                gpu_clock_mhz: None,
                memory_clock_mhz: None,
                temperature_c: None,
                utilization_percent: None,
                driver_version: None,
            },
            cpu: CpuInfo {
                name: "Test CPU".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: Some(4200),
                max_frequency_mhz: None,
                architecture: Some("x86_64".to_string()),
                vendor: "Unknown".to_string(),
            },
            ram: RamInfo {
                installed_mb: Some(16_384),
                usable_mb: 16_000,
                speed_mhz: Some(3200),
                ram_type: None,
                stick_count: None,
                model: None,
            },
            os: "Linux".to_string(),
            os_version: None,
        };

        let submission = BenchmarkSubmission::new(
            system_info,
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            60.0,
            None,
            false,
            None,
        );

        let pending_id = storage
            .save_pending_benchmark_with_idempotency_key(
                &submission,
                "fps-tracker-submit-test-mark-uploaded",
            )
            .unwrap();
        let pending_path = storage
            .data_dir
            .join("pending")
            .join(format!("{pending_id}.json"));
        assert!(pending_path.exists());

        storage
            .mark_pending_benchmark_uploaded(&pending_id)
            .expect("mark uploaded should succeed");

        assert!(!pending_path.exists());
        assert!(storage
            .data_dir
            .join("uploaded")
            .join(format!("{pending_id}.json"))
            .exists());
    }

    #[test]
    fn test_load_pending_benchmarks_skips_already_uploaded_records() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };
        fs::create_dir_all(storage.data_dir.join("pending")).unwrap();
        fs::create_dir_all(storage.data_dir.join("uploaded")).unwrap();

        let system_info = SystemInfo {
            gpu: GpuInfo {
                name: "Test GPU".to_string(),
                vendor: GpuVendor::Unknown,
                pci_id: None,
                vram_mb: Some(8192),
                gpu_clock_mhz: None,
                memory_clock_mhz: None,
                temperature_c: None,
                utilization_percent: None,
                driver_version: None,
            },
            cpu: CpuInfo {
                name: "Test CPU".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: Some(4200),
                max_frequency_mhz: None,
                architecture: Some("x86_64".to_string()),
                vendor: "Unknown".to_string(),
            },
            ram: RamInfo {
                installed_mb: Some(16_384),
                usable_mb: 16_000,
                speed_mhz: Some(3200),
                ram_type: None,
                stick_count: None,
                model: None,
            },
            os: "Linux".to_string(),
            os_version: None,
        };

        let submission = BenchmarkSubmission::new(
            system_info,
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            60.0,
            None,
            false,
            None,
        );
        let pending_id = storage
            .save_pending_benchmark_with_idempotency_key(
                &submission,
                "fps-tracker-submit-test-skip-uploaded",
            )
            .unwrap();
        let pending_path = storage
            .data_dir
            .join("pending")
            .join(format!("{pending_id}.json"));
        let uploaded_path = storage
            .data_dir
            .join("uploaded")
            .join(format!("{pending_id}.json"));
        fs::copy(&pending_path, &uploaded_path).unwrap();

        let loaded = storage.load_pending_benchmarks().unwrap();
        assert!(loaded.is_empty());
        assert!(!pending_path.exists());
        assert!(uploaded_path.exists());
    }

    #[test]
    fn test_load_pending_benchmarks_supports_legacy_format() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };
        fs::create_dir_all(storage.data_dir.join("pending")).unwrap();

        let system_info = SystemInfo {
            gpu: GpuInfo {
                name: "Test GPU".to_string(),
                vendor: GpuVendor::Unknown,
                pci_id: None,
                vram_mb: Some(8192),
                gpu_clock_mhz: None,
                memory_clock_mhz: None,
                temperature_c: None,
                utilization_percent: None,
                driver_version: None,
            },
            cpu: CpuInfo {
                name: "Test CPU".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: Some(4200),
                max_frequency_mhz: None,
                architecture: Some("x86_64".to_string()),
                vendor: "Unknown".to_string(),
            },
            ram: RamInfo {
                installed_mb: Some(16_384),
                usable_mb: 16_000,
                speed_mhz: Some(3200),
                ram_type: None,
                stick_count: None,
                model: None,
            },
            os: "Linux".to_string(),
            os_version: None,
        };

        let submission = BenchmarkSubmission::new(
            system_info,
            "Cyberpunk 2077".to_string(),
            "1440p".to_string(),
            "High".to_string(),
            60.0,
            None,
            false,
            None,
        );
        let legacy_id = "pending_legacy_record";
        let legacy_path = storage
            .data_dir
            .join("pending")
            .join(format!("{legacy_id}.json"));
        fs::write(
            legacy_path,
            serde_json::to_string_pretty(&submission).unwrap(),
        )
        .unwrap();

        let loaded = storage.load_pending_benchmarks().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, legacy_id);
        assert_eq!(
            loaded[0].idempotency_key,
            format!("fps-tracker-pending-{legacy_id}")
        );
    }

    #[test]
    fn test_pending_sync_lock_with_running_pid_can_still_be_stale_by_age() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join(".sync.lock");
        let old = Utc::now() - chrono::Duration::hours(8);
        fs::write(
            &lock_path,
            format!("pid={} at={}", std::process::id(), old.to_rfc3339()),
        )
        .unwrap();
        assert!(pending_sync_lock_is_stale(&lock_path));
    }

    #[test]
    fn test_pending_sync_lock_allows_only_one_holder() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage {
            data_dir: temp_dir.path().to_path_buf(),
        };
        fs::create_dir_all(storage.data_dir.join("pending")).unwrap();

        let lock = storage
            .try_acquire_pending_sync_lock()
            .unwrap()
            .expect("first lock should acquire");
        let second = storage.try_acquire_pending_sync_lock().unwrap();
        assert!(second.is_none());

        drop(lock);

        let third = storage.try_acquire_pending_sync_lock().unwrap();
        assert!(third.is_some());
    }
}
