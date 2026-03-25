# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

agentsandbox (Agent Sandbox) is a Go CLI tool that creates isolated Ubuntu Docker containers with development agents pre-installed for development work. The tool automatically handles Docker container lifecycle, mounts the current directory as a workspace, and transfers configuration for seamless development.

## Build and Development Commands

### Build Commands

```bash
# Build in debug mode
go build -o agentsandbox ./cmd/agentsandbox

# Build optimized release version
go build -ldflags="-s -w" -o agentsandbox ./cmd/agentsandbox

# Install locally using go
go install ./cmd/agentsandbox

# Install to system
sudo cp $(go env GOPATH)/bin/agentsandbox /usr/local/bin/
```

### Testing and Development

```bash
# Run the tool
go run ./cmd/agentsandbox

# Run with flags
go run ./cmd/agentsandbox --continue
go run ./cmd/agentsandbox --agent gemini
go run ./cmd/agentsandbox --shell

# Run tests
go test ./...

# Run tests with verbose output
go test -v ./...

# Check code formatting
go fmt ./...

# Run vet for static analysis
go vet ./...

# Install dependencies
go mod tidy
```

## Architecture Overview

The codebase is structured into focused modules:

- **`cmd/agentsandbox/main.go`**: Entry point that calls the CLI package
- **`internal/cli/`**: Command-line interface using cobra
  - `root.go`: Root command and default action (start container)
  - `start.go`: Start subcommand (alias for default behavior)
  - `attach.go`: Attach to running container
  - `cleanup.go`: Remove old containers
  - `copy_config.go`: Copy agent configs to container
  - `list.go`: List containers
  - `logs.go`: View container logs
- **`internal/config/`**: Configuration management
  - `agent.go`: Agent types (Claude, Gemini, Codex, Qwen, Cursor)
  - `settings.go`: Settings file handling
- **`internal/container/`**: Core Docker operations
  - `runtime.go`: Dockerfile generation, container creation, attach
  - `manager.go`: Container lifecycle management
  - `naming.go`: Container naming conventions
- **`internal/clipboard/`**: Clipboard image sharing (X11 only)
- **`internal/state/`**: Persistent state for last container tracking
- **`internal/language/`**: Language detection and toolchain installation
- **`internal/git/`**: Git worktree management
- **`internal/logs/`**: Log parsing and viewing

### Key Design Patterns

1. **Configuration Discovery**: Searches multiple standard locations for agent configs and mounts them into containers
2. **Container Lifecycle**: Supports creating new containers and resuming existing ones with state tracking
3. **Dynamic Dockerfile**: Generates Ubuntu 22.04-based containers with AI agents and development tools
4. **User Context Preservation**: Maintains user identity and sudo privileges within containers

### Dependencies and External Tools

- **Docker**: Required for container operations - tool validates availability before proceeding
- **AI Agents**: Claude Code, Gemini CLI, Codex, Qwen, Cursor - installed via their respective install scripts in containers
- **Development Tools**: Containers include Node.js v22, Go, Rust/Cargo, Python3, and build-essential based on detected project languages

### Container Environment

Containers are created with:

- Base: Ubuntu 22.04
- Working directory: `/workspace` (mounted from current directory)
- User: Matches host user with sudo privileges
- Agent configs: Auto-mounted from `~/.claude`, `~/.gemini`, etc., XDG locations, or local `.json` files
- AI agents: Installed to user's `~/.local/bin` during Docker build

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

## Command-Line Flags

```bash
--agent <name>       Agent to start (claude, gemini, codex, qwen, cursor)
--continue           Resume the last created container
--add-dir <path>     Additional directory to mount read-only
--worktree <branch>  Create and use a git worktree for the specified branch
--shell              Attach to container shell without starting the agent
--no-clipboard       Disable clipboard image sharing
-p, --port <spec>    Publish container port (HOST:CONTAINER format)
```
