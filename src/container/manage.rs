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
        .filter(|n| n.starts_with("agent-") && n.contains(&dir_marker))
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
        .filter(|n| n.starts_with("agent-") && n.contains(&dir_marker))
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
    for name in names.lines().filter(|n| n.starts_with("agent-")) {
        let project = extract_project_name(name);
        let path = get_container_directory(name).ok().flatten();
        containers.push((project, name.to_string(), path));
    }
    Ok(containers)
}

fn extract_project_name(name: &str) -> String {
    if !name.starts_with("agent-") {
        return "unknown".to_string();
    }

    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() < 4 {
        return "unknown".to_string();
    }

    // Check if last part is a timestamp (10 digits)
    let timestamp_idx = if let Some(last_part) = parts.last() {
        if last_part.len() == 10 && last_part.chars().all(|c| c.is_ascii_digit()) {
            Some(parts.len() - 1)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(ts_idx) = timestamp_idx {
        // Known agent names
        let agents = ["claude", "gemini", "codex", "qwen", "cursor"];

        // Find the agent (should be at index 1)
        if parts.len() > 1 {
            let potential_agent = parts[1];
            if agents.contains(&potential_agent) {
                // Agent is at index 1, timestamp at ts_idx
                // Everything between index 2 and ts_idx-1 (inclusive) is project + branch
                // Assume the branch is always the part just before timestamp
                if ts_idx >= 3 {
                    // Project is from index 2 to ts_idx-2 (inclusive)
                    let project_parts = &parts[2..ts_idx - 1];
                    if !project_parts.is_empty() {
                        return project_parts.join("-");
                    }
                }
            }
        }
    }

    "unknown".to_string()
}

fn get_container_directory(name: &str) -> Result<Option<String>> {
    // Get all mounts where source equals destination and is read-write
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

    // Look for a project directory that doesn't start with a dot (config/hidden dirs)
    // and prefer directories that don't contain common config path patterns
    let mut candidates: Vec<String> = Vec::new();

    for line in paths.lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }

        // Skip obvious config directories
        if path.contains("/.claude") || path.contains("/.serena") {
            continue;
        }

        // Get the last component of the path to check if it's a hidden directory
        if let Some(last_component) = std::path::Path::new(path).file_name() {
            if let Some(name_str) = last_component.to_str() {
                if name_str.starts_with('.') {
                    // This is a hidden directory, likely a config dir, but keep as backup
                    candidates.push(path.to_string());
                    continue;
                }
            }
        }

        // This looks like a regular project directory
        return Ok(Some(path.to_string()));
    }

    // If no non-hidden directory found, return the first candidate
    Ok(candidates.into_iter().next())
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
    for name in names.lines().filter(|n| n.starts_with("agent-")) {
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
