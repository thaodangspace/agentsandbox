use anyhow::{Context, Result};
use chrono::Utc;
use std::path::Path;
use std::process::Command;

use super::naming::sanitize;

pub fn cleanup_containers(current_dir: &Path) -> Result<()> {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize)
        .unwrap_or_else(|| "unknown".to_string());
    let dir_marker = format!("-{dir_name}-");

    let list_output = Command::new("docker")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .context("Failed to list Docker containers")?;

    if !list_output.status.success() {
        anyhow::bail!(
            "Failed to list containers: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );
    }

    let names = String::from_utf8_lossy(&list_output.stdout);
    for name in names
        .lines()
        .filter(|n| n.starts_with("csb-") && n.contains(&dir_marker))
    {
        println!("Removing container {name}");
        let rm_output = Command::new("docker")
            .args(["rm", "-f", name])
            .output()
            .context("Failed to remove container")?;

        if !rm_output.status.success() {
            anyhow::bail!(
                "Failed to remove container {}: {}",
                name,
                String::from_utf8_lossy(&rm_output.stderr)
            );
        }
    }

    Ok(())
}

pub fn list_containers(current_dir: &Path) -> Result<Vec<String>> {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize)
        .unwrap_or_else(|| "unknown".to_string());
    let dir_marker = format!("-{dir_name}-");

    let list_output = Command::new("docker")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .context("Failed to list Docker containers")?;

    if !list_output.status.success() {
        anyhow::bail!(
            "Failed to list containers: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );
    }

    let names = String::from_utf8_lossy(&list_output.stdout);
    let containers = names
        .lines()
        .filter(|n| n.starts_with("csb-") && n.contains(&dir_marker))
        .map(|s| s.to_string())
        .collect();
    Ok(containers)
}

pub fn list_all_containers() -> Result<Vec<(String, String, Option<String>)>> {
    let list_output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .output()
        .context("Failed to list Docker containers")?;

    if !list_output.status.success() {
        anyhow::bail!(
            "Failed to list containers: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );
    }

    let names = String::from_utf8_lossy(&list_output.stdout);
    let mut containers = Vec::new();
    for name in names.lines().filter(|n| n.starts_with("csb-")) {
        let project = extract_project_name(name);
        let path = get_container_directory(name).ok().flatten();
        containers.push((project, name.to_string(), path));
    }
    Ok(containers)
}

fn extract_project_name(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() >= 3 {
        parts[2].to_string()
    } else {
        "unknown".to_string()
    }
}

fn get_container_directory(name: &str) -> Result<Option<String>> {
    // First try to get the main project mount (where source equals destination and is read-write)
    let output = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{range .Mounts}}{{if and .RW (eq .Source .Destination)}}{{.Source}}{{\"\\n\"}}{{end}}{{end}}",
            name,
        ])
        .output()
        .context("Failed to inspect container")?;
    if !output.status.success() {
        return Ok(None);
    }
    let paths = String::from_utf8_lossy(&output.stdout);

    // Filter out config directories and get the first valid project path
    for line in paths.lines() {
        let path = line.trim();
        if !path.is_empty() && !path.contains("/.claude") && !path.contains("/.serena") {
            return Ok(Some(path.to_string()));
        }
    }
    Ok(None)
}

pub fn auto_remove_old_containers(minutes: u64) -> Result<()> {
    if minutes == 0 {
        return Ok(());
    }

    let cutoff = Utc::now() - chrono::Duration::minutes(minutes as i64);

    let list_output = Command::new("docker")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .context("Failed to list Docker containers")?;

    if !list_output.status.success() {
        anyhow::bail!(
            "Failed to list containers: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );
    }

    let names = String::from_utf8_lossy(&list_output.stdout);
    for name in names.lines().filter(|n| n.starts_with("csb-")) {
        let inspect_output = Command::new("docker")
            .args(["inspect", "-f", "{{.Created}}", name])
            .output()
            .context("Failed to inspect container")?;
        if !inspect_output.status.success() {
            continue;
        }
        let created_str = String::from_utf8_lossy(&inspect_output.stdout)
            .trim()
            .to_string();
        let created = match chrono::DateTime::parse_from_rfc3339(&created_str) {
            Ok(c) => c.with_timezone(&Utc),
            Err(_) => continue,
        };
        if created > cutoff {
            continue;
        }

        let logs_output = Command::new("docker")
            .args(["logs", name])
            .output()
            .context("Failed to check container logs")?;
        if !logs_output.status.success() {
            continue;
        }
        if logs_output.stdout.is_empty() && logs_output.stderr.is_empty() {
            println!("Auto removing unused container {name}");
            let rm_output = Command::new("docker")
                .args(["rm", "-f", name])
                .output()
                .context("Failed to remove container")?;
            if !rm_output.status.success() {
                anyhow::bail!(
                    "Failed to remove container {}: {}",
                    name,
                    String::from_utf8_lossy(&rm_output.stderr)
                );
            }
        }
    }
    Ok(())
}

pub fn check_docker_availability() -> Result<()> {
    let output = Command::new("docker").arg("--version").output().context(
        "Failed to check Docker availability. Make sure Docker is installed and running.",
    )?;

    if !output.status.success() {
        anyhow::bail!("Docker is not available or not running");
    }

    Ok(())
}

pub fn is_container_running(container_name: &str) -> Result<bool> {
    let output = Command::new("docker")
        .args(&["inspect", "-f", "{{.State.Running}}", container_name])
        .output()
        .context("Failed to check container status")?;

    if !output.status.success() {
        return Ok(false);
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let status = output_str.trim();
    Ok(status == "true")
}

pub fn container_exists(container_name: &str) -> Result<bool> {
    let output = Command::new("docker")
        .args(&["inspect", container_name])
        .output()
        .context("Failed to check if container exists")?;

    Ok(output.status.success())
}
