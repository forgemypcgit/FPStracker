#[cfg(target_os = "windows")]
use anyhow::Context;
use anyhow::Result;
#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "windows")]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct DependencyStatus {
    pub(crate) name: &'static str,
    pub(crate) required: bool,
    pub(crate) available: bool,
    pub(crate) details: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub(crate) struct WindowsRuntimeProbe {
    pub(crate) winget_available: bool,
    pub(crate) presentmon_path: Option<PathBuf>,
    pub(crate) presentmon_help_ok: bool,
    pub(crate) presentmon_help_summary: String,
}

#[cfg(any(test, target_os = "windows"))]
pub(crate) fn parse_where_output(output: &str) -> Option<PathBuf> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
}

#[cfg(any(test, target_os = "windows"))]
pub(crate) fn prepend_path_once(current_path: &str, directory: &str) -> String {
    let separator = if current_path.contains(';')
        || looks_like_windows_path(current_path)
        || looks_like_windows_path(directory)
    {
        ';'
    } else {
        ':'
    };
    let mut entries: Vec<String> = current_path
        .split(separator)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if entries
        .iter()
        .any(|entry| normalize_path_for_compare(entry) == normalize_path_for_compare(directory))
    {
        return entries.join(&separator.to_string());
    }

    entries.insert(0, directory.to_string());
    entries.join(&separator.to_string())
}

#[cfg(any(test, target_os = "windows"))]
fn looks_like_windows_path(value: &str) -> bool {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    trimmed.contains('\\') || trimmed.starts_with("\\\\") || (bytes.len() > 1 && bytes[1] == b':')
}

#[cfg(any(test, target_os = "windows"))]
fn normalize_path_for_compare(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['\\', '/'])
        .to_ascii_lowercase()
}

