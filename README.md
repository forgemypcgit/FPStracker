# FPS Tracker

Rust-based FPS benchmark collection CLI with a browser-mode UI.

[![Quality](https://github.com/forgemypcgit/FPStracker/actions/workflows/quality.yml/badge.svg)](https://github.com/forgemypcgit/FPStracker/actions/workflows/quality.yml)
[![Smoke](https://github.com/forgemypcgit/FPStracker/actions/workflows/smoke.yml/badge.svg)](https://github.com/forgemypcgit/FPStracker/actions/workflows/smoke.yml)
[![Security](https://github.com/forgemypcgit/FPStracker/actions/workflows/security.yml/badge.svg)](https://github.com/forgemypcgit/FPStracker/actions/workflows/security.yml)

## License

This project is **source-available** under the **PolyForm Noncommercial 1.0.0** license.

- Noncommercial use (including forks) is allowed under the license terms.
- Commercial use is not allowed without separate permission from the licensor.

See `LICENSE` for details.

## Project docs

- Contributor guide: `CONTRIBUTING.md`
- Security policy: `SECURITY.md`
- Code of conduct: `CODE_OF_CONDUCT.md`
- Package publishing automation: `docs/PUBLISH.md`
- Install details: `docs/INSTALL.md`
- Release runbook: `docs/RELEASE_CHECKLIST.md`
- Changelog: `CHANGELOG.md`

## Install

### Linux/macOS

```bash
curl -fsSL https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.sh -o /tmp/fps-tracker-install.sh
bash /tmp/fps-tracker-install.sh
```

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/forgemypcgit/FPStracker/main/scripts/install.ps1 -UseBasicParsing -OutFile $env:TEMP\\fps-tracker-install.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File $env:TEMP\\fps-tracker-install.ps1
```

### Homebrew (macOS/Linux)

```bash
brew install --formula https://github.com/forgemypcgit/FPStracker/releases/latest/download/fps-tracker.rb
```

### winget (Windows, after manifest publish)

```powershell
winget install --id ForgeMyPC.FPSTracker
```

### From source

```bash
cargo build --release
./target/release/fps-tracker --help
```

Install scripts always verify checksum, and verify cosign signatures when the release includes signature assets (`*.sig` + `cosign.pub`).

For air-gapped/debug use only:
- Linux/macOS: `FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1`
- Windows: `$env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY='1'`
- Require signatures (fail if signature assets are missing):
  - Linux/macOS: `FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY=1`
  - Windows: `$env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY='1'`
- Custom artifact mirror/base URL:
  - Linux/macOS: `FPS_TRACKER_BASE_URL=https://mirror.example.com/fps-tracker`
  - Windows: `$env:FPS_TRACKER_BASE_URL='https://mirror.example.com/fps-tracker'`

## Quick start

```bash
fps-tracker app
```

Or terminal flow:

```bash
fps-tracker start
```

## Capture preview (high-FPS hardened)

```bash
fps-tracker benchmark preview \
  --source auto \
  --game "Counter-Strike 2" \
  --process-name cs2.exe \
  --focus-policy strict \
  --pause-on-unfocus true \
  --process-validation true \
  --poll-ms 100
```

Key flags:
- `--process-name`: target process for strict focus/process safety.
- If `--game` is a known title and `--process-name` is omitted, fps-tracker auto-selects a suggested executable and prints alternatives.
- `--focus-policy strict|lenient`: strict blocks uncertain focus/process states.
- `--pause-on-unfocus true|false`: drop samples while unfocused.
- `--process-validation true|false`: enforce process-level validation.
- `--poll-ms 50..500`: file-tail polling interval.
- `--max-frame-time-ms`: ignore outlier frame times above threshold.
- Windows: if `presentmon` is required (explicit `--source presentmon`, or `--source auto` with no MangoHud fallback) and missing, preview offers a secure install path and re-checks availability. It prefers `winget` (`Intel.PresentMon.Console`) and falls back to a verified local bootstrap when `winget` is unavailable.

## Other commands

```bash
fps-tracker detect
fps-tracker games
fps-tracker feedback
fps-tracker config
fps-tracker install-info
fps-tracker doctor
fps-tracker doctor --fix
fps-tracker doctor --fix --yes
fps-tracker doctor --windows-runtime
fps-tracker doctor --fix --yes --windows-runtime
```

## Release artifacts

Tagging `v*` triggers `.github/workflows/release.yml` and publishes:
- Linux: `fps-tracker-x86_64-unknown-linux-gnu.tar.gz`
- macOS Intel: `fps-tracker-x86_64-apple-darwin.tar.gz`
- macOS Apple Silicon: `fps-tracker-aarch64-apple-darwin.tar.gz`
- Windows x64: `fps-tracker-x86_64-pc-windows-msvc.zip`
- Windows x64 (direct binary): `fps-tracker-x86_64-pc-windows-msvc.exe`
- Per-asset checksum files + consolidated `SHA256SUMS`
- Optional per-asset cosign signatures (`*.sig`) + `cosign.pub` when `COSIGN_*` release secrets are configured
- Package-manager artifacts:
  - `fps-tracker.rb` (Homebrew formula)
  - `ForgeMyPC.FPSTracker*.yaml` (winget manifests)
- Optional publish automation workflow for Homebrew/winget PRs:
  - `docs/PUBLISH.md`

## CI

- `Quality` workflow: format + clippy + tests + release build (Linux and Windows)
- `Smoke` workflow: cross-platform smoke checks + installer end-to-end tests
- `Security` workflow: weekly and PR dependency vulnerability scan (`cargo-audit`)

## Security model

This tool is designed around non-injecting capture workflows (MangoHud/PresentMon/CapFrameX imports). It does not inject into games. Anti-cheat compatibility still depends on game policy and can change over time.
