# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

agentsandbox (Agent Sandbox) is a Rust CLI tool that creates isolated Ubuntu Docker containers with development agents pre-installed for development work. The tool automatically handles Docker container lifecycle, mounts the current directory as a workspace, and transfers configuration for seamless development.

## Build and Development Commands

### Build Commands

```bash
# Build in debug mode
cargo build

# Build optimized release version
cargo build --release

# Install locally using cargo
cargo install --path .

# Install to system (requires release build first)
sudo cp target/release/agentsandbox /usr/local/bin/
```

### Testing and Development

```bash
# Run the tool
cargo run

# Run with the continue flag
cargo run -- --continue

# Check code formatting
cargo fmt --check

# Run clippy for linting
cargo clippy

# Run any tests
cargo test
```

## Architecture Overview

The codebase is structured into focused modules:

-   **`main.rs`**: Entry point handling command-line parsing, Docker availability checks, and orchestrating container creation or resumption
-   **`cli.rs`**: Command-line interface definition using clap with support for resuming previous containers via `--continue` flag
-   **`config.rs`**: Claude configuration discovery and management, handling multiple config locations (.claude directory, XDG, local .claude.json files)
-   **`container.rs`**: Core Docker operations including container creation, lifecycle management, and dynamic Dockerfile generation
-   **`state.rs`**: Persistent state management for tracking the last created container in `~/.config/agentsandbox/last_container`

### Key Design Patterns

1. **Configuration Discovery**: The tool searches multiple standard locations for Claude configs and automatically mounts them into containers
2. **Container Lifecycle**: Supports both creating new containers and resuming existing ones with state tracking
3. **Dynamic Dockerfile**: Generates Ubuntu 22.04-based containers with comprehensive development tools (Node.js, Go, Rust, Python, build tools)
4. **User Context Preservation**: Maintains user identity and sudo privileges within containers

### Dependencies and External Tools

-   **Docker**: Required for container operations - tool validates availability before proceeding
-   **Claude Code**: Automatically installed via npm in containers and can be launched with agent-specific permission-skipping flags (e.g., `--dangerously-skip-permissions`, `--yolo`) configured in `settings.json`
-   **Development Tools**: Containers include Node.js v22, Go 1.24.5, Rust/Cargo, Python3, and build-essential

### Container Environment

Containers are created with:

-   Base: Ubuntu 22.04
-   Working directory: `/workspace` (mounted from current directory)
-   User: Matches host user with sudo privileges
-   Claude configs: Auto-mounted from `~/.claude`, XDG locations, or local `.claude.json`
-   Development tools: Pre-installed and added to PATH via `.bashrc`

## Container Management

The tool generates contextual container names using the format `csb-{agent}-{dir}-{branch}-{yymmddhhmm}` and tracks the last container for resumption. State is persisted in `~/.config/agentsandbox/last_container`.
