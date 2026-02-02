# Agent Sandbox

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/your-repo/code-sandbox)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-required-blue.svg)](https://www.docker.com)
<a href="https://webuild.community">
<img src="https://raw.githubusercontent.com/webuild-community/badge/master/svg/by.svg" alt="By Vietnamese" />
</a>

## Why Sandbox an AI Agent?

Running an AI agent with direct access to your host machine is risky. An agent could inadvertently or maliciously:

-   Install dangerous packages (`npm install some-malware`)
-   Execute destructive commands (`rm -rf /`, `pkill`)
-   Run sensitive operations (`git push --force`, `db:migrate`)

Using an isolated container provides critical benefits:

-   **Security**: Protects your host machine by keeping the agent's file system changes and processes separate from your environment.
-   **Integrity**: Ensures a clean, reproducible workspace with all dependencies installed from scratch.
-   **Flexibility**: Makes it easy to experiment with untrusted code or dependencies and then discard the container when finished.

## Highlights

-   **Multi-Agent Support**: Compatible with Claude, Gemini, Codex, Qwen, and Cursor development agents
-   **Automatic Workspace Mounting**: Seamlessly mounts your current directory to same path with the host machine in the container
-   **Node Modules Isolation**: For Node.js projects, `node_modules` is overlaid with a container-only volume. Existing host `node_modules` are copied to the container on first run to accelerate setup.
-   **Configuration Management**: Automatically copies and applies your agent configurations
-   **Language Tooling**: Detects common project languages and installs missing package managers like Cargo, npm, pip, Composer, Go, or Bundler

## Demo

[![Watch the video](https://img.youtube.com/vi/HghV3XvWKBQ/maxresdefault.jpg)](https://youtu.be/HghV3XvWKBQ)

## Requirements

-   Docker 20.10+ (running and accessible to your user)
-   Go 1.21+ (only required for building from source)
-   Git
-   Linux, macOS (Intel or Apple Silicon), or Windows via WSL2 + Docker Desktop

## Quick Start

1. `cd` into the project you want to explore.
2. Run `agentsandbox`.
3. The tool builds a fresh Ubuntu container, mounts the current directory at `/workspace`, copies your agent configuration (for example `~/.claude`), and launches the default agent.

## Installation

### Homebrew (macOS/Linux)

```bash
brew tap thaodangspace/agentsandbox
brew install agentsandbox
```

### Go Install

```bash
go install github.com/thaodangspace/agentsandbox/cmd/agentsandbox@latest
```

### Build from Source

```bash
git clone https://github.com/thaodangspace/agentsandbox.git
cd agentsandbox
make build
# or: go build -o bin/agentsandbox ./cmd/agentsandbox
sudo cp bin/agentsandbox /usr/local/bin/  # optional
```

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/thaodangspace/agentsandbox/releases) page.

## Everyday Usage

### Start the default agent

```bash
agentsandbox
```

### Launch a specific agent

```bash
agentsandbox --agent qwen
agentsandbox --agent gemini
agentsandbox --agent cursor
```

### Mount extra directories (read-only)

```bash
agentsandbox --add-dir /path/to/reference/repo
```

### Manage sessions

```bash
agentsandbox --continue   # resume the last container for this directory
agentsandbox ls           # list containers tied to the current directory
agentsandbox ps           # list every running sandbox across directories
```

### Shell access only

```bash
agentsandbox --shell
```

### Attach with Docker

```bash
docker exec -it <container-name> /bin/bash
```

The container name appears in the startup log (format: `agentsandbox-{project_dir}`).

## Container Layout

-   Base image: Ubuntu 22.04
-   User: `ubuntu` (sudo-enabled)
-   Mounted workspace: `/workspace`
-   Tooling: curl, wget, git, build-essential, python3, nodejs, npm
-   Agents: Claude Code pre-installed (others start when requested)

## Configuration

Agent Sandbox automatically looks for Claude configuration in `~/.claude` or `$XDG_CONFIG_HOME/claude`. Global settings live at `~/.config/agentsandbox/settings.json`, for example:

```json
{
    "auto_remove_minutes": 60,
    "skip_permission_flags": {
        "claude": "--dangerously-skip-permissions",
        "gemini": "--yolo",
        "qwen": "--yolo",
        "cursor": "--yolo"
    },
    "env_files": [
        ".env",
        ".env.local",
        ".env.development.local",
        ".env.test.local",
        ".env.production.local"
    ]
}
```

Environment files listed under `env_files` are shadowed by empty overlays inside the container so secrets never leave your host machine.

## Maintenance

```bash
agentsandbox cleanup        # remove containers created from the current directory
docker rmi agentsandbox-image
```

## Development & Contributing

1. Fork the repository and clone your fork:
    ```bash
    git clone https://github.com/thaodangspace/agentsandbox.git
    cd agentsandbox
    ```
2. Build and test:
    ```bash
    make build
    make test
    make fmt
    make lint
    ```
3. Build for multiple platforms:
    ```bash
    make build-all
    # or use GoReleaser for full release:
    goreleaser release --snapshot --clean
    ```
4. Push your branch and open a pull request with a clear description, linked issues, and validation steps.

## Troubleshooting

-   **Docker not found**: confirm Docker Desktop/daemon is running and you are in the `docker` group.
-   **Permission errors**: re-log after adding yourself to the `docker` group or run with elevated privileges.
-   **Agent fails to launch**: use `docker exec -it <container-name> <agent>` to inspect the container and logs.
-   **Slow startup**: first run may copy dependencies like `node_modules`; subsequent runs reuse the cached overlay volume.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for full text.

---

Made with ❤️ by the Agent Sandbox contributors.
