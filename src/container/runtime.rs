use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

use crate::cli::Agent;
use crate::config::{get_claude_config_dir, get_claude_json_paths};
use crate::language::{
    detect_project_languages, ensure_language_tools, sync_node_modules_from_host, ProjectLanguage,
};
use crate::settings::load_settings;

use super::manage::{container_exists, is_container_running};

fn mount_agent_config(
    docker_run: &mut Command,
    agent_names: &[&str],
    current_dir: &Path,
    current_user: &str,
) {
    let home_dir = home::home_dir().unwrap_or_default();

    for agent in agent_names {
        let paths = [
            current_dir.join(format!(".{agent}")),
            home_dir.join(format!(".{agent}")),
            home_dir.join(".config").join(agent),
        ];

        for (i, host_path) in paths.iter().enumerate() {
            if host_path.exists() {
                let container_path = match i {
                    0 | 1 => format!("/home/{current_user}/.{agent}"),
                    _ => format!("/home/{current_user}/.config/{agent}"),
                };
                docker_run.args(["-v", &format!("{}:{}", host_path.display(), container_path)]);
                println!(
                    "Mounting {agent} config from: {} -> {}",
                    host_path.display(),
                    container_path
                );
                break;
            }
        }
    }
}

fn mount_language_configs(
    docker_run: &mut Command,
    languages: &[ProjectLanguage],
    current_user: &str,
) {
    let home_dir = home::home_dir().unwrap_or_default();

    for language in languages {
        for config_path in language.global_config_paths() {
            let host_path = home_dir.join(config_path);
            if host_path.exists() {
                let container_path = format!("/home/{current_user}/{config_path}");
                docker_run.args(["-v", &format!("{}:{}", host_path.display(), container_path)]);
                println!(
                    "Mounting {} config from: {} -> {}",
                    language.name(),
                    host_path.display(),
                    container_path
                );
            }
        }
    }
}

