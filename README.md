# Code Sandbox

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/your-repo/code-sandbox)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-required-blue.svg)](https://www.docker.com)

A robust Rust CLI tool that creates isolated Ubuntu Docker containers with development agents pre-installed. Code Sandbox provides a secure, disposable environment for running AI assistants like Claude, Gemini, Codex, and Qwen, ensuring their actions are confined to the container while maintaining a clean, reproducible workspace.

## Table of Contents

-   [Overview](#overview)
-   [Features](#features)
-   [Demo](#demo)
-   [Prerequisites](#prerequisites)
-   [Installation](#installation)
-   [Usage](#usage)
-   [Configuration](#configuration)
-   [API](#api)
-   [Troubleshooting](#troubleshooting)
-   [Contributing](#contributing)
-   [License](#license)

## Overview

### Why Sandbox an AI Agent?

Running an agent inside an isolated container provides several benefits:

-   Protects your host machine by keeping the agent's file system changes and
    processes separate from your environment
-   Ensures a clean, reproducible workspace with all dependencies installed
    from scratch
-   Makes it easy to experiment with untrusted code or dependencies and then
    discard the container when finished

## Demo

[![Watch the video](https://img.youtube.com/vi/HghV3XvWKBQ/maxresdefault.jpg)](https://youtu.be/HghV3XvWKBQ)

## Features

### Core Functionality

-   **Multi-Agent Support**: Compatible with Claude, Gemini, Codex, and Qwen development agents
-   **Automatic Workspace Mounting**: Seamlessly mounts your current directory to same path with the host machine in the container
-   **Configuration Management**: Automatically copies and applies your agent configurations
-   **Intelligent Naming**: Generates contextual container names to prevent conflicts (`csb-{agent}-{dir}-{branch}-{yymmddhhmm}`)

### Workflow Management

-   **Session Continuity**: Resume your last container session with `codesandbox --continue`
-   **Git Integration**: Create and use git worktrees for isolated branch development
-   **Cleanup Utilities**: Efficient container management and cleanup tools
-   **Directory Mounting**: Add additional read-only directories for extended workspace access

## Prerequisites

### System Requirements

-   **Docker**: Version 20.10 or higher, installed and running
-   **Rust**: Version 1.70 or higher (for building from source)
-   **Git**: For repository cloning and worktree functionality

### Platform Support

-   Linux (tested on Ubuntu 20.04+, Fedora 35+)
-   macOS (Intel and Apple Silicon)
-   Windows (with WSL2 and Docker Desktop)

## Installation

### Method 1: Build from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/your-org/code-sandbox.git
cd code-sandbox

# Build the release binary
cargo build --release

# Install globally (optional)
sudo cp target/release/codesandbox /usr/local/bin/
```

### Method 2: Install via Cargo

```bash
# Install directly from the local repository
cargo install --path .

# Or install from crates.io (when published)
cargo install codesandbox
```

### Method 3: Download Pre-built Binaries

Visit the [Releases](https://github.com/your-org/code-sandbox/releases) page to download pre-built binaries for your platform.

## Usage

### Quick Start

Navigate to your project directory and run:

```bash
codesandbox
```

This command will:

1. **Create a Container**: Generate a new Ubuntu container with a unique identifier
2. **Mount Workspace**: Bind your current directory to `/workspace` in the container
3. **Configure Agent**: Copy your agent configuration files (e.g., `.claude` from `~/.claude`)
4. **Launch Agent**: Start the default agent (Claude) within the container environment

### Advanced Usage

#### Specify a Different Agent

```bash
# Use Qwen instead of Claude
codesandbox --agent qwen

# Use Gemini
codesandbox --agent gemini
```

#### Mount Additional Directories

```bash
# Add a read-only reference directory
codesandbox --add_dir /path/to/reference/repo
```

#### Session Management

```bash
# Resume the last container from this directory
codesandbox --continue

# List all containers and optionally attach
codesandbox ls
```

#### Git Workflow Integration

```bash
# Create and use a git worktree for isolated branch work
codesandbox --worktree feature-branch
```

## Connecting to the Container

After the container is created, you can connect to it using:

```bash
docker exec -it <container-name> /bin/bash
```

The container name will be displayed when `codesandbox` runs.

## Listing Existing Containers

List all sandbox containers created from the current directory and optionally attach to one:

```bash
codesandbox ls
```

You will be shown a numbered list of containers. Enter a number to attach or press Enter to cancel.

## API

### REST API for Container Changes

This repository includes an optional HTTP server that reports file changes inside a running sandbox container.

Start the server:

```bash
codesandbox serve
```

Run it as a background daemon:

```bash
codesandbox serve -d
```

The server listens on port 6789. Query the changes for a specific container:

```bash
curl http://localhost:6789/api/changed/<container-name>
```

The response lists changed files along with their git status and diff contents.

### Container Contents

-   **Base**: Ubuntu 22.04
-   **Tools**: curl, wget, git, build-essential, python3, nodejs, npm
-   **User**: `ubuntu` with sudo privileges
-   **Agent**: Claude Code pre-installed (other agents can be started if available)
-   **Working Directory**: `/workspace` (your mounted folder)

## Configuration

The tool automatically detects and mounts your Claude configuration from:

-   `~/.claude` (standard location)
-   `$XDG_CONFIG_HOME/claude` (XDG standard)

Additional behavior can be configured via `settings.json` located at
`~/.config/codesandbox/settings.json`. Example:

```json
{
    "auto_remove_minutes": 30,
    "skip_permission_flags": {
        "claude": "--dangerously-skip-permissions",
        "gemini": "--yolo",
        "qwen": "--yolo"
    },
    "env_files": [
        ".env",
        ".env.local"
    ]
}
```

The `skip_permission_flags` map assigns a permission-skipping flag to each
agent. When launching an agent, the corresponding flag is appended to the
command.

Environment files listed in `env_files` are masked from the container by
overlaying them with empty temporary files, keeping sensitive data on the host.

## Cleanup

To remove all containers created from the current directory:

```bash
codesandbox --cleanup
```

To remove the built image:

```bash
docker rmi codesandbox-image
```

## Troubleshooting

-   **Docker not found**: Ensure Docker is installed and running
-   **Permission denied**: Make sure your user is in the `docker` group
-   **Agent fails to start**: You can manually start it with `docker exec -it <container> <agent>`

## Contributing

We welcome contributions to Code Sandbox! Here's how you can help:

### Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
    ```bash
    git clone https://github.com/thaodangspace/code-sandbox.git
    cd code-sandbox
    ```
3. **Create a feature branch** from `main`:
    ```bash
    git checkout -b feature/your-feature-name
    ```

### Development Setup

1. **Install Rust** (if not already installed):

    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```

2. **Install dependencies** and build:

    ```bash
    cargo build
    ```

3. **Run tests**:
    ```bash
    cargo test
    ```

### Making Changes

-   **Follow Rust conventions**: Use `cargo fmt` and `cargo clippy`
-   **Write tests** for new functionality
-   **Update documentation** as needed
-   **Keep commits atomic** and write clear commit messages

### Submitting Changes

1. **Push your changes** to your fork:

    ```bash
    git push origin feature/your-feature-name
    ```

2. **Create a Pull Request** with:
    - Clear description of the changes
    - Reference to any related issues
    - Screenshots/demos for UI changes

### Code Style

-   Follow the existing code style
-   Run `cargo fmt` before committing
-   Ensure `cargo clippy` passes without warnings
-   Add documentation for public APIs

### Reporting Issues

When reporting bugs, please include:

-   Operating system and version
-   Docker version
-   Rust version (`rustc --version`)
-   Steps to reproduce the issue
-   Expected vs actual behavior

### Feature Requests

For new features:

-   Check existing issues first
-   Clearly describe the use case
-   Propose the API/interface if applicable
-   Consider backward compatibility

Thank you for contributing to Code Sandbox!

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### MIT License Summary

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

**Made with ❤️ by the Code Sandbox contributors**
