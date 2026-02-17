# Installation Guide

## Supported binaries

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## One-command install

### Linux/macOS

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
bash /tmp/fps-tracker-install.sh
```

Optional explicit version:

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
FPS_TRACKER_VERSION=v0.2.5 bash /tmp/fps-tracker-install.sh
```

Skip signature verification (not recommended):

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 bash /tmp/fps-tracker-install.sh
```

Require signatures (fail if signature assets are missing):

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY=1 bash /tmp/fps-tracker-install.sh
```

Use a pinned cosign public key (recommended if you want to avoid trusting `cosign.pub` from the release):

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
FPS_TRACKER_COSIGN_PUBKEY="$HOME/.config/fps-tracker/cosign.pub" bash /tmp/fps-tracker-install.sh
```

### Windows

```powershell
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

Optional explicit version:

```powershell
$env:FPS_TRACKER_VERSION='v0.2.5'
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

Skip signature verification (not recommended):

```powershell
$env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY='1'
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

Require signatures (fail if signature assets are missing):

```powershell
$env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY='1'
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

Use a pinned cosign public key:

```powershell
$env:FPS_TRACKER_COSIGN_PUBKEY = "$HOME\\AppData\\Local\\fps-tracker\\cosign.pub"
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

## Package managers

### Homebrew (macOS/Linux)

```bash
brew install --formula https://github.com/forgemypcgit/FPStracker/releases/latest/download/fps-tracker.rb
```

### winget (Windows)

```powershell
winget install --id ForgeMyPC.FPSTracker
```

Note: winget availability depends on the manifest being merged in `microsoft/winget-pkgs`.

## Local install destinations

- Linux/macOS default: `~/.local/bin/fps-tracker`
- Windows default: `%LOCALAPPDATA%\fps-tracker\bin\fps-tracker.exe`

Override target directory:

- Linux/macOS: `FPS_TRACKER_INSTALL_DIR=/custom/bin`
- Windows: `$env:FPS_TRACKER_INSTALL_DIR='C:\custom\bin'`

Override download base URL (self-hosted mirror/testing):

- Linux/macOS: `FPS_TRACKER_BASE_URL=https://mirror.example.com/fps-tracker`
- Windows: `$env:FPS_TRACKER_BASE_URL='https://mirror.example.com/fps-tracker'`

Skip Windows PATH mutation:

- `$env:FPS_TRACKER_SKIP_PATH_UPDATE='1'`

## Verify install

```bash
fps-tracker --help
fps-tracker install-info
fps-tracker doctor
```

## Optional Benchmark Tools (Synthetic Baseline)

FPS Tracker can optionally run third-party synthetic benchmarks to improve the quality of your submission.
These tools are installed only with your approval (via prompts), and each tool is governed by its own
upstream license/terms.

- Windows: WinSAT (built-in), PresentMon, DiskSpd, 7-Zip, Blender
- Linux: glmark2, sysbench, fio, stress-ng

If you prefer not to install any of them, you can continue with empty synthetic fields (or enter values
manually if you trust your source).

Windows-only auto-fix for missing live-capture dependency:

```powershell
fps-tracker doctor --fix
fps-tracker doctor --fix --yes
fps-tracker doctor --windows-runtime
fps-tracker doctor --fix --yes --windows-runtime
```

`doctor --fix` can install `Intel.PresentMon.Console` via `winget`, or use the fallback bootstrap path when `winget` is unavailable, and then re-check runtime availability.

Use `--yes` for non-interactive shells (for example CI runners or scripted setup).

## Windows troubleshooting

### `winget` command not found

`winget` ships with **App Installer** on modern Windows.  
If `winget` is missing, install/update App Installer from Microsoft Store, then open a new terminal.

`fps-tracker doctor --fix` can still bootstrap PresentMon without `winget`, but installing App Installer is recommended for long-term package management.

### PowerShell blocks script execution

If your environment enforces restrictive execution policy, run installation in a process-scoped bypass (the bypass applies only to this invocation, not system-wide):

```powershell
$script = Join-Path $env:TEMP 'fps-tracker-install.ps1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $script
powershell -NoProfile -ExecutionPolicy Bypass -File $script
```

### PresentMon installed but capture still fails

Check runtime first:

```powershell
presentmon --help
```

If that command fails, repair/install Microsoft Visual C++ Redistributable and retry `fps-tracker doctor --fix`.

### SmartScreen warning on first run

Unsigned/low-reputation binaries can trigger Windows Defender SmartScreen prompts.
This is reputation-based and expected for new releases before enough trust signals accumulate.

## Integrity checks

Installer scripts validate:
- SHA-256 checksum (`*.sha256`) always
- cosign signature (`*.sig`) against `cosign.pub` when signature assets are present
  - `cosign` download itself is verified via a pinned SHA-256 for the default cosign version

## Update

Re-run the installer command.

## Uninstall

Delete the installed binary from your install directory and remove the directory from PATH if no longer needed.

## Publish automation

See `docs/PUBLISH.md` for Homebrew/winget PR automation from release events.
