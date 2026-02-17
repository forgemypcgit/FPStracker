# Release Checklist

Use this checklist before publishing a new version.

## 1) Quality checks

Run locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

## 2) Windows confidence checks

On a clean Windows machine or VM:

1. Install via `scripts/install.ps1` path (or release asset).
2. Run:
   - `fps-tracker --help`
   - `fps-tracker benchmark preview --help`
   - `fps-tracker doctor --windows-runtime`
   - `fps-tracker doctor --fix --yes --windows-runtime`
   - `presentmon --help`
3. Run at least one PresentMon capture with `--focus-policy strict` and a real game process.
4. Validate alt-tab behavior:
   - brief alt-tab should not fail immediately
   - sustained unfocus should fail in strict mode
5. Validate process mismatch behavior:
   - wrong process name must fail with remediation guidance

## 3) Signing and package manager prerequisites

Ensure secrets/variables are configured (see `docs/PUBLISH.md`).

Notes:
- `COSIGN_*` are optional but recommended (without them, releases publish without signature assets).
- `WINDOWS_CODESIGN_*` are optional. If present, Windows binaries are Authenticode-signed before packaging.
- Release workflow verifies embedded installer keys in `scripts/install.sh` and `scripts/install.ps1` match generated `dist/cosign.pub`.

- `COSIGN_PRIVATE_KEY_B64`
- `COSIGN_PASSWORD`
- `WINDOWS_CODESIGN_CERT_PFX_B64` (optional)
- `WINDOWS_CODESIGN_CERT_PASSWORD` (optional, required if cert is set)
- `WINDOWS_CODESIGN_TIMESTAMP_URL` (optional)
- `HOMEBREW_TAP_TOKEN`
- `WINGET_PAT`
- `HOMEBREW_TAP_REPO`
- `WINGET_TARGET_REPO`
- `WINGET_FORK_REPO`

## 4) Publish

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

## 5) Post-release validation

1. Verify release assets exist:
   - binaries
   - `*.sha256`
   - if `COSIGN_*` secrets are configured: `*.sig` + `cosign.pub`
2. Verify Homebrew PR opened.
3. Verify winget PR opened.
4. Record release notes summary in `CHANGELOG.md`.