pub(crate) fn is_command_available(command: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        locate_windows_command(command).is_some()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(command)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

pub(crate) fn collect_dependency_statuses() -> Vec<DependencyStatus> {
    #[cfg(target_os = "windows")]
    {
        let presentmon_path = locate_presentmon_executable();
        vec![
            DependencyStatus {
                name: "presentmon",
                required: true,
                available: presentmon_path.is_some(),
                details: presentmon_path
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "Missing (required for live auto-capture)".to_string()),
            },
            DependencyStatus {
                name: "winget",
                required: false,
                available: is_command_available("winget"),
                details: "Used for secure dependency bootstrap".to_string(),
            },
            DependencyStatus {
                name: "winsat",
                required: false,
                available: is_command_available("winsat"),
                details: "Optional synthetic benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "7z",
                required: false,
                available: is_command_available("7z"),
                details: "Optional CPU fallback benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "diskspd",
                required: false,
                available: locate_diskspd_executable().is_some(),
                details: "Optional disk synthetic benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "blender",
                required: false,
                available: locate_blender_executable().is_some(),
                details: "Optional CPU render benchmark tool".to_string(),
            },
        ]
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![
            DependencyStatus {
                name: "mangohud logs",
                required: true,
                available: true,
                details: "Required only when using live capture on Linux (MANGOHUD_LOG=1)"
                    .to_string(),
            },
            DependencyStatus {
                name: "glmark2",
                required: false,
                available: is_command_available("glmark2"),
                details: "Optional synthetic GPU benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "sysbench",
                required: false,
                available: is_command_available("sysbench"),
                details: "Optional synthetic CPU benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "fio",
                required: false,
                available: is_command_available("fio"),
                details: "Optional synthetic disk benchmark tool".to_string(),
            },
            DependencyStatus {
                name: "stress-ng",
                required: false,
                available: is_command_available("stress-ng"),
                details: "Optional synthetic CPU fallback benchmark tool".to_string(),
            },
        ]
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn probe_windows_runtime() -> WindowsRuntimeProbe {
    let winget_available = is_command_available("winget");
    let presentmon_path = locate_presentmon_executable();
    let (presentmon_help_ok, presentmon_help_summary) = match presentmon_path.as_ref() {
        Some(path) => match Command::new(path).arg("--help").output() {
            Ok(output) => {
                let summary = first_non_empty_output_line(&output)
                    .unwrap_or_else(|| format!("presentmon --help returned {}", output.status));
                // PresentMon sometimes returns a non-zero exit code even when printing help/version
                // information successfully. Treat "help text present" as a valid runtime signal.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{stdout}\n{stderr}");
                let combined_lower = combined.to_ascii_lowercase();
                let has_help_text =
                    combined.contains("PresentMon") || combined.contains("Capture Target Options");
                let has_fatal_error = combined_lower.contains("not found")
                    || combined_lower.contains("is not recognized")
                    || combined_lower.contains("no such file or directory");

                (
                    output.status.success() || (has_help_text && !has_fatal_error),
                    summary,
                )
            }
            Err(err) => (
                false,
                format!("Failed to execute {}: {err}", path.display()),
            ),
        },
        None => (
            false,
            "presentmon executable not found in PATH or known install locations".to_string(),
        ),
    };

    WindowsRuntimeProbe {
        winget_available,
        presentmon_path,
        presentmon_help_ok,
        presentmon_help_summary,
    }
}

#[cfg(target_os = "windows")]
const PRESENTMON_WINGET_ID: &str = "Intel.PresentMon.Console";
#[cfg(target_os = "windows")]
const SEVEN_ZIP_WINGET_ID: &str = "7zip.7zip";
#[cfg(target_os = "windows")]
const DISKSPD_WINGET_ID: &str = "Microsoft.DiskSpd";
#[cfg(target_os = "windows")]
const BLENDER_WINGET_ID: &str = "BlenderFoundation.Blender";

#[cfg(target_os = "windows")]
pub(crate) fn ensure_presentmon_for_session(allow_install: bool) -> Result<Option<PathBuf>> {
    if let Some(path) = locate_presentmon_executable() {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    if !allow_install {
        return Ok(None);
    }

    if is_command_available("winget") {
        if let Err(err) = install_winget_package(PRESENTMON_WINGET_ID) {
            if let Some(path) = install_presentmon_download_fallback()
                .context("PresentMon winget install failed and fallback bootstrap also failed")?
            {
                ensure_parent_dir_in_path(&path);
                return Ok(Some(path));
            }
            return Err(err).context("Failed while installing PresentMon with winget");
        }
    } else if let Some(path) = install_presentmon_download_fallback()
        .context("winget is unavailable and PresentMon fallback bootstrap failed")?
    {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    } else {
        anyhow::bail!(
            "winget is not available and fallback bootstrap could not install PresentMon."
        );
    }

    if let Some(path) = locate_presentmon_executable() {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    anyhow::bail!(
        "PresentMon installation completed but executable is not visible in this session. Open a new terminal and retry."
    );
}

#[cfg(target_os = "windows")]
pub(crate) fn ensure_7zip_for_session(allow_install: bool) -> Result<Option<PathBuf>> {
    if let Some(path) = locate_windows_command("7z") {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    if !allow_install {
        return Ok(None);
    }

    install_winget_package(SEVEN_ZIP_WINGET_ID)
        .context("Failed while installing 7-Zip with winget")?;

    if let Some(path) = locate_windows_command("7z") {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    anyhow::bail!(
        "7-Zip installation completed but '7z' is not visible in this session. Open a new terminal and retry."
    );
}

#[cfg(target_os = "windows")]
pub(crate) fn ensure_diskspd_for_session(allow_install: bool) -> Result<Option<PathBuf>> {
    if let Some(path) = locate_diskspd_executable() {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    if !allow_install {
        return Ok(None);
    }

    if is_command_available("winget") {
        if let Err(err) = install_winget_package(DISKSPD_WINGET_ID) {
            if let Some(path) = install_diskspd_download_fallback()
                .context("DiskSpd winget install failed and fallback bootstrap also failed")?
            {
                ensure_parent_dir_in_path(&path);
                return Ok(Some(path));
            }
            return Err(err).context("Failed while installing DiskSpd with winget");
        }
    } else if let Some(path) = install_diskspd_download_fallback()
        .context("winget is unavailable and DiskSpd fallback bootstrap failed")?
    {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    } else {
        anyhow::bail!("winget is not available and fallback bootstrap could not install DiskSpd.");
    }

    if let Some(path) = locate_diskspd_executable() {
        ensure_parent_dir_in_path(&path);
        return Ok(Some(path));
    }

    anyhow::bail!(
        "DiskSpd installation completed but 'diskspd' is not visible in this session. Open a new terminal and retry."
    );
}

#[cfg(target_os = "windows")]
pub(crate) fn ensure_blender_for_session(allow_install: bool) -> Result<Option<PathBuf>> {
    if let Some(path) = locate_blender_executable() {
        return Ok(Some(path));
    }
    if !allow_install {
        return Ok(None);
    }

    if is_command_available("winget") {
        install_winget_package(BLENDER_WINGET_ID).context("Failed while installing Blender")?;
    } else {
        anyhow::bail!("winget is not available. Install Blender manually.");
    }

    if let Some(path) = locate_blender_executable() {
        return Ok(Some(path));
    }

    anyhow::bail!(
        "Blender installation completed but blender.exe could not be located. Reboot or open a new terminal and retry."
    );
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn ensure_presentmon_for_session(_allow_install: bool) -> Result<Option<PathBuf>> {
    Ok(None)
}

#[cfg(target_os = "windows")]
pub(crate) fn locate_presentmon_executable() -> Option<PathBuf> {
    if let Some(path) = locate_windows_command("presentmon") {
        return Some(path);
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let links_candidate =
            PathBuf::from(&local_app_data).join("Microsoft/WinGet/Links/presentmon.exe");
        if links_candidate.exists() {
            return Some(links_candidate);
        }

        let packages_root = PathBuf::from(local_app_data).join("Microsoft/WinGet/Packages");
        if let Some(found) = find_presentmon_under(&packages_root) {
            return Some(found);
        }
    }

    if let Some(fallback) = fallback_presentmon_path() {
        if fallback.exists() {
            return Some(fallback);
        }
    }

    None
}

#[cfg(target_os = "windows")]
pub(crate) fn locate_diskspd_executable() -> Option<PathBuf> {
    if let Some(path) = locate_windows_command("diskspd") {
        return Some(path);
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let links_candidate =
            PathBuf::from(&local_app_data).join("Microsoft/WinGet/Links/diskspd.exe");
        if links_candidate.exists() {
            return Some(links_candidate);
        }
    }

    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("fps-tracker/tools/diskspd/diskspd.exe"))
        .filter(|p| p.exists())
}

#[cfg(target_os = "windows")]
pub(crate) fn locate_blender_executable() -> Option<PathBuf> {
    if let Some(path) = locate_windows_command("blender") {
        return Some(path);
    }

    let base = Path::new("C:\\Program Files\\Blender Foundation");
    let mut best: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let exe = path.join("blender.exe");
            if exe.is_file() {
                best = Some(exe);
            }
        }
    }
    best
}

#[cfg(target_os = "windows")]
fn fallback_presentmon_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("fps-tracker/tools/presentmon/presentmon.exe"))
}

#[cfg(target_os = "windows")]
fn locate_windows_command(command: &str) -> Option<PathBuf> {
    let where_exe = windows_system_command_path("where.exe");
    let output = Command::new(&where_exe)
        .arg(command)
        .output()
        .or_else(|_| Command::new("where").arg(command).output());

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = parse_where_output(&stdout) {
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_system_command_path(binary: &str) -> PathBuf {
    if let Some(root) = std::env::var_os("SystemRoot") {
        return PathBuf::from(root).join("System32").join(binary);
    }
    PathBuf::from(binary)
}

#[cfg(target_os = "windows")]
fn install_winget_package(package_id: &str) -> Result<()> {
    let winget_path = locate_windows_command("winget").ok_or_else(|| {
        anyhow::anyhow!(
            "winget is not available. Install '{}' manually.",
            package_id
        )
    })?;

    let attempts: &[&[&str]] = &[
        &[
            "install",
            "--id",
            package_id,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        &[
            "install",
            "--id",
            package_id,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        &[
            "install",
            "--id",
            package_id,
            "--exact",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
    ];

    // `winget` can fail if sources are stale (common on fresh installs and CI). Do one best-effort
    // `winget source update` pass and retry.
    let mut failures: Vec<String> = Vec::new();
    for pass in 0..2 {
        failures.clear();
        for args in attempts {
            let output = Command::new(&winget_path)
                .args(*args)
                .output()
                .context("Failed to execute winget")?;
            if output.status.success() {
                return Ok(());
            }
            let summary = first_non_empty_output_line(&output)
                .unwrap_or_else(|| format!("exit status {}", output.status));
            failures.push(format!("{} -> {}", args.join(" "), summary));
        }

        if pass == 0 {
            let _ = Command::new(&winget_path)
                .args(["source", "update"])
                .output();
        }
    }

    anyhow::bail!(
        "winget could not install {}. Attempted commands:\n{}",
        package_id,
        failures
            .iter()
            .map(|line| format!("  - {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[cfg(target_os = "windows")]
fn install_presentmon_download_fallback() -> Result<Option<PathBuf>> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Ok(None);
    };

    let install_dir = PathBuf::from(local_app_data)
        .join("fps-tracker")
        .join("tools")
        .join("presentmon");
    fs::create_dir_all(&install_dir).with_context(|| {
        format!(
            "Failed to create fallback PresentMon install directory {}",
            install_dir.display()
        )
    })?;

    let powershell = locate_windows_command("pwsh")
        .or_else(|| locate_windows_command("powershell"))
        .or_else(|| {
            let candidate = windows_system_command_path("WindowsPowerShell\\v1.0\\powershell.exe");
            candidate.exists().then_some(candidate)
        });

    let Some(powershell) = powershell else {
        return Ok(None);
    };

    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let script_path =
        std::env::temp_dir().join(format!("fps-tracker-presentmon-bootstrap-{epoch_ms}.ps1"));

    let install_dir_ps = install_dir.display().to_string().replace('\'', "''");
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$outDir = '{install_dir}'

function Assert-SignatureOrWarn([string] $path) {{
  $skip = $env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY
  if ($skip -and $skip -ne '0') {{ return }}

  $require = $env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY
  $signature = Get-AuthenticodeSignature -FilePath $path

  if ($require -and $require -ne '0') {{
    if ($signature.Status -ne 'Valid') {{
      throw ('Downloaded presentmon.exe signature status: ' + $signature.Status)
    }}
    if (-not $signature.SignerCertificate) {{
      throw 'Downloaded presentmon.exe has a Valid signature but no signer certificate.'
    }}
    return
  }}

  if ($signature.Status -ne 'Valid' -and $signature.Status -ne 'NotSigned') {{
    throw ('Downloaded presentmon.exe signature status: ' + $signature.Status)
  }}
  if ($signature.Status -eq 'Valid') {{
    if (-not $signature.SignerCertificate) {{
      throw 'Downloaded presentmon.exe has a Valid signature but no signer certificate.'
    }}
  }} else {{
    Write-Warning 'Downloaded presentmon.exe is not Authenticode signed. Proceeding because it was fetched from the official GitHub release.'
  }}
}}

$assetType = $null
$assetUrl = $null
$release = $null
try {{
  $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/GameTechDev/PresentMon/releases/latest' -Headers @{{ 'User-Agent' = 'fps-tracker' }} -TimeoutSec 45
}} catch {{
  $release = $null
}}

if ($release) {{
  # Newer PresentMon releases ship as standalone EXE/MSI (no zip). Prefer the x64 EXE if present.
  $assetExe = $release.assets `
    | Where-Object {{ $_.name -match '(?i)(x64|amd64)' -and $_.name -match '\.exe$' }} `
    | Sort-Object -Property size -Descending `
    | Select-Object -First 1
  if ($assetExe -and $assetExe.browser_download_url) {{
    $assetType = 'exe'
    $assetUrl = $assetExe.browser_download_url
  }}

  if (-not $assetUrl) {{
    # Legacy fallback: zip asset containing presentmon.exe somewhere inside.
    $assetZip = $release.assets `
      | Where-Object {{ $_.name -match '\.zip$' -and $_.name -notmatch '(?i)(source|src|symbols|pdb|debug)' }} `
      | Sort-Object -Property size -Descending `
      | Select-Object -First 1
    if ($assetZip -and $assetZip.browser_download_url) {{
      $assetType = 'zip'
      $assetUrl = $assetZip.browser_download_url
    }}
  }}
}}

if (-not $assetUrl) {{
  # GitHub API can be rate-limited; fall back to scraping releases/latest HTML.
  $page = Invoke-WebRequest -Uri 'https://github.com/GameTechDev/PresentMon/releases/latest' -Headers @{{ 'User-Agent' = 'fps-tracker' }} -TimeoutSec 45 -UseBasicParsing
  $content = $page.Content

  $mExe = [regex]::Match($content, 'href=\"(?<href>/GameTechDev/PresentMon/releases/download/[^\"]+(x64|amd64)[^\"]+?\.exe)\"', 'IgnoreCase')
  if ($mExe.Success) {{
    $assetType = 'exe'
    $assetUrl = 'https://github.com' + $mExe.Groups['href'].Value
  }} else {{
    $mZip = [regex]::Match($content, 'href=\"(?<href>/GameTechDev/PresentMon/releases/download/[^\"]+(x64|amd64)[^\"]+?\.zip)\"', 'IgnoreCase')
    if ($mZip.Success) {{
      $assetType = 'zip'
      $assetUrl = 'https://github.com' + $mZip.Groups['href'].Value
    }}
  }}
}}

if (-not $assetUrl) {{ throw 'Could not locate a PresentMon installable asset (x64/amd64 exe or zip) in the latest release.' }}

New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$dest = Join-Path $outDir 'presentmon.exe'

if ($assetType -eq 'exe') {{
  Invoke-WebRequest -Uri $assetUrl -OutFile $dest -TimeoutSec 120
  Assert-SignatureOrWarn $dest
  Write-Output $dest
  exit 0
}}

$zipPath = Join-Path $env:TEMP ('presentmon-' + [guid]::NewGuid().ToString('N') + '.zip')
$extractDir = Join-Path $env:TEMP ('presentmon-' + [guid]::NewGuid().ToString('N'))
Invoke-WebRequest -Uri $assetUrl -OutFile $zipPath -TimeoutSec 120
New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force
$exe = Get-ChildItem -Path $extractDir -Recurse -Filter 'presentmon.exe' | Select-Object -First 1
if (-not $exe) {{ throw 'presentmon.exe not found in downloaded archive.' }}
Copy-Item -Path $exe.FullName -Destination $dest -Force
Assert-SignatureOrWarn $dest
Write-Output $dest
"#,
        install_dir = install_dir_ps
    );

    fs::write(&script_path, script).with_context(|| {
        format!(
            "Failed to write PresentMon bootstrap script to {}",
            script_path.display()
        )
    })?;

    let output = Command::new(&powershell)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.display().to_string(),
        ])
        .output()
        .context("Failed to execute PresentMon bootstrap script")?;

    let _ = fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "PresentMon fallback bootstrap failed (exit: {}). stdout: {} stderr: {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }

    if let Some(path) = locate_presentmon_executable() {
        return Ok(Some(path));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(path_text) = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.to_ascii_lowercase().ends_with("presentmon.exe"))
    {
        let path = PathBuf::from(path_text);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    let candidate = install_dir.join("presentmon.exe");
    if candidate.exists() {
        return Ok(Some(candidate));
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
fn install_diskspd_download_fallback() -> Result<Option<PathBuf>> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Ok(None);
    };

    let install_dir = PathBuf::from(local_app_data)
        .join("fps-tracker")
        .join("tools")
        .join("diskspd");
    fs::create_dir_all(&install_dir).with_context(|| {
        format!(
            "Failed to create fallback DiskSpd install directory {}",
            install_dir.display()
        )
    })?;

    let powershell = locate_windows_command("pwsh")
        .or_else(|| locate_windows_command("powershell"))
        .or_else(|| {
            let candidate = windows_system_command_path("WindowsPowerShell\\v1.0\\powershell.exe");
            candidate.exists().then_some(candidate)
        });

    let Some(powershell) = powershell else {
        return Ok(None);
    };

    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let script_path =
        std::env::temp_dir().join(format!("fps-tracker-diskspd-bootstrap-{epoch_ms}.ps1"));

    let install_dir_ps = install_dir.display().to_string().replace('\'', "''");

    // Important: avoid `format!` here. PowerShell uses `{}` heavily, and missing an escape will
    // break compilation on non-Windows targets (CI builds all targets).
    let script_template = r#"$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$outDir = '__INSTALL_DIR__'

$zipPath = Join-Path $env:TEMP ('diskspd-' + [guid]::NewGuid().ToString('N') + '.zip')
$extractDir = Join-Path $env:TEMP ('diskspd-' + [guid]::NewGuid().ToString('N'))
Invoke-WebRequest -Uri 'https://aka.ms/getdiskspd' -OutFile $zipPath -TimeoutSec 60
New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force
$arch = $env:PROCESSOR_ARCHITECTURE
if (-not $arch) { $arch = '' }
$arch = $arch.ToLowerInvariant()
$want = if ($arch -match 'arm64') { 'arm64' } elseif ($arch -match 'x86') { 'x86' } else { 'amd64' }
$exe = Get-ChildItem -Path $extractDir -Recurse -Filter 'diskspd.exe' | Where-Object { $_.FullName.ToLowerInvariant().Contains(('\\' + $want + '\\')) } | Select-Object -First 1
if (-not $exe) { $exe = Get-ChildItem -Path $extractDir -Recurse -Filter 'diskspd.exe' | Select-Object -First 1 }
if (-not $exe) { throw 'diskspd.exe not found in downloaded archive.' }

$signature = Get-AuthenticodeSignature -FilePath $exe.FullName
if ($signature.Status -ne 'Valid') {
  throw ('Downloaded diskspd.exe signature status: ' + $signature.Status)
}
if (-not $signature.SignerCertificate -or $signature.SignerCertificate.Subject -notmatch 'Microsoft') {
  throw ('Downloaded diskspd.exe signer is unexpected: ' + $signature.SignerCertificate.Subject)
}

New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$dest = Join-Path $outDir 'diskspd.exe'
Copy-Item -Path $exe.FullName -Destination $dest -Force
Write-Output $dest
"#;
    let script = script_template.replace("__INSTALL_DIR__", &install_dir_ps);

    fs::write(&script_path, script).with_context(|| {
        format!(
            "Failed to write DiskSpd bootstrap script to {}",
            script_path.display()
        )
    })?;

    let output = Command::new(&powershell)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.display().to_string(),
        ])
        .output()
        .context("Failed to execute DiskSpd bootstrap script")?;

    let _ = fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "DiskSpd fallback bootstrap failed (exit: {}). stdout: {} stderr: {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }

    if let Some(path) = locate_diskspd_executable() {
        return Ok(Some(path));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(path_text) = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.to_ascii_lowercase().ends_with("diskspd.exe"))
    {
        let path = PathBuf::from(path_text);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    let candidate = install_dir.join("diskspd.exe");
    if candidate.exists() {
        return Ok(Some(candidate));
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
fn first_non_empty_output_line(output: &std::process::Output) -> Option<String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().map(str::trim).find(|line| !line.is_empty()) {
        return Some(line.to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

#[cfg(target_os = "windows")]
fn find_presentmon_under(packages_root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(packages_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name
            .to_ascii_lowercase()
            .starts_with("intel.presentmon.console_")
        {
            continue;
        }

        let candidate = path.join("presentmon.exe");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn ensure_parent_dir_in_path(executable: &Path) {
    let Some(parent) = executable.parent() else {
        return;
    };
    let current = std::env::var("PATH").unwrap_or_default();
    let updated = prepend_path_once(&current, &parent.display().to_string());
    std::env::set_var("PATH", updated);
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn dependency_bulk_install_command(statuses: &[DependencyStatus]) -> Option<String> {
    let mut packages: Vec<&str> = Vec::new();
    for item in statuses {
        if item.available {
            continue;
        }
        match item.name {
            "glmark2" => packages.push("glmark2"),
            "sysbench" => packages.push("sysbench"),
            "fio" => packages.push("fio"),
            "stress-ng" => packages.push("stress-ng"),
            _ => {}
        }
    }
    packages.sort_unstable();
    packages.dedup();
    if packages.is_empty() {
        return None;
    }

    if is_command_available("apt-get") {
        return Some(format!(
            "sudo apt-get update && sudo apt-get install -y {}",
            packages.join(" ")
        ));
    }
    if is_command_available("dnf") {
        return Some(format!("sudo dnf install -y {}", packages.join(" ")));
    }
    if is_command_available("pacman") {
        return Some(format!("sudo pacman -S --needed {}", packages.join(" ")));
    }
    if is_command_available("brew") {
        return Some(format!("brew install {}", packages.join(" ")));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{parse_where_output, prepend_path_once};

    #[test]
    fn parse_where_output_selects_first_non_empty_path() {
        let output = "\r\nC:\\Tools\\presentmon.exe\r\nC:\\Other\\presentmon.exe\r\n";
        let parsed = parse_where_output(output).expect("expected path");
        assert_eq!(parsed.to_string_lossy(), "C:\\Tools\\presentmon.exe");
    }

    #[test]
    fn prepend_path_once_adds_directory_at_front() {
        let updated = prepend_path_once("C:\\Windows\\System32;C:\\Windows", "C:\\Tools");
        assert_eq!(updated, "C:\\Tools;C:\\Windows\\System32;C:\\Windows");
    }

    #[test]
    fn prepend_path_once_does_not_duplicate_existing_directory() {
        let updated = prepend_path_once("C:\\Tools;C:\\Windows", "C:\\Tools");
        assert_eq!(updated, "C:\\Tools;C:\\Windows");
    }

    #[test]
    fn prepend_path_once_uses_windows_separator_for_windows_style_paths() {
        let updated = prepend_path_once("C:\\Windows", "C:\\Tools");
        assert_eq!(updated, "C:\\Tools;C:\\Windows");
    }
}
