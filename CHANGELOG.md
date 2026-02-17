# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.7] - 2026-02-13

### Fixed

- Windows PresentMon runtime probe no longer fails when `presentmon --help` returns a non-zero exit code while printing help.

## [0.2.6] - 2026-02-13

### Added

- Windows synthetic component scoring via WinSAT (CPU/GPU/RAM/SSD) and submission fields.
- Assisted flow to review/keep/edit synthetic scores before submitting.
- Optional 0.1% low FPS capture in CLI and UI.
- `doctor` command and hardened Windows dependency checks for live capture tooling.

### Fixed

- UI validation now matches backend FPS limits to prevent late submit failures.

## [0.2.5] - 2026-02-10

### Added

- Initial public release.

## [0.2.2] - 2026-02-10

### Added

- Signed release artifacts (cosign) when signatures are configured.

## [0.2.0] - 2026-02-10

### Added

- Governance docs (`CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`)
- Issue/PR templates and Dependabot configuration
- `Quality` and `Security` CI workflows
- Release and publishing runbooks (`docs/RELEASE_CHECKLIST.md`, `docs/PUBLISH.md`)

### Changed

- License set to PolyForm Noncommercial 1.0.0
- Windows focus detection moved to native WinAPI path for lower polling overhead
- Release and package-manager workflows hardened for explicit tag and winget config requirements
- Installer scripts hardened for CI/mirror usage

### Fixed

- Strict focus handling no longer fails solely on unknown foreground process lookup
- Installer smoke tests now wait for local server readiness before install execution
