#![allow(
    clippy::uninlined_format_args,
    clippy::needless_borrows_for_generic_args,
    clippy::manual_contains
)]

mod cli;
mod clipboard;
mod config;
mod container;
mod language;
mod log_parser;
mod log_viewer;
mod settings;
mod state;
mod worktree;

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};

use cli::{Agent, Cli, Commands, LogAction};
use clipboard::{
    clear_watcher_pid, clipboard_feature_enabled, ensure_clipboard_dir, is_process_running,
    load_watcher_pid, save_watcher_pid,
};
use container::{
    auto_remove_old_containers, check_docker_availability, cleanup_containers, create_container,
    find_existing_container, generate_container_name, list_all_containers, list_containers,
    resume_container,
};
use settings::load_settings;
use state::{
    cleanup_old_logs, clear_last_container, list_containers_with_logs, list_session_logs,
    load_last_container, save_last_container,
};
use tabled::settings::Style;
use tabled::{Table, Tabled};
use worktree::create_worktree;

// Embed the clipboard watcher script at compile time
const CLIPBOARD_WATCHER_SCRIPT: &str = include_str!("../scripts/clipboard-watcher.sh");

/// Start the clipboard watcher if it's not already running
fn ensure_clipboard_watcher_running() -> Result<()> {
    // Check if there's an existing watcher PID
    if let Ok(Some(pid)) = load_watcher_pid() {
        if is_process_running(pid) {
            // Watcher is already running
            return Ok(());
        } else {
            // Stale PID file, clean it up
            let _ = clear_watcher_pid();
        }
    }

    // Ensure clipboard directory exists
    ensure_clipboard_dir()?;

    // Write the embedded script to the state directory
    let home_dir = home::home_dir().context("Failed to get home directory")?;
    let state_dir = home_dir.join(".config").join("agentsandbox");
    fs::create_dir_all(&state_dir)?;

    let script_path = state_dir.join("clipboard-watcher.sh");
    fs::write(&script_path, CLIPBOARD_WATCHER_SCRIPT).with_context(|| {
        format!(
            "Failed to write clipboard watcher script to {:?}",
            script_path
        )
    })?;

    // Make the script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }

    // Start the clipboard watcher as a background process
    println!("Starting clipboard watcher...");
    let child = std::process::Command::new(&script_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start clipboard watcher from {:?}", script_path))?;

    let pid = child.id();
    save_watcher_pid(pid)?;
    println!("Clipboard watcher started (PID: {})", pid);
    println!(
        "Images copied to clipboard will be available at /workspace/.clipboard/ in the container"
    );

    Ok(())
}

/// Handle the logs subcommand
fn handle_logs_command(action: &LogAction, current_dir: &std::path::Path) -> Result<()> {
    match action {
        LogAction::List { container } => {
            let containers = if let Some(container_name) = container {
                vec![container_name.clone()]
            } else {
                list_containers_with_logs(current_dir)?
            };

            if containers.is_empty() {
                println!("No session logs found.");
                return Ok(());
            }

            for container_name in containers {
                println!("\nContainer: {}", container_name);
                match list_session_logs(&container_name, current_dir) {
                    Ok(logs) => {
                        if logs.is_empty() {
                            println!("  No logs found");
                        } else {
                            for log in logs {
                                println!("  {}", log.display());
                            }
                        }
                    }
                    Err(e) => {
                        println!("  Error listing logs: {}", e);
                    }
                }
            }
        }
        LogAction::View {
            log_file,
            output,
            open,
        } => {
            // Read JSONL log
            let events =
                log_parser::parse_raw_log(log_file).context("Failed to parse log file")?;

            // Determine output path
            let html_path = output
                .clone()
                .unwrap_or_else(|| log_file.with_extension("html"));

            // Generate HTML
            let title = log_file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Session Log");
            log_viewer::write_html(&events, &html_path, title)
                .context("Failed to generate HTML")?;

            println!("HTML log generated: {}", html_path.display());

            // Open in browser if requested
            if *open {
                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("xdg-open")
                        .arg(&html_path)
                        .spawn()
                        .context("Failed to open browser")?;
                }
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg(&html_path)
                        .spawn()
                        .context("Failed to open browser")?;
                }
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("cmd")
                        .args(["/c", "start", html_path.to_str().unwrap()])
                        .spawn()
                        .context("Failed to open browser")?;
                }
                println!("Opened in browser");
            }
        }
        LogAction::Clean { days, container } => {
            let containers = if let Some(container_name) = container {
                vec![container_name.clone()]
            } else {
                list_containers_with_logs(current_dir)?
            };

            if containers.is_empty() {
                println!("No containers with logs found.");
                return Ok(());
            }

            let mut total_deleted = 0;
            for container_name in containers {
                match cleanup_old_logs(&container_name, current_dir, *days) {
                    Ok(deleted) => {
                        if deleted > 0 {
                            println!(
                                "Deleted {} old log files from container {}",
                                deleted, container_name
                            );
                            total_deleted += deleted;
                        }
                    }
                    Err(e) => {
                        println!(
                            "Warning: Failed to cleanup logs for {}: {}",
                            container_name, e
                        );
                    }
                }
            }

            if total_deleted == 0 {
                println!("No logs older than {} days found.", days);
            } else {
                println!("Total deleted: {} files", total_deleted);
            }
        }
    }

    Ok(())
}

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

    let clipboard_enabled = clipboard_feature_enabled();

    // Start clipboard watcher for image sharing between host and container when available
    if clipboard_enabled && !cli.no_clipboard {
        if let Err(e) = ensure_clipboard_watcher_running() {
            println!("Warning: Failed to start clipboard watcher: {}", e);
        }
    } else if !clipboard_enabled && !cli.no_clipboard {
        println!("Clipboard sharing is temporarily disabled due to known issues.");
    }

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

    if let Some(Commands::Logs { action }) = cli.command.as_ref() {
        return handle_logs_command(action, &current_dir);
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
                // Prompt for attach mode
                print!("Choose attach mode:\n  1) Attach with agent\n  2) Attach to shell only\nEnter choice: ");
                io::stdout().flush().ok();
                let mut mode_input = String::new();
                io::stdin().read_line(&mut mode_input)?;
                let mode_input = mode_input.trim();

                let shell_mode = match mode_input {
                    "1" => false,
                    "2" => true,
                    _ => {
                        println!("Invalid choice");
                        return Ok(());
                    }
                };

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
                        shell_mode,
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
                // Prompt for attach mode
                print!("Choose attach mode:\n  1) Attach with agent\n  2) Attach to shell only\nEnter choice: ");
                io::stdout().flush().ok();
                let mut mode_input = String::new();
                io::stdin().read_line(&mut mode_input)?;
                let mode_input = mode_input.trim();

                let shell_mode = match mode_input {
                    "1" => false,
                    "2" => true,
                    _ => {
                        println!("Invalid choice");
                        return Ok(());
                    }
                };

                let selected = &containers[num - 1];
                let agent =
                    Agent::from_container_name(selected).unwrap_or_else(|| cli.agent.clone());
                resume_container(
                    selected,
                    &agent,
                    false,
                    skip_permission_flag.as_deref(),
                    shell_mode,
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

        let agent =
            Agent::from_container_name(&existing_container).unwrap_or_else(|| cli.agent.clone());
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
