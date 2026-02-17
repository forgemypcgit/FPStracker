# Package Manager Publishing

This project can automatically open Homebrew and winget PRs after each GitHub release.

## Workflows involved

- Release build + signing: `.github/workflows/release.yml`
- Package manager PR automation: `.github/workflows/publish-package-managers.yml`

## Required GitHub repository settings

In GitHub UI:

1. Open your repo.
2. Go to `Settings` -> `Secrets and variables` -> `Actions`.
3. Add the secrets and variables below exactly as named.

### Release signing secrets (optional, recommended)

If `COSIGN_*` secrets are not configured, the `Release` workflow still publishes binaries and checksums, but will skip signature generation (`*.sig` + `cosign.pub`).

Installer scripts:
- Always verify checksums.
- Verify signatures when signature assets are present.
- Can be forced to require signatures via `FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY=1`.

#### `COSIGN_PRIVATE_KEY_B64` (secret)

Create signing keys locally once:

```bash
cosign generate-key-pair
```

This creates:

- `cosign.key` (private key)
- `cosign.pub` (public key)

Set `COSIGN_PRIVATE_KEY_B64` to the base64-encoded contents of `cosign.key`.

Examples:

```bash
base64 -w0 cosign.key > cosign.key.b64   # GNU/Linux
base64 cosign.key | tr -d '\n' > cosign.key.b64   # macOS
```

#### `COSIGN_PASSWORD` (secret)

Set to the same password used when generating `cosign.key`.

Release workflow safeguard:
- The generated `dist/cosign.pub` is checked against the embedded pinned keys in:
  - `scripts/install.sh`
  - `scripts/install.ps1`

### Optional Windows Authenticode signing

If these secrets are configured, the Windows release binary is signed before packaging:

- `WINDOWS_CODESIGN_CERT_PFX_B64`
- `WINDOWS_CODESIGN_CERT_PASSWORD`
- `WINDOWS_CODESIGN_TIMESTAMP_URL` (optional; default timestamp server is used if unset)

How to prepare:

1. Export your code-signing certificate as `.pfx`.
2. Base64-encode it into one line and store as `WINDOWS_CODESIGN_CERT_PFX_B64`.
3. Store the export password as `WINDOWS_CODESIGN_CERT_PASSWORD`.

## Homebrew automation setup

### 1) Create or choose tap repository

Example: `forgemypcgit/homebrew-tap`

### 2) Add repository variable

- `HOMEBREW_TAP_REPO` = `forgemypcgit/homebrew-tap`

### 3) Create token for the tap repo

Recommended: fine-grained personal access token scoped to the tap repo with:

- Repository permissions -> `Contents: Read and write`
- Repository permissions -> `Pull requests: Read and write`

Save it as:

- `HOMEBREW_TAP_TOKEN` (secret)

Result:

- Workflow updates `Formula/fps-tracker.rb`
- Opens PR to `main` in your tap repo

## winget automation setup

### 1) Fork winget-pkgs

Create a fork of `microsoft/winget-pkgs` under your account/org.

### 2) Add repository variables

- `WINGET_TARGET_REPO` = `microsoft/winget-pkgs`
- `WINGET_FORK_REPO` = `<your-user-or-org>/winget-pkgs`

### 3) Create token for fork push + PR creation

Recommended for cross-repo PR automation: classic PAT.

- If fork is public: scope `public_repo`
- If fork is private: scope `repo`

Store as:

- `WINGET_PAT` (secret)

Result:

- Workflow updates `manifests/f/ForgeMyPC/FPSTracker/<version>/`
- Pushes branch to your fork
- Opens PR against `microsoft/winget-pkgs` `master`

## Optional CLI setup with GitHub CLI

If you prefer command line configuration:

```bash
gh secret set COSIGN_PRIVATE_KEY_B64 < cosign.key.b64
gh secret set COSIGN_PASSWORD
gh secret set WINDOWS_CODESIGN_CERT_PFX_B64 < cert.pfx.b64
gh secret set WINDOWS_CODESIGN_CERT_PASSWORD
gh secret set WINDOWS_CODESIGN_TIMESTAMP_URL --body "http://timestamp.digicert.com"
gh secret set HOMEBREW_TAP_TOKEN
gh secret set WINGET_PAT

gh variable set HOMEBREW_TAP_REPO --body "forgemypcgit/homebrew-tap"
gh variable set WINGET_TARGET_REPO --body "microsoft/winget-pkgs"
gh variable set WINGET_FORK_REPO --body "<your-user-or-org>/winget-pkgs"
```

## How to run release end-to-end

1. Push a tag, for example:

```bash
git tag v0.2.0
git push origin v0.2.0
```

2. Wait for `Release` workflow to publish artifacts/signatures.
3. `Publish Package Managers` runs automatically on release publish.

## Manual fallback

If package-manager secrets/variables are missing, publish jobs are skipped.

Manual path:

1. Download `fps-tracker.rb` and `ForgeMyPC.FPSTracker*.yaml` from release assets.
2. Commit changes to tap/fork repositories.
3. Open PRs manually.
