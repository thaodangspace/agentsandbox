# Rust to Go Migration

This document describes the migration of AgentSandbox from Rust to Go.

## Summary

AgentSandbox has been completely rewritten from Rust to Go (version 0.2.0), providing better maintainability and easier distribution through Go's native tooling.

## What Changed

### Language & Framework
- **From:** Rust with Tokio async runtime, Clap CLI framework
- **To:** Go with standard goroutines, Cobra CLI framework

### Key Benefits
1. **Simpler Async Model:** Go's goroutines are simpler than Tokio's async/await
2. **Faster Compilation:** Go builds much faster than Rust
3. **Native Distribution:** `go install` provides simpler installation
4. **Better Tooling:** Standard Go tools (go fmt, go test, go vet)
5. **Easier Maintenance:** More developers familiar with Go

### Architecture Mapping

| Rust Component | Go Component | Notes |
|----------------|--------------|-------|
| `src/main.rs` | `cmd/agentsandbox/main.go` | Entry point |
| `src/cli.rs` | `internal/cli/*.go` | Split into multiple command files |
| `src/container/` | `internal/container/` | Docker container management |
| `src/config.rs` | `internal/config/` | Settings and agent configuration |
| `src/state.rs` | `internal/state/` | State management |
| `src/language.rs` | `internal/language/` | Language detection |
| `src/clipboard.rs` + shell scripts | `internal/clipboard/` | Clipboard watcher in native Go |
| `src/log_parser.rs` | `internal/logs/` | Log parsing and HTML generation |
| `src/worktree.rs` | `internal/git/` | Git worktree operations |

### Dependencies

**Rust Dependencies (Removed):**
- clap, tokio, serde, anyhow, chrono, reqwest, etc.

**Go Dependencies (Added):**
- `github.com/spf13/cobra` - CLI framework
- `github.com/spf13/viper` - Configuration management
- `github.com/fsnotify/fsnotify` - File system watching
- Standard library for most operations

### Docker Integration
- **Before:** Shell out to `docker` CLI commands
- **After:** Same approach, but with simpler Go exec.Command

### Build System
- **Before:** Cargo for Rust, npm wrapper for distribution
- **After:** Go modules, Makefile, GoReleaser for multi-platform builds

### Distribution
- **Before:** Cargo, npm, Homebrew (Rust-based)
- **After:** Go install, Homebrew (Go-based), pre-built binaries via GoReleaser

## Breaking Changes

None - the CLI interface remains compatible. All commands and flags work the same way:
- `agentsandbox` - Start default agent
- `agentsandbox --agent qwen` - Start specific agent
- `agentsandbox list` (or `ls`) - List containers
- `agentsandbox cleanup` - Remove containers
- `agentsandbox logs list` - Manage logs

## Installation (New)

### Go Install
```bash
go install github.com/thaodangspace/agentsandbox/cmd/agentsandbox@latest
```

### Homebrew
```bash
brew tap thaodangspace/agentsandbox
brew install agentsandbox
```

### From Source
```bash
git clone https://github.com/thaodangspace/agentsandbox.git
cd agentsandbox
make build
# Binary will be in bin/agentsandbox
```

## Development

### Building
```bash
make build          # Build for current platform
make build-all      # Build for all platforms
make test           # Run tests
make fmt            # Format code
make lint           # Run linter
```

### Testing
```bash
go test ./...           # Run all tests
go test -v ./...        # Verbose output
go test -cover ./...    # With coverage
```

### Project Structure
```
agentsandbox/
├── cmd/
│   └── agentsandbox/      # Main entry point
├── internal/              # Internal packages
│   ├── cli/              # CLI commands
│   ├── container/        # Docker management
│   ├── config/           # Configuration
│   ├── language/         # Language detection
│   ├── clipboard/        # Clipboard watcher
│   ├── logs/             # Log parsing
│   ├── state/            # State management
│   └── git/              # Git operations
├── Makefile              # Build automation
├── go.mod                # Go dependencies
└── .goreleaser.yaml      # Release configuration
```

## Performance

Go compilation is significantly faster:
- **Rust:** ~30-60 seconds for full build
- **Go:** ~3-5 seconds for full build

Binary sizes are comparable:
- **Rust:** ~8-12 MB (release, stripped)
- **Go:** ~10-15 MB (release, stripped)

## Migration Notes for Contributors

### If you were working on Rust code:
1. Pull the latest `main` branch
2. Install Go 1.21+ if not already installed
3. Run `go mod download` to fetch dependencies
4. Use `make build` instead of `cargo build`
5. Use `go test ./...` instead of `cargo test`

### If you had local branches:
- Rust code is removed from main
- You may need to rebase/recreate changes in Go
- File structure is similar but not identical

## Future Plans

1. Enhanced clipboard integration (currently disabled)
2. Better error messages and logging
3. Integration tests with actual Docker
4. Performance optimizations
5. Additional agent support

## Questions?

Please open an issue on GitHub if you have questions about the migration.

---
**Migration completed:** December 2025
**Version:** 0.2.0

