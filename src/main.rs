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
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use cli::{Cli, Commands};
use container::{
    auto_remove_old_containers, check_docker_availability, cleanup_containers, create_container,
    generate_container_name, list_all_containers, list_containers, resume_container,
};
use settings::load_settings;
use state::{clear_last_container, load_last_container, save_last_container};
use worktree::create_worktree;

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

    check_docker_availability()?;
    auto_remove_old_containers(settings.auto_remove_minutes.unwrap_or(60))?;
    let skip_permission_flag = settings
        .skip_permission_flags
        .iter()
        .find(|(agent, _)| agent.eq_ignore_ascii_case(cli.agent.command()))
        .map(|(_, flag)| flag.to_string());

    if let Some(Commands::Cleanup) = cli.command.as_ref() {
        cleanup_containers(&current_dir)?;
        clear_last_container()?;
        println!(
            "Removed all Code Sandbox containers for directory {}",
            current_dir.display()
        );
        return Ok(());
    }

    if cli.continue_ {
        match load_last_container()? {
            Some(container_name) => {
                resume_container(
                    &container_name,
                    &cli.agent,
                    true,
                    skip_permission_flag.as_deref(),
                    cli.shell,
                    true,
                )
                .await?;
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
            println!("No running Code Sandbox containers found.");
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
                    resume_container(
                        name,
                        &cli.agent,
                        false,
                        skip_permission_flag.as_deref(),
                        cli.shell,
                        true,
                    )
                    .await?;
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
                "No Code Sandbox containers found for directory {}",
                current_dir.display()
            );
            let global = list_all_containers()?;
            if global.is_empty() {
                println!("No running Code Sandbox containers found.");
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
                resume_container(
                    selected,
                    &cli.agent,
                    false,
                    skip_permission_flag.as_deref(),
                    cli.shell,
                    true,
                )
                .await?;
            }
            _ => println!("Invalid selection"),
        }
        return Ok(());
    }

    if cli.worktree.is_some() {
        let containers = list_containers(&current_dir)?;
        if let Some(latest) = containers.first() {
            println!("Attaching to existing container for worktree: {}", latest);
            resume_container(
                latest,
                &cli.agent,
                false,
                skip_permission_flag.as_deref(),
                cli.shell,
                true,
            )
            .await?;
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

    println!(
        "Starting {} Code Sandbox container: {container_name}",
        cli.agent
    );
    println!("Container {container_name} started successfully!");
    println!(
        "To attach to the container manually, run: docker exec -it {container_name} /bin/bash"
    );

    create_container(
        &container_name,
        &current_dir,
        additional_dir.as_deref(),
        &cli.agent,
        skip_permission_flag.as_deref(),
        cli.shell,
        true,
    )
    .await?;
    save_last_container(&container_name)?;

    Ok(())
}
