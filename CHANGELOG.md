# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
