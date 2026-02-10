# Installation Guide

## Supported binaries

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## One-command install

### Linux/macOS

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash
```

Optional explicit version:

```bash
FPS_TRACKER_VERSION=v0.2.0 curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash
```

Skip signature verification (not recommended):

```bash
FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash
```

Require signatures (fail if signature assets are missing):

```bash
FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY=1 curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash
```

Use a pinned cosign public key (recommended if you want to avoid trusting `cosign.pub` from the release):

```bash
FPS_TRACKER_COSIGN_PUBKEY="$HOME/.config/fps-tracker/cosign.pub" \
  curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh | bash
```

### Windows

```powershell
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex
```

Optional explicit version:

```powershell
$env:FPS_TRACKER_VERSION='v0.2.0'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex
```

Skip signature verification (not recommended):

```powershell
$env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY='1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex
```

Require signatures (fail if signature assets are missing):

```powershell
$env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY='1'
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex
```

Use a pinned cosign public key:

```powershell
$env:FPS_TRACKER_COSIGN_PUBKEY = "$HOME\\AppData\\Local\\fps-tracker\\cosign.pub"
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing | iex
```

## Package managers

### Homebrew (macOS/Linux)

```bash
brew install --formula https://github.com/forgemypcgit/FPStracker/releases/latest/download/fps-tracker.rb
```

### winget (Windows)

```powershell
winget install --id PCBuilder.FPSTracker
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
```

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
