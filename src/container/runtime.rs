use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use tempfile::NamedTempFile;

use crate::cli::Agent;
use crate::clipboard::ensure_clipboard_dir;
use crate::config::{get_claude_config_dir, get_claude_json_paths};
use crate::language::{
    detect_project_languages, ensure_language_tools, sync_node_modules_from_host, ProjectLanguage,
};
use crate::settings::load_settings;
use crate::state::{
    load_container_run_command, load_image_agent_versions, prepare_session_log,
    save_container_run_command, save_image_agent_versions,
};

use super::manage::{container_exists, is_container_running};

fn mount_agent_config(
    docker_run: &mut Command,
    agent_names: &[&str],
    current_dir: &Path,
    current_user: &str,
) {
    let home_dir = home::home_dir().unwrap_or_default();

    for agent in agent_names {
        let mut mounted_for_agent = false;
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
                mounted_for_agent = true;
                break;
            }
        }

        if mounted_for_agent {
            let mut label = String::new();
            for (idx, part) in agent.split('_').enumerate() {
                if idx > 0 {
                    label.push(' ');
                }
                let mut chars = part.chars();
                if let Some(first) = chars.next() {
                    label.extend(first.to_uppercase());
                }
                label.extend(chars);
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

fn parse_version_output(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let stdout_text = String::from_utf8_lossy(stdout);
    for line in stdout_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let stderr_text = String::from_utf8_lossy(stderr);
    for line in stderr_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn detect_host_agent_version(agent: &Agent) -> Option<String> {
    let output = Command::new(agent.command())
        .arg("--version")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_version_output(&output.stdout, &output.stderr)
}

fn versions_match(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn query_agent_version_in_image(agent: &Agent) -> Result<Option<String>> {
    let check_command = format!("{} --version", agent.command());
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "agentsandbox-image",
            "bash",
            "-lc",
            &check_command,
        ])
        .output()
        .context(format!(
            "Failed to inspect {} version inside sandbox image",
            agent
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            println!(
                "Warning: unable to determine {} version in sandbox image: {}",
                agent,
                stderr.trim()
            );
        }
        return Ok(None);
    }

    Ok(parse_version_output(&output.stdout, &output.stderr))
}

fn capture_agent_versions_from_image() -> Result<HashMap<String, String>> {
    let mut versions = HashMap::new();

    for agent in [
        Agent::Claude,
        Agent::Gemini,
        Agent::Codex,
        Agent::Qwen,
        Agent::Cursor,
    ] {
        if let Some(version) = query_agent_version_in_image(&agent)? {
            versions.insert(agent.command().to_string(), version);
        }
    }

    Ok(versions)
}

fn evaluate_agent_version_status(agent: &Agent) -> Result<(Option<String>, Option<String>, bool)> {
    let recorded_versions = load_image_agent_versions().unwrap_or_else(|err| {
        println!(
            "Warning: failed to read cached agent version information: {}",
            err
        );
        HashMap::new()
    });
    let image_version = recorded_versions
        .get(agent.command())
        .map(|v| v.to_string());
    let host_version = detect_host_agent_version(agent);
    let mut force_rebuild = false;

    match (&host_version, &image_version) {
        (Some(host), Some(image)) if !versions_match(host, image) => {
            println!(
                "Detected {} version mismatch between host ({}) and sandbox image ({}).",
                agent, host, image
            );
            println!(
                "Rebuilding the sandbox image to refresh {}. After the rebuild, please update the environment that remains out of sync.",
                agent
            );
            force_rebuild = true;
        }
        (Some(host), None) => {
            println!(
                "No recorded {} version for sandbox image. Rebuilding image to capture version information (host reports {}).",
                agent, host
            );
            force_rebuild = true;
        }
        (None, None) => {
            println!(
                "Unable to determine {} version on host or sandbox image. Rebuilding image to capture version information.",
                agent
            );
            force_rebuild = true;
        }
        _ => {}
    }

    Ok((host_version, image_version, force_rebuild))
}

fn build_docker_image(current_user: &str, force_rebuild: bool) -> Result<HashMap<String, String>> {
    // Determine host UID/GID so the container user matches host permissions
    let uid_output = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to get host UID")?;
    let gid_output = Command::new("id")
        .arg("-g")
        .output()
        .context("Failed to get host GID")?;

    let uid: u32 = String::from_utf8_lossy(&uid_output.stdout)
        .trim()
        .parse()
        .context("Invalid UID")?;
    let gid: u32 = String::from_utf8_lossy(&gid_output.stdout)
        .trim()
        .parse()
        .context("Invalid GID")?;

    let dockerfile_content = create_dockerfile_content(current_user, uid, gid);
    let temp_dir = std::env::temp_dir();
    let dockerfile_path = temp_dir.join("Dockerfile.agentsandbox");
    fs::write(&dockerfile_path, dockerfile_content).context("Failed to write Dockerfile")?;

    println!(
        "Building Docker image{}...",
        if force_rebuild {
            " (refreshing agent versions)"
        } else {
            ""
        }
    );
    let mut build_command = Command::new("docker");
    build_command.arg("build");
    build_command.arg("-t");
    build_command.arg("agentsandbox-image");
    if force_rebuild {
        // Use build arg to invalidate only agent layers, keeping base layers cached
        let cache_bust = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        build_command.arg("--build-arg");
        build_command.arg(format!("AGENT_CACHE_BUST={}", cache_bust));
    }
    build_command.arg("-f");
    build_command.arg(dockerfile_path.to_str().unwrap());
    build_command.arg(".");
    let build_output = build_command
        .current_dir(&temp_dir)
        .output()
        .context("Failed to build Docker image")?;

    if !build_output.status.success() {
        anyhow::bail!(
            "Docker build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );
    }

    match capture_agent_versions_from_image() {
        Ok(versions) => {
            if let Err(err) = save_image_agent_versions(&versions) {
                println!(
                    "Warning: failed to cache sandbox agent version information: {}",
                    err
                );
            }
            Ok(versions)
        }
        Err(err) => {
            println!("Warning: unable to capture sandbox agent versions: {}", err);
            Ok(HashMap::new())
        }
    }
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

    // Mount clipboard directory (read-only)
    if let Ok(clipboard_dir) = ensure_clipboard_dir() {
        docker_run.args([
            "-v",
            &format!("{}:/workspace/.clipboard:ro", clipboard_dir.display()),
        ]);
        println!(
            "Mounting clipboard directory: {} -> /workspace/.clipboard",
            clipboard_dir.display()
        );
    } else {
        println!("Warning: Failed to setup clipboard directory, clipboard sharing will not be available");
    }

    docker_run.args(["agentsandbox-image", "/bin/bash"]);

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
    let (host_version, image_version_before, force_rebuild) = evaluate_agent_version_status(agent)?;
    let image_versions = build_docker_image(&current_user, force_rebuild)?;
    let mut image_version = image_versions.get(agent.command()).cloned();
    if image_version.is_none() {
        image_version = image_version_before;
    }

    if let (Some(host), Some(image)) = (&host_version, image_version.as_ref()) {
        if !versions_match(host, image) {
            println!(
                "Warning: {} version mismatch persists (host: {}, sandbox image: {}).",
                agent, host, image
            );
            println!(
                "Please update {} on your host to {} (or reinstall the sandbox image) to keep environments aligned.",
                agent, image
            );
        } else if force_rebuild {
            println!(
                "Sandbox image {} version is now {} (matches host).",
                agent, image
            );
        }
    } else if host_version.is_some() && image_version.is_none() {
        println!(
            "Warning: unable to determine sandbox image version for {} after rebuild.",
            agent
        );
    }

    let languages = detect_project_languages(current_dir);
    let (mut docker_run, _env_file_overlays) = build_run_command(
        container_name,
        current_dir,
        additional_dir,
        agent,
        &current_user,
        &languages,
    )?;
    println!("Docker run command: {:?}", docker_run);
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
    // Persist the initial agent run command so we can reuse it on attach/continue
    let initial_cmd = build_agent_command(current_dir, agent, false, skip_permission_flag);
    let _ = save_container_run_command(container_name, &initial_cmd);
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
        println!("Container {} is running", container_name);
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
        "cd '{}' && export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\" && if [ -f \"$HOME/.cargo/env\" ]; then . \"$HOME/.cargo/env\"; fi && {}",
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

    // Try to use the originally saved agent command if available when not in shell mode
    let mut stored_cmd: Option<String> = None;
    if !shell {
        if let Ok(cmd) = load_container_run_command(container_name) {
            stored_cmd = cmd;
        }
    }
    // Ensure the directory structure exists only when we will cd into the current_dir
    if shell || stored_cmd.is_none() {
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
    }

    let command = if shell {
        let path_str = current_dir.display().to_string();
        let escaped = path_str.replace('\'', "'\\''");
        format!(
            "cd '{}' && (source ~/.cargo/env 2>/dev/null || true); (source ~/.bashrc 2>/dev/null || true); exec /bin/bash",
            escaped
        )
    } else if let Some(mut cmd) = stored_cmd {
        if agent_continue && !cmd.contains(" --continue") {
            cmd.push_str(" --continue");
        }
        cmd
    } else {
        build_agent_command(current_dir, agent, agent_continue, skip_permission_flag)
    };

    let should_log_session = allocate_tty;
    let script_available = if should_log_session {
        Command::new("docker")
            .args([
                "exec",
                container_name,
                "sh",
                "-c",
                "command -v script >/dev/null 2>&1",
            ])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    } else {
        false
    };

    let mut session_logging = if script_available {
        match prepare_session_log(container_name, current_dir) {
            Ok(paths) => Some(paths),
            Err(err) => {
                println!(
                    "Warning: failed to prepare session logging directory: {}",
                    err
                );
                None
            }
        }
    } else {
        if should_log_session {
            println!(
                "Warning: 'script' command not available in container; session logging disabled."
            );
        }
        None
    };

    let attach_status = run_docker_exec_with_logging(
        container_name,
        allocate_tty,
        &command,
        session_logging.as_ref(),
    )?;

    if let Some((host_log_path, container_log_path)) = session_logging.take() {
        let log_output = Command::new("docker")
            .args(["exec", container_name, "cat", &container_log_path])
            .output();

        match log_output {
            Ok(output) if output.status.success() => {
                if let Err(err) = fs::write(&host_log_path, output.stdout) {
                    println!(
                        "Warning: failed to write session log to {}: {}",
                        host_log_path.display(),
                        err
                    );
                } else {
                    println!("Session log saved to {}", host_log_path.display());
                }
            }
            Ok(output) => {
                if !output.stderr.is_empty() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    println!(
                        "Warning: failed to capture session log from container: {}",
                        err.trim()
                    );
                } else {
                    println!("Warning: failed to capture session log from container");
                }
            }
            Err(err) => {
                println!(
                    "Warning: failed to read session log from container: {}",
                    err
                );
            }
        }

        let _ = Command::new("docker")
            .args(["exec", container_name, "rm", "-f", &container_log_path])
            .status();
    }

    if !attach_status.success() {
        if shell {
            println!(
                "You can manually attach with: docker exec -it {} /bin/bash",
                container_name
            );
        } else {
            println!("Failed to start {} automatically.", agent);
            println!(
                "You can manually attach with: docker exec -it {} /bin/bash",
                container_name
            );
        }
    }

    Ok(())
}

fn run_docker_exec_with_logging(
    container_name: &str,
    allocate_tty: bool,
    command: &str,
    session_logging: Option<&(PathBuf, String)>,
) -> Result<ExitStatus> {
    let mut args: Vec<String> = vec!["exec".to_string()];
    if allocate_tty {
        args.push("-it".to_string());
    } else {
        args.push("-i".to_string());
    }
    args.push(container_name.to_string());

    if let Some((_, container_log_path)) = session_logging {
        // Use util-linux 'script' with -c to run the command and log output.
        // Correct ordering per util-linux: options, -c <command>, then [file].
        args.push("script".to_string());
        args.push("-q".to_string());
        args.push("-f".to_string());
        args.push("-c".to_string());
        // Wrap the provided command in bash -lc "<command>"
        let mut quoted = String::from(command);
        quoted = quoted.replace("'", "'\\''");
        let bash_c = format!("/bin/bash -lc '{}'", quoted);
        args.push(bash_c);
        // file argument last
        args.push(container_log_path.clone());
    } else {
        args.push("/bin/bash".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());
    }

    let status = Command::new("docker")
        .args(&args)
        .status()
        .context("Failed to attach to container")?;

    // If logging was requested but 'script' failed (non-zero), retry once without logging
    if !status.success() && session_logging.is_some() {
        let mut args_no_log: Vec<String> = vec!["exec".to_string()];
        if allocate_tty {
            args_no_log.push("-it".to_string());
        } else {
            args_no_log.push("-i".to_string());
        }
        args_no_log.push(container_name.to_string());
        args_no_log.push("/bin/bash".to_string());
        args_no_log.push("-c".to_string());
        args_no_log.push(command.to_string());

        let retry_status = Command::new("docker")
            .args(&args_no_log)
            .status()
            .context("Failed to attach to container (retry without logging)")?;
        return Ok(retry_status);
    }
    Ok(status)
}

fn create_dockerfile_content(user: &str, uid: u32, gid: u32) -> String {
    format!(
        r#"FROM ubuntu:24.04

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Update and install required packages
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
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

# Install Rust and Cargo (root)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    /root/.cargo/bin/rustup component add rustfmt clippy && \
    echo 'source ~/.cargo/env' >> /root/.bashrc

# Create user with host UID/GID to avoid permissions issues on mounted volumes
RUN set -eux; \
    existing_grp_by_gid="$(getent group {gid} | cut -d: -f1 || true)"; \
    existing_usr_by_uid="$(getent passwd {uid} | cut -d: -f1 || true)"; \
    # Ensure group named {user} exists with desired GID
    if [ -n "$existing_grp_by_gid" ] && [ "$existing_grp_by_gid" != "{user}" ]; then \
        groupmod -n {user} "$existing_grp_by_gid"; \
    elif ! getent group {user} >/dev/null; then \
        groupadd -g {gid} {user}; \
    fi; \
    # Ensure user named {user} exists with desired UID/GID and home
    if id -u {user} >/dev/null 2>&1; then \
        usermod -u {uid} -g {user} -s /bin/bash {user}; \
    elif [ -n "$existing_usr_by_uid" ] && [ "$existing_usr_by_uid" != "{user}" ]; then \
        usermod -l {user} "$existing_usr_by_uid"; \
        usermod -d /home/{user} -m {user}; \
        usermod -g {user} {user}; \
    else \
        useradd -m -u {uid} -g {user} -s /bin/bash {user}; \
    fi; \
    echo "{user} ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers
ENV HOME=/home/{user}
USER root

# Cache-busting arg: change this to invalidate only agent installation layers
# All layers above remain cached (Ubuntu, Node, Go, Rust, user setup)
ARG AGENT_CACHE_BUST=default
RUN echo "Agent cache bust: ${{AGENT_CACHE_BUST}}"

# Install Claude Code
RUN npm install -g @anthropic-ai/claude-code
RUN npm install -g @google/gemini-cli
RUN npm install -g @openai/codex
RUN npm install -g @qwen-code/qwen-code@latest

# Install Cursor CLI
RUN curl https://cursor.com/install -fsS | bash

# Prepare home directory and user-local bin
RUN mkdir -p /home/{user}/.local/bin && chown -R {user}:{user} /home/{user}

# Switch to user
USER {user}
WORKDIR /home/{user}

# Ensure rustup/cargo and other tools are on PATH (prefer user toolchains)
ENV PATH="/usr/local/go/bin:/home/{user}/.cargo/bin:/home/{user}/.local/bin:$PATH"

# Install Rust for the user and ensure cargo is available
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    ~/.cargo/bin/rustup component add rustfmt clippy && \
    echo 'source ~/.cargo/env' >> ~/.bashrc

# Install uv for Python tooling
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

# Add Go, Rust, Cargo, and uv to PATH
RUN echo 'export PATH="/usr/local/go/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"' >> ~/.bashrc && \
    echo 'source ~/.cargo/env' >> ~/.bashrc

# Install clipboard helper utility
USER root
RUN cat > /usr/local/bin/clipboard << 'CLIPBOARD_EOF'
#!/bin/bash
CLIPBOARD_DIR="/workspace/.clipboard"
get_latest() {{
    if [ ! -d "$CLIPBOARD_DIR" ]; then
        echo "Error: Clipboard directory not found" >&2
        return 1
    fi
    if [ -L "$CLIPBOARD_DIR/latest" ]; then
        echo "$CLIPBOARD_DIR/latest"
        return 0
    fi
    local latest=$(find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" -o -name "clipboard-*.jpeg" \) -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -n 1 | cut -d' ' -f2-)
    if [ -z "$latest" ]; then
        echo "No clipboard images found" >&2
        return 1
    fi
    echo "$latest"
}}
case "${{1:-latest}}" in
    latest) get_latest ;;
    list) find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" \) 2>/dev/null | sort -r ;;
    *) echo "Usage: clipboard [latest|list]" >&2; exit 1 ;;
esac
CLIPBOARD_EOF
RUN chmod +x /usr/local/bin/clipboard
USER {user}

# Set working directory to home
WORKDIR /home/{user}

# Keep container running
CMD ["/bin/bash"]
"#,
        user = user,
        uid = uid,
        gid = gid
    )
}
