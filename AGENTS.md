# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust CLI and server modules (e.g., `cli.rs`, `server.rs`, `container/`).
- `tests/`: Rust tests (e.g., `cli_test.rs`, `server_test.rs`).
- `web/`: Vite + React UI (`src/`, `index.html`, Tailwind config).
- `bin/`, `install.js`, `package.json`: npm wrapper for distributing the CLI.
- Root configs: `Cargo.toml`, `Cargo.lock`, `README.md`, `HOMEBREW_SETUP.md`.

## Build, Test, and Development Commands
- Build (Rust): `cargo build` (release: `cargo build --release`).
- Run CLI: `cargo run -- --help` or installed `agentsandbox --help`.
- Test (Rust): `cargo test`.
- Format/Lint: `cargo fmt --all` and `cargo clippy -- -D warnings`.
- Web UI: `cd web && npm ci && npm run dev` (build: `npm run build`, preview: `npm run preview`).

## Coding Style & Naming Conventions
- Rust: use `rustfmt` defaults; `snake_case` for modules/functions, `CamelCase` for types; prefer clear, small modules under `src/`.
- TypeScript/React: functional components in `PascalCase`; keep props/types explicit; match existing file naming in `web/src/`.
- No JS/TS linter is enforced; keep diffs minimal and consistent with nearby code.

## Testing Guidelines
- Framework: Rust’s built-in test harness; integration tests live in `tests/*.rs` and unit tests via `#[test]`.
- Add tests for new CLI flags, parsing, and server endpoints. Run locally with `cargo test` before opening a PR.
- UI: no formal tests; add lightweight checks only if necessary for your change.

## Commit & Pull Request Guidelines
- Commits: prefer Conventional Commits (e.g., `feat: add stop/restart`, `fix: enable local network ws`). Keep commits atomic.
- PRs: include a clear description, linked issues, and steps to validate (commands or screenshots for UI). Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass.

## Security & Configuration Tips
- Do not commit secrets or `.env*` files. The CLI masks env files per settings.
- User config lives at `~/.config/agentsandbox/settings.json` (e.g., permission flags, env masks). Verify Docker is running before local tests.
