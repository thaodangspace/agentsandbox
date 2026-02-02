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

-   **`main.rs`**: Entry point handling command-line parsing, Docker availability checks, clipboard watcher management, and orchestrating container creation or resumption
-   **`cli.rs`**: Command-line interface definition using clap with support for resuming previous containers via `--continue` flag and disabling clipboard with `--no-clipboard`
-   **`clipboard.rs`**: Clipboard directory management, watcher PID tracking, and process monitoring for image sharing between host and containers
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

## Clipboard Image Sharing

> **Note:** Clipboard sharing is temporarily disabled while we investigate stability issues. The CLI prints a warning and skips mounting the clipboard directory regardless of the `--no-clipboard` flag.

The information below captures the intended workflow once the feature returns.

agentsandbox supports sharing images from the host clipboard to containers, making it easy to paste screenshots and images directly into agents running inside containers.

### How It Works

1. **Automatic Clipboard Watcher**: When you start agentsandbox, a background process monitors your X11 clipboard for images
2. **Image Detection**: When you copy an image (PNG, JPG, JPEG) to your clipboard, it's automatically saved to `~/.config/agentsandbox/clipboard/`
3. **Container Access**: The clipboard directory is mounted read-only into containers at `/workspace/.clipboard/`
4. **Helper Command**: Inside containers, use the `clipboard` command to get the path to the latest image

### Usage Examples

```bash
# Inside the container, get the latest clipboard image path
clipboard

# Use with Claude Code
claude code $(clipboard)

# List all clipboard images
clipboard list
```

### Disabling Clipboard Sharing

If you don't want clipboard monitoring, use the `--no-clipboard` flag:

```bash
agentsandbox --no-clipboard
```

While the integration is disabled this flag is redundant but left in place for future compatibility.

### Requirements

- **X11 Display Server**: Currently supports X11 (most common on Linux)
- **xclip**: Required for clipboard monitoring (usually pre-installed)
  ```bash
  # Install if needed
  sudo apt-get install xclip
  ```

### Technical Details

- **Clipboard Directory**: `~/.config/agentsandbox/clipboard/`
- **Image Format**: Saved as `clipboard-YYYYMMDD-HHMMSS.{ext}`
- **Automatic Cleanup**: Keeps only the 10 most recent images to prevent disk bloat
- **Container Mount**: Read-only at `/workspace/.clipboard/`
- **Helper Script**: Available at `/usr/local/bin/clipboard` inside containers

## Container Management

The tool generates container names using the format `agentsandbox-{project_dir}` and tracks the last container for resumption. State is persisted in `~/.config/agentsandbox/last_container`.
