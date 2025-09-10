#![allow(
    clippy::uninlined_format_args,
    clippy::needless_borrows_for_generic_args,
    clippy::manual_contains
)]

mod cli;
mod config;
mod container;
mod language;
mod server;
mod settings;
mod state;
mod worktree;

use anyhow::{Context, Result};
use base64::Engine as _;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use cli::{Agent, Cli, Commands};
use container::{
    auto_remove_old_containers, check_docker_availability, cleanup_containers, create_container,
    generate_container_name, list_all_containers, list_containers, resume_container,
};
use settings::{load_settings, Settings};
use state::{clear_last_container, load_last_container, save_last_container};
use worktree::create_worktree;

fn skip_permission_flag_for(settings: &Settings, agent: &Agent) -> Option<String> {
    settings
        .skip_permission_flags
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(agent.command()))
        .map(|(_, flag)| flag.clone())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Serve { daemon } => {
                check_docker_availability()?;
                if *daemon {
                    let exe = env::current_exe()?;
                    Command::new(exe)
                        .arg("serve")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .stdin(Stdio::null())
                        .spawn()
                        .context("failed to start daemonized server")?;
                } else {
                    server::serve().await?;
                }
                return Ok(());
            }
            Commands::Stop => {
                server::stop().await?;
                return Ok(());
            }
            Commands::Restart { daemon } => {
                let _ = server::stop().await;
                check_docker_availability()?;
                if *daemon {
                    let exe = env::current_exe()?;
                    Command::new(exe)
                        .arg("serve")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .stdin(Stdio::null())
                        .spawn()
                        .context("failed to start daemonized server")?;
                } else {
                    server::serve().await?;
                }
                return Ok(());
            }
            _ => {}
        }
    }

    let mut current_dir = env::current_dir().context("Failed to get current directory")?;
    if let Some(branch) = &cli.worktree {
        current_dir = create_worktree(&current_dir, branch)
            .with_context(|| format!("Failed to create worktree for branch {}", branch))?;
    }
    let settings = load_settings().unwrap_or_default();
    let web_host = settings.web_host.as_deref().unwrap_or("localhost");

    check_docker_availability()?;
    auto_remove_old_containers(settings.auto_remove_minutes.unwrap_or(60))?;

    // Determine whether to use web flow
    let use_web = cli.web || settings.web.unwrap_or(false);

    if let Some(Commands::Cleanup) = cli.command.as_ref() {
        cleanup_containers(&current_dir)?;
        clear_last_container()?;
        println!(
            "Removed all Agent Sandbox containers for directory {}",
            current_dir.display()
        );
        return Ok(());
    }

    if cli.continue_ {
        match load_last_container()? {
            Some(container_name) => {
                let agent = Agent::from_container_name(&container_name)
                    .unwrap_or_else(|| cli.agent.clone());
                let skip_permission_flag = skip_permission_flag_for(&settings, &agent);
                resume_container(
                    &container_name,
                    &agent,
                    true,
                    skip_permission_flag.as_deref(),
                    cli.shell,
                    !use_web,
                )
                .await?;
                if use_web {
                    maybe_open_web(
                        &container_name,
                        &agent,
                        &current_dir,
                        true,
                        skip_permission_flag.as_deref(),
                        web_host,
                    )
                    .await?;
                }
                return Ok(());
            }
            None => {
                anyhow::bail!("No previous container found. Run without --continue to create a new container.");
            }
        }
    }

    if let Some(Commands::Ps) = cli.command.as_ref() {
        let containers = list_all_containers()?;
        if containers.is_empty() {
            println!("No running Agent Sandbox containers found.");
            return Ok(());
        }
        println!("{:<4}{:<20}{:<20}Directory", "No.", "Project", "Container");
        for (i, (project, name, path)) in containers.iter().enumerate() {
            println!(
                "{:<4}{:<20}{:<20}{}",
                i + 1,
                project,
                name,
                path.as_deref().unwrap_or("")
            );
        }
        print!(
            "Select a container to attach (number), or type 'cd <number>' to open its directory: "
        );
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            return Ok(());
        }
        if let Some(rest) = input.strip_prefix("cd ") {
            match rest.parse::<usize>() {
                Ok(num) if num >= 1 && num <= containers.len() => {
                    if let Some(path) = &containers[num - 1].2 {
                        let escaped = path.replace('\'', "'\\''");
                        Command::new("bash")
                            .args(["-c", &format!("cd '{}' && exec bash", escaped)])
                            .status()
                            .ok();
                    } else {
                        println!("Path not available for selected container");
                    }
                }
                _ => println!("Invalid selection"),
            }
            return Ok(());
        }
        match input.parse::<usize>() {
            Ok(num) if num >= 1 && num <= containers.len() => {
                if let Some(path) = &containers[num - 1].2 {
                    env::set_current_dir(path)
                        .with_context(|| format!("Failed to change directory to {}", path))?;
                    let (_, name, _) = &containers[num - 1];
                    let agent =
                        Agent::from_container_name(name).unwrap_or_else(|| cli.agent.clone());
                    let skip_permission_flag = skip_permission_flag_for(&settings, &agent);
                    resume_container(
                        name,
                        &agent,
                        false,
                        skip_permission_flag.as_deref(),
                        cli.shell,
                        !use_web,
                    )
                    .await?;
                    if use_web {
                        maybe_open_web(
                            name,
                            &agent,
                            &current_dir,
                            false,
                            skip_permission_flag.as_deref(),
                            web_host,
                        )
                        .await?;
                    }
                } else {
                    println!("Path not available for selected container");
                }
            }
            _ => println!("Invalid selection"),
        }
        return Ok(());
    }

    if let Some(Commands::Ls) = cli.command.as_ref() {
        let containers = list_containers(&current_dir)?;
        if containers.is_empty() {
            println!(
                "No Agent Sandbox containers found for directory {}",
                current_dir.display()
            );
            let global = list_all_containers()?;
            if global.is_empty() {
                println!("No running Agent Sandbox containers found.");
            } else {
                println!("\nCurrently running containers:");
                println!("{:<20}Container", "Project");
                for (project, name, _) in global {
                    println!("{:<20}{}", project, name);
                }
            }
            return Ok(());
        }

        for (i, name) in containers.iter().enumerate() {
            println!("{}: {}", i + 1, name);
        }

        print!("Select a container to attach (number, or press Enter to cancel): ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            return Ok(());
        }

        match input.parse::<usize>() {
            Ok(num) if num >= 1 && num <= containers.len() => {
                let selected = &containers[num - 1];
                let agent =
                    Agent::from_container_name(selected).unwrap_or_else(|| cli.agent.clone());
                let skip_permission_flag = skip_permission_flag_for(&settings, &agent);
                resume_container(
                    selected,
                    &agent,
                    false,
                    skip_permission_flag.as_deref(),
                    cli.shell,
                    !use_web,
                )
                .await?;
                if use_web {
                    maybe_open_web(
                        selected,
                        &agent,
                        &current_dir,
                        false,
                        skip_permission_flag.as_deref(),
                        web_host,
                    )
                    .await?;
                }
            }
            _ => println!("Invalid selection"),
        }
        return Ok(());
    }

    if cli.worktree.is_some() {
        let containers = list_containers(&current_dir)?;
        if let Some(latest) = containers.first() {
            println!("Attaching to existing container for worktree: {}", latest);
            let agent = Agent::from_container_name(latest).unwrap_or_else(|| cli.agent.clone());
            let skip_permission_flag = skip_permission_flag_for(&settings, &agent);
            resume_container(
                latest,
                &agent,
                false,
                skip_permission_flag.as_deref(),
                cli.shell,
                !use_web,
            )
            .await?;
            if use_web {
                maybe_open_web(
                    latest,
                    &agent,
                    &current_dir,
                    false,
                    skip_permission_flag.as_deref(),
                    web_host,
                )
                .await?;
            }
            return Ok(());
        }
    }

    let additional_dir = match &cli.add_dir {
        Some(dir) => Some(
            fs::canonicalize(dir)
                .with_context(|| format!("Failed to canonicalize path {}", dir.display()))?,
        ),
        None => None,
    };

    let container_name = generate_container_name(&current_dir, &cli.agent);

    let token = container_name.clone();
    println!(
        "Starting {} Agent Sandbox container: {container_name}",
        cli.agent
    );
    println!("Container {container_name} started successfully!");
    println!(
        "Access the terminal at: http://{}:6789/container/{container_name}?token={token}",
        web_host
    );
    println!(
        "To attach to the container manually, run: docker exec -it {container_name} /bin/bash"
    );

    let skip_permission_flag = skip_permission_flag_for(&settings, &cli.agent);
    create_container(
        &container_name,
        &current_dir,
        additional_dir.as_deref(),
        &cli.agent,
        skip_permission_flag.as_deref(),
        cli.shell,
        !use_web,
    )
    .await?;
    save_last_container(&container_name)?;

    if use_web {
        maybe_open_web(
            &container_name,
            &cli.agent,
            &current_dir,
            false,
            skip_permission_flag.as_deref(),
            web_host,
        )
        .await?;
    }

    Ok(())
}

