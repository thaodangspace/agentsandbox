# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust CLI and server modules (e.g., `cli.rs`, `server.rs`, `container/`).
- `tests/`: Rust tests (e.g., `cli_test.rs`, `server_test.rs`).
- `bin/`, `install.js`, `package.json`: npm wrapper for distributing the CLI.
- Root configs: `Cargo.toml`, `Cargo.lock`, `README.md`, `HOMEBREW_SETUP.md`.

## Build, Test, and Development Commands
- Build (Rust): `cargo build` (release: `cargo build --release`).
- Run CLI: `cargo run -- --help` or installed `codesandbox --help`.
- Test (Rust): `cargo test`.
- Format/Lint: `cargo fmt --all` and `cargo clippy -- -D warnings`.

## Coding Style & Naming Conventions
- Rust: use `rustfmt` defaults; `snake_case` for modules/functions, `CamelCase` for types; prefer clear, small modules under `src/`.

## Testing Guidelines
- Framework: Rust’s built-in test harness; integration tests live in `tests/*.rs` and unit tests via `#[test]`.
- Add tests for new CLI flags, parsing, and server endpoints. Run locally with `cargo test` before opening a PR.

## Commit & Pull Request Guidelines
- Commits: prefer Conventional Commits (e.g., `feat: add stop/restart`, `fix: enable local network ws`). Keep commits atomic.
- PRs: include a clear description, linked issues, and steps to validate (commands or screenshots for UI). Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass.

## Security & Configuration Tips
- Do not commit secrets or `.env*` files. The CLI masks env files per settings.
- User config lives at `~/.config/codesandbox/settings.json` (e.g., permission flags, env masks). Verify Docker is running before local tests.

