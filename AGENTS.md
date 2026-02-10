# FPStracker (Repo Agent Notes)

This file captures the repo-specific decisions and operational steps that should remain stable across context compactions.

## Repo Purpose

`fps-tracker` is a Rust CLI (with an embedded web UI) for collecting and submitting FPS benchmark data.

Key directories:
- `src/`: Rust CLI + embedded web server
- `ui/`: React UI bundled into the Rust binary
- `scripts/`: installer scripts + package manager manifest generator
- `.github/workflows/`: CI and release workflows

## Local Dev Commands

Rust:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `cargo build --release`

UI:
- `cd ui && npm ci && npm run build`

## CI / Releases

Workflows:
- `Quality` (`.github/workflows/quality.yml`): fmt/clippy/test/build (Linux + Windows)
- `Smoke` (`.github/workflows/smoke.yml`): basic binary smoke + installer end-to-end (Linux + Windows installer)
- `Release` (`.github/workflows/release.yml`): builds artifacts for Linux/macOS/Windows, generates checksums, optionally signs with cosign, publishes GitHub Release assets
- `publish-package-managers` (`.github/workflows/publish-package-managers.yml`): prepares Homebrew + winget manifests; PR publishing is gated on secrets/vars (skips if not configured)

Release trigger:
- Push a tag like `v0.2.0` to trigger the `Release` workflow, or run it via `workflow_dispatch` with a `tag`.

### Cosign Signing (Optional but Recommended)

If `COSIGN_*` secrets are configured, `Release` produces:
- `*.sig` for each artifact
- `cosign.pub` as a release asset

GitHub Secrets expected (repo-level):
- `COSIGN_PRIVATE_KEY` (full contents of `cosign.key`)
- `COSIGN_PASSWORD` (the password used to generate the key)

See: `docs/PUBLISH.md` and `docs/RELEASE_CHECKLIST.md`.

## Installers

Unix:
- `scripts/install.sh`

Windows:
- `scripts/install.ps1`

Notes:
- Installers verify artifact checksum and will verify cosign signatures when signature assets are present.
- For local installer smoke tests, `install.sh` allows `http://127.0.0.1` / `http://localhost` (CI uses a local HTTP server). Non-loopback HTTP is refused unless `FPS_TRACKER_ALLOW_INSECURE_HTTP=1`.

## Dependabot ("detectabot")

GitHub Dependabot is enabled via `.github/dependabot.yml`.
It automatically opens PRs/branches like `dependabot/cargo/...` and can create many CI runs.

To reduce noise, consider:
- grouping updates
- lowering `open-pull-requests-limit`
- running weekly instead of daily
- or disabling Dependabot in repo settings

## Git / Identity Hygiene (Email + SSH)

### Hide Email on Commits

On GitHub:
- Settings -> Emails -> enable "Keep my email addresses private"
- Optionally enable "Block command line pushes that expose my email"

Locally:
- Use your GitHub `noreply` email for commits (format is shown in GitHub email settings).

### SSH Host Alias Used Here

This workspace uses `~/.ssh/config` host `github.com-forgemypcgit` pointing to GitHub with a dedicated key:
- private key: `~/.ssh/id_ed25519_forgemypcgit`
- public key: `~/.ssh/id_ed25519_forgemypcgit.pub`

Repo remote uses:
- `git@github.com-forgemypcgit:forgemypcgit/FPStracker.git`

## License Choice

Project uses:
- PolyForm Noncommercial 1.0.0 (`LICENSE`)

Intent:
- source-visible for learning/review and contributions
- no commercial use by third parties