fn build_agent_command_for_web(
    current_dir: &Path,
    agent: &cli::Agent,
    agent_continue: bool,
    skip_permission_flag: Option<&str>,
) -> String {
    // Safely quote project path for bash -c
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

async fn ensure_server_running() -> Result<()> {
    // Try to contact the server briefly; if unavailable, spawn it in the background
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(300))
        .build()?;
    let reachable = client.get("http://127.0.0.1:6789/").send().await.is_ok();
    if !reachable {
        let exe = env::current_exe()?;
        let _child = Command::new(exe)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .context("failed to start server in background")?;
        // Give it a moment to bind
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(())
}

async fn maybe_open_web(
    container_name: &str,
    agent: &cli::Agent,
    current_dir: &Path,
    agent_continue: bool,
    skip_permission_flag: Option<&str>,
    web_host: &str,
) -> Result<()> {
    ensure_server_running().await?;

    let token = container_name;
    let cmd = build_agent_command_for_web(current_dir, agent, agent_continue, skip_permission_flag);
    let run_b64 = base64::engine::general_purpose::STANDARD.encode(cmd.as_bytes());
    // Also pass the desired working directory so the shell starts in project root
    let cwd_b64 = base64::engine::general_purpose::STANDARD
        .encode(current_dir.display().to_string().as_bytes());
    let url = format!(
        "http://{}:6789/container/{}?token={}&run_b64={}&cwd_b64={}",
        web_host, container_name, token, run_b64, cwd_b64
    );

    // Try to open the system browser
    let opener = if cfg!(target_os = "macos") {
        ("open", vec![url.as_str()])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", url.as_str()])
    } else {
        ("xdg-open", vec![url.as_str()])
    };

    let _ = Command::new(opener.0).args(opener.1).spawn();
    println!("Opened web UI: {}", url);
    Ok(())
}
