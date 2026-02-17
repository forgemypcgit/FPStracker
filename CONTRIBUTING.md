# Contributing to FPS Tracker

Contributions are welcome. This document defines the engineering bar for changes.

## License

By submitting a pull request, you agree that your contribution is licensed under
this repository's license (see `LICENSE`).

## Development setup

1. Install Rust (stable toolchain).
2. Clone this repository.
3. Build once:

```bash
cargo build
```

## Quality gates (must pass)

Run these before opening a PR:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

## Engineering standards

- Keep changes small and focused.
- Prefer explicit errors over silent fallbacks.
- Keep platform behavior consistent across Linux, macOS, and Windows.
- Avoid introducing breaking CLI behavior without updating docs and release notes.

## Testing guidance

- Add or update tests for every behavior change.
- For Windows-specific capture/focus behavior, include at least one test for fallback behavior.
- For installer or release flow updates, validate corresponding GitHub workflow logic.

## Commit and PR guidance

- Use clear commit messages (imperative mood).
- Include a short "what changed" and "why" in PR description.
- List verification commands and outcomes in PR description.
- Keep one logical change per commit when practical.

## Security

If you find a vulnerability, do not open a public issue. See `SECURITY.md`.
