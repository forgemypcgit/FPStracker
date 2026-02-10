//! Local storage for offline benchmarks and build configurations
//!
//! Stores data in:
//! - Linux: ~/.local/share/fps-tracker/
//! - macOS: ~/Library/Application Support/fps-tracker/
//! - Windows: %APPDATA%/fps-tracker/

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

use crate::benchmark::BenchmarkSubmission;

/// Local storage manager for fps-tracker data
pub struct LocalStorage {
    data_dir: PathBuf,
}

impl LocalStorage {
    /// Initialize local storage, creating directories if needed
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "pcbuilder", "fps-tracker")
            .context("Could not determine project directories")?;

        let data_dir = proj_dirs.data_dir().to_path_buf();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        // Create subdirectories
        fs::create_dir_all(data_dir.join("pending"))?;
        fs::create_dir_all(data_dir.join("builds"))?;
        fs::create_dir_all(data_dir.join("captures"))?;

        Ok(Self { data_dir })
    }

    /// Save a benchmark submission for later upload
    pub fn save_pending_benchmark(&self, submission: &BenchmarkSubmission) -> Result<String> {
        let id = format!(
            "pending_{}_{}",
            Utc::now().timestamp_millis(),
            Uuid::new_v4().simple()
        );
        let path = self.data_dir.join("pending").join(format!("{}.json", id));

        let json =
            serde_json::to_string_pretty(submission).context("Failed to serialize benchmark")?;

        let mut file = open_private_file_new(&path)
            .with_context(|| format!("Failed to create benchmark at {}", path.display()))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("Failed to write benchmark to {}", path.display()))?;

        Ok(id)
    }

    /// Load all pending benchmarks
    pub fn load_pending_benchmarks(&self) -> Result<Vec<(String, BenchmarkSubmission)>> {
        let pending_dir = self.data_dir.join("pending");
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

            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_benchmark(&path);
                    continue;
                }
            };

            let submission: BenchmarkSubmission = match serde_json::from_str(&content) {
                Ok(submission) => submission,
                Err(_) => {
                    let _ = quarantine_corrupt_pending_benchmark(&path);
                    continue;
                }
            };

            benchmarks.push((id, submission));
        }

        Ok(benchmarks)
    }

    /// Remove a pending benchmark after successful upload
    pub fn remove_pending_benchmark(&self, id: &str) -> Result<()> {
        let path = self.data_dir.join("pending").join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("Failed to remove pending benchmark: {}", path.display())
            })?;
        }
        Ok(())
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
            .join(format!("{}.json", safe_name));

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
            .join(format!("{}.json", safe_name));

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
            .join(format!("{}.json", safe_name));
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

fn quarantine_corrupt_pending_benchmark(path: &std::path::Path) -> std::io::Result<()> {
    let invalid_path = path.with_extension("invalid");

    // Try to preserve the file for troubleshooting. If we can't rename, fall back to removing it
    // so one bad file doesn't block all future uploads.
    fs::rename(path, &invalid_path).or_else(|_| fs::remove_file(path))
}

fn open_private_file_new(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);

    // Best-effort: restrict permissions at creation time on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)
}

fn open_private_file_overwrite(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)
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

        let pending_id = storage.save_pending_benchmark(&submission).unwrap();

        let bad_path = storage.data_dir.join("pending").join("bad.json");
        fs::write(&bad_path, "{not valid json").unwrap();

        let loaded = storage.load_pending_benchmarks().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, pending_id);

        assert!(!bad_path.exists());
        assert!(storage
            .data_dir
            .join("pending")
            .join("bad.invalid")
            .exists());
    }
}
