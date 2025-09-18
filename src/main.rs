#![allow(
    clippy::uninlined_format_args,
    clippy::needless_borrows_for_generic_args,
    clippy::manual_contains
)]

mod cli;
mod config;
mod container;
mod language;
mod settings;
mod state;
mod worktree;

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

use cli::{Agent, Cli, Commands};
use container::{
    auto_remove_old_containers, check_docker_availability, cleanup_containers, create_container,
    find_existing_container, generate_container_name, list_all_containers, list_containers,
    resume_container,
};
use settings::load_settings;
use state::{clear_last_container, load_last_container, save_last_container};
use tabled::settings::Style;
use tabled::{Table, Tabled};
use worktree::create_worktree;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();

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
                resume_container(
                    &container_name,
                    &agent,
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
            println!("No running Agent Sandbox containers found.");
            return Ok(());
        }
        #[derive(Tabled)]
        struct Row {
            #[tabled(rename = "No.")]
            no: usize,
            #[tabled(rename = "Project")]
            project: String,
            #[tabled(rename = "Container")]
            container: String,
            #[tabled(rename = "Directory")]
            directory: String,
        }
        let rows: Vec<Row> = containers
            .iter()
            .enumerate()
            .map(|(i, (project, name, path))| Row {
                no: i + 1,
                project: project.clone(),
                container: name.clone(),
                directory: path.as_deref().unwrap_or("").to_string(),
            })
            .collect();
        println!("{}", Table::new(rows).with(Style::rounded()));
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
                    resume_container(
                        name,
                        &agent,
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
                "No Agent Sandbox containers found for directory {}",
                current_dir.display()
            );
            let global = list_all_containers()?;
            if global.is_empty() {
                println!("No running Agent Sandbox containers found.");
            } else {
                println!("\nCurrently running containers:");
                #[derive(Tabled)]
                struct GlobalRow {
                    #[tabled(rename = "Project")]
                    project: String,
                    #[tabled(rename = "Container")]
                    container: String,
                }
                let rows: Vec<GlobalRow> = global
                    .into_iter()
                    .map(|(project, name, _)| GlobalRow {
                        project,
                        container: name,
                    })
                    .collect();
                println!("{}", Table::new(rows).with(Style::rounded()));
            }
            return Ok(());
        }
        #[derive(Tabled)]
        struct Row {
            #[tabled(rename = "No.")]
            no: usize,
            #[tabled(rename = "Container")]
            container: String,
        }
        let rows: Vec<Row> = containers
            .iter()
            .enumerate()
            .map(|(i, name)| Row {
                no: i + 1,
                container: name.clone(),
            })
            .collect();
        println!("{}", Table::new(rows).with(Style::rounded()));

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
                resume_container(
                    selected,
                    &agent,
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
            let agent = Agent::from_container_name(latest).unwrap_or_else(|| cli.agent.clone());
            resume_container(
                latest,
                &agent,
                false,
                skip_permission_flag.as_deref(),
                cli.shell,
                true,
            )
            .await?;
            return Ok(());
        }
    }

    // Check if there's already an existing container for this directory/agent/branch combination
    if let Some(existing_container) = find_existing_container(&current_dir, &cli.agent)? {
        println!("Found existing container: {}", existing_container);
        println!("Attaching to existing container instead of creating a new one...");

        let agent = Agent::from_container_name(&existing_container).unwrap_or_else(|| cli.agent.clone());
        resume_container(
            &existing_container,
            &agent,
            false,
            skip_permission_flag.as_deref(),
            cli.shell,
            true,
        )
        .await?;
        save_last_container(&existing_container)?;
        return Ok(());
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
        "Starting {} Agent Sandbox container: {container_name}",
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