fn build_docker_image(current_user: &str) -> Result<()> {
    let dockerfile_content = create_dockerfile_content(current_user);
    let temp_dir = std::env::temp_dir();
    let dockerfile_path = temp_dir.join("Dockerfile.codesandbox");
    std::fs::write(&dockerfile_path, dockerfile_content).context("Failed to write Dockerfile")?;

    println!("Building Docker image...");
    let build_output = Command::new("docker")
        .args([
            "build",
            "-t",
            "codesandbox-image",
            "-f",
            dockerfile_path.to_str().unwrap(),
            ".",
        ])
        .current_dir(&temp_dir)
        .output()
        .context("Failed to build Docker image")?;

    if !build_output.status.success() {
        anyhow::bail!(
            "Docker build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );
    }

    Ok(())
}

fn build_run_command(
    container_name: &str,
    current_dir: &Path,
    additional_dir: Option<&Path>,
    agent: &Agent,
    current_user: &str,
    languages: &[ProjectLanguage],
) -> Result<(Command, Vec<NamedTempFile>)> {
    let mut docker_run = Command::new("docker");
    docker_run.args([
        "run",
        "-d",
        "-it",
        "--name",
        container_name,
        "-v",
        &format!("{}:{}", current_dir.display(), current_dir.display()),
    ]);

    // For Node.js projects, avoid mounting host node_modules by overlaying
    // an anonymous volume at the container's node_modules path. This prevents
    // install scripts from affecting the host machine.
    let project_has_node =
        current_dir.join("package.json").exists() || current_dir.join("node_modules").exists();
    if project_has_node {
        let node_modules_path = current_dir.join("node_modules");
        docker_run.args(["-v", &format!("{}", node_modules_path.display())]);
        println!(
            "Isolating node_modules with container volume: {}",
            node_modules_path.display()
        );
    }

    let settings = load_settings().unwrap_or_default();
    let mut env_file_overlays: Vec<NamedTempFile> = Vec::new();
    for file in settings.env_files.iter() {
        let target = current_dir.join(file);
        if target.is_file() {
            let tmp = NamedTempFile::new().context("Failed to create temp file for env masking")?;
            docker_run.args([
                "-v",
                &format!("{}:{}:ro", tmp.path().display(), target.display()),
            ]);
            println!("Excluding {} from container mount", target.display());
            env_file_overlays.push(tmp);
        }
    }

    if let Some(dir) = additional_dir {
        docker_run.args(["-v", &format!("{}:{}:ro", dir.display(), dir.display())]);
        println!("Mounting additional directory read-only: {}", dir.display());
    }

    if let Some(claude_config_dir) = get_claude_config_dir() {
        if claude_config_dir.exists() {
            docker_run.args([
                "-v",
                &format!(
                    "{}:/home/{}/.claude",
                    claude_config_dir.display(),
                    current_user
                ),
            ]);
            println!(
                "Mounting Claude config from: {}",
                claude_config_dir.display()
            );
        }
    }

    let claude_json_paths = get_claude_json_paths();
    for (i, config_path) in claude_json_paths.iter().enumerate() {
        if config_path.exists() {
            let container_path = if config_path.file_name().unwrap() == ".claude.json" {
                format!("/home/{}/.claude.json", current_user)
            } else {
                format!("/home/{}/.claude/config_{}.json", current_user, i)
            };
            docker_run.args([
                "-v",
                &format!("{}:{}", config_path.display(), container_path),
            ]);
            println!(
                "Mounting Claude config from: {} -> {}",
                config_path.display(),
                container_path
            );
        }
    }

    let serena_paths = [
        current_dir.join(".serena"),
        home::home_dir().unwrap_or_default().join(".serena"),
    ];
    for serena_path in serena_paths.iter() {
        if serena_path.exists() {
            let container_serena_path = format!("/home/{}/.serena", current_user);
            docker_run.args([
                "-v",
                &format!("{}:{}", serena_path.display(), container_serena_path),
            ]);
            println!(
                "Mounting Serena MCP config from: {} -> {}",
                serena_path.display(),
                container_serena_path
            );
            break;
        }
    }

    match agent {
        Agent::Gemini => {
            mount_agent_config(&mut docker_run, &["gemini"], current_dir, current_user);
        }
        Agent::Codex => {
            // Map Codex config directories (e.g., ~/.codex) into the container
            mount_agent_config(&mut docker_run, &["codex"], current_dir, current_user);
        }
        Agent::Qwen => {
            mount_agent_config(&mut docker_run, &["qwen"], current_dir, current_user);
        }
        Agent::Cursor => {
            mount_agent_config(&mut docker_run, &["cursor"], current_dir, current_user);
        }
        _ => {}
    }

    if !languages.is_empty() {
        println!(
            "Detected languages: {:?}",
            languages.iter().map(|l| l.name()).collect::<Vec<_>>()
        );
        mount_language_configs(&mut docker_run, languages, current_user);
    }

    docker_run.args(["codesandbox-image", "/bin/bash"]);

    Ok((docker_run, env_file_overlays))
}

pub async fn create_container(
    container_name: &str,
    current_dir: &Path,
    additional_dir: Option<&Path>,
    agent: &Agent,
    skip_permission_flag: Option<&str>,
    shell: bool,
    attach: bool,
) -> Result<()> {
    let current_user = env::var("USER").unwrap_or_else(|_| "ubuntu".to_string());
    build_docker_image(&current_user)?;
    let languages = detect_project_languages(current_dir);
    let (mut docker_run, _env_file_overlays) = build_run_command(
        container_name,
        current_dir,
        additional_dir,
        agent,
        &current_user,
        &languages,
    )?;
    let run_output = docker_run
        .output()
        .context("Failed to run Docker container")?;
    if !run_output.status.success() {
        anyhow::bail!(
            "Failed to create container: {}",
            String::from_utf8_lossy(&run_output.stderr)
        );
    }
    ensure_language_tools(container_name, &languages)?;
    // For Node.js projects, copy host node_modules into the isolated volume in container
    sync_node_modules_from_host(container_name, current_dir, &languages)?;
    if attach {
        attach_to_container(
            container_name,
            current_dir,
            agent,
            false,
            skip_permission_flag,
            shell,
        )
        .await
    } else {
        Ok(())
    }
}

pub async fn resume_container(
    container_name: &str,
    agent: &Agent,
    agent_continue: bool,
    skip_permission_flag: Option<&str>,
    shell: bool,
    attach: bool,
) -> Result<()> {
    println!("Resuming container: {}", container_name);

    if !container_exists(container_name)? {
        anyhow::bail!("Container '{}' does not exist", container_name);
    }

    if !is_container_running(container_name)? {
        println!("Starting stopped container: {}", container_name);
        let start_output = Command::new("docker")
            .args(&["start", container_name])
            .output()
            .context("Failed to start container")?;

        if !start_output.status.success() {
            anyhow::bail!(
                "Failed to start container: {}",
                String::from_utf8_lossy(&start_output.stderr)
            );
        }
    } else {
        println!("Container is already running");
    }

    if attach {
        let current_dir = env::current_dir().context("Failed to get current directory")?;
        attach_to_container(
            container_name,
            &current_dir,
            agent,
            agent_continue,
            skip_permission_flag,
            shell,
        )
        .await
    } else {
        Ok(())
    }
}

pub fn build_agent_command(
    current_dir: &Path,
    agent: &Agent,
    agent_continue: bool,
    skip_permission_flag: Option<&str>,
) -> String {
    let path_str = current_dir.display().to_string();
    let escaped = path_str.replace('\'', "'\\''");
    let mut command = format!(
        "cd '{}' && export PATH=\"$HOME/.local/bin:$PATH\" && {}",
        escaped,
        agent.command()
    );

    if agent_continue {
        command.push_str(" --continue");
    }

    if let Some(flag) = skip_permission_flag {
        command.push(' ');
        command.push_str(flag);
    }

    command
}

async fn attach_to_container(
    container_name: &str,
    current_dir: &Path,
    agent: &Agent,
    agent_continue: bool,
    skip_permission_flag: Option<&str>,
    shell: bool,
) -> Result<()> {
    let allocate_tty = atty::is(atty::Stream::Stdout) && atty::is(atty::Stream::Stdin);
    if shell {
        println!("Attaching to container shell...");
    } else {
        println!("Attaching to container and starting {}...", agent);
    }

    // Ensure the directory structure exists in the container
    let mkdir_status = Command::new("docker")
        .args(&[
            "exec",
            container_name,
            "mkdir",
            "-p",
            &current_dir.display().to_string(),
        ])
        .status()
        .context("Failed to create directory structure in container")?;

    if !mkdir_status.success() {
        println!("Warning: Failed to create directory structure in container");
    }

    if shell {
        let path_str = current_dir.display().to_string();
        let escaped = path_str.replace('\'', "'\\''");
        let command = format!("cd '{}' && source ~/.bashrc && exec /bin/bash", escaped);
        let mut args = vec!["exec"];
        if allocate_tty {
            args.push("-it");
        } else {
            args.push("-i");
        }
        args.push(container_name);
        args.extend(["/bin/bash", "-c", &command]);
        let attach_status = Command::new("docker")
            .args(&args)
            .status()
            .context("Failed to attach to container")?;
        if !attach_status.success() {
            println!(
                "You can manually attach with: docker exec -it {} /bin/bash",
                container_name
            );
        }
        return Ok(());
    }

    let command = build_agent_command(current_dir, agent, agent_continue, skip_permission_flag);

    let mut args = vec!["exec"];
    if allocate_tty {
        args.push("-it");
    } else {
        args.push("-i");
    }
    args.push(container_name);
    args.extend(["/bin/bash", "-c", &command]);
    let attach_status = Command::new("docker")
        .args(&args)
        .status()
        .context("Failed to attach to container")?;

    if !attach_status.success() {
        println!("Failed to start {} automatically.", agent);
        println!(
            "You can manually attach with: docker exec -it {} /bin/bash",
            container_name
        );
    }

    Ok(())
}

fn create_dockerfile_content(user: &str) -> String {
    format!(
        r#"FROM ubuntu:22.04

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Update and install required packages
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    python3 \
    python3-pip \
    sudo \
    ca-certificates \
    gnupg \
    lsb-release \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js v22
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y nodejs

# Install Go
RUN wget https://go.dev/dl/go1.24.5.linux-amd64.tar.gz && \
    tar -C /usr/local -xzf go1.24.5.linux-amd64.tar.gz && \
    rm go1.24.5.linux-amd64.tar.gz

# Install Rust and Cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    /root/.cargo/bin/rustup component add rustfmt clippy

# Create user with sudo privileges
RUN useradd -m -s /bin/bash {user} && \
    echo "{user} ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers
USER root
# Install Claude Code
RUN npm install -g @anthropic-ai/claude-code
RUN npm install -g @google/gemini-cli
RUN npm install -g @openai/codex
RUN npm install -g @qwen-code/qwen-code@latest

# Install Cursor CLI
RUN curl https://cursor.com/install -fsS | bash 
# Switch to user
USER {user}
WORKDIR /home/{user}

# Set up PATH environment for the user session
ENV PATH="/home/{user}/.local/bin:/usr/local/go/bin:/home/{user}/.cargo/bin:$PATH"

# Install Rust for the user and ensure cargo is available
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    ~/.cargo/bin/rustup component add rustfmt clippy

# Install uv for Python tooling
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

# Add Go, Rust, Cargo, and uv to PATH
RUN echo 'export PATH="/usr/local/go/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"' >> ~/.bashrc

# Set working directory to home
WORKDIR /home/{user}

# Keep container running
CMD ["/bin/bash"]
"#,
        user = user
    )
}
