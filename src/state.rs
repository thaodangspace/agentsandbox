use anyhow::{Context, Result};
use chrono::{Local, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn get_state_file_path() -> Result<PathBuf> {
    let home_dir = home::home_dir().context("Failed to get home directory")?;
    let config_dir = home_dir.join(".config").join("agentsandbox");
    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
    Ok(config_dir.join("last_container"))
}

pub fn save_last_container(container_name: &str) -> Result<()> {
    let state_file = get_state_file_path()?;
    fs::write(&state_file, container_name).context("Failed to save last container name")?;
    Ok(())
}

pub fn load_last_container() -> Result<Option<String>> {
    let state_file = get_state_file_path()?;
    if !state_file.exists() {
        return Ok(None);
    }

    let container_name = fs::read_to_string(&state_file)
        .context("Failed to read last container name")?
        .trim()
        .to_string();

    if container_name.is_empty() {
        return Ok(None);
    }

    Ok(Some(container_name))
}

pub fn clear_last_container() -> Result<()> {
    let state_file = get_state_file_path()?;
    if state_file.exists() {
        fs::remove_file(state_file).context("Failed to remove last container state")?;
    }
    Ok(())
}

fn get_base_config_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().context("Failed to get home directory")?;
    Ok(home_dir.join(".config").join("agentsandbox"))
}

fn get_image_versions_path() -> Result<PathBuf> {
    let base_dir = get_base_config_dir()?;
    fs::create_dir_all(&base_dir).context("Failed to ensure config directory")?;
    Ok(base_dir.join("image_agent_versions.json"))
}

fn get_container_dir(container_name: &str) -> Result<PathBuf> {
    let dir = get_base_config_dir()?
        .join("containers")
        .join(container_name);
    fs::create_dir_all(&dir).context("Failed to create container state directory")?;
    Ok(dir)
}

fn ensure_session_logs_dir(container_name: &str, project_dir: &Path) -> Result<PathBuf> {
    // Primary location: project-local .agentsandbox directory
    let candidate = project_dir
        .join(".agentsandbox")
        .join("session_logs")
        .join(container_name);

    match fs::create_dir_all(&candidate) {
        Ok(()) => Ok(candidate),
        Err(project_err) => {
            // Fallback to config directory if project directory is not writable
            let fallback = get_container_dir(container_name)?.join("logs");
            let candidate_display = candidate.display().to_string();
            let project_err_msg = project_err.to_string();
            fs::create_dir_all(&fallback).with_context(|| {
                format!(
                    "Failed to create session log directory after project directory creation failed at {}: {}",
                    candidate_display, project_err_msg
                )
            })?;
            Ok(fallback)
        }
    }
}

fn get_run_command_path(container_name: &str) -> Result<PathBuf> {
    Ok(get_container_dir(container_name)?.join("run_cmd"))
}

pub fn save_container_run_command(container_name: &str, command: &str) -> Result<()> {
    let path = get_run_command_path(container_name)?;
    fs::write(&path, command).context("Failed to save container run command")?;
    Ok(())
}

pub fn load_container_run_command(container_name: &str) -> Result<Option<String>> {
    let path = get_base_config_dir()?
        .join("containers")
        .join(container_name)
        .join("run_cmd");
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path).context("Failed to read container run command")?;
    let trimmed = contents.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

pub fn prepare_session_log(container_name: &str, project_dir: &Path) -> Result<(PathBuf, String)> {
    let logs_dir = ensure_session_logs_dir(container_name, project_dir)?;
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let host_path = logs_dir.join(format!("session-{}.log", Local::now().format("%Y%m%d")));
    let container_path = format!("/tmp/session-{}-{}.log", container_name, timestamp);

    Ok((host_path, container_path))
}

/// Get paths for session log files (raw, JSONL, HTML)
pub fn get_session_log_paths(raw_log_path: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let jsonl_path = raw_log_path.with_extension("jsonl");
    let html_path = raw_log_path.with_extension("html");
    let raw_dir = raw_log_path.parent().unwrap().join("raw");
    let raw_filename = raw_log_path.file_name().unwrap();
    let raw_path = raw_dir.join(raw_filename);

    (raw_path, jsonl_path, html_path)
}

/// List all session logs for a container (returns JSONL paths)
pub fn list_session_logs(container_name: &str, project_dir: &Path) -> Result<Vec<PathBuf>> {
    let logs_dir = ensure_session_logs_dir(container_name, project_dir)?;
    let mut logs = Vec::new();

    if logs_dir.exists() {
        for entry in fs::read_dir(&logs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                logs.push(path);
            }
        }
    }

    logs.sort_by(|a, b| b.cmp(a)); // Most recent first
    Ok(logs)
}

/// Clean up old session logs based on retention days
pub fn cleanup_old_logs(container_name: &str, project_dir: &Path, retention_days: u64) -> Result<usize> {
    let logs_dir = ensure_session_logs_dir(container_name, project_dir)?;
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let mut deleted_count = 0;

    if !logs_dir.exists() {
        return Ok(0);
    }

    for entry in fs::read_dir(&logs_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip directories (like 'raw')
        if path.is_dir() {
            continue;
        }

        // Check file modification time
        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                let modified_time: chrono::DateTime<Utc> = modified.into();
                if modified_time < cutoff {
                    // Delete the file and its related files
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    match ext {
                        "jsonl" | "html" => {
                            if fs::remove_file(&path).is_ok() {
                                deleted_count += 1;
                            }
                        }
                        "log" => {
                            // Only delete raw logs from the 'raw' subdirectory
                            if path.parent().and_then(|p| p.file_name()) == Some("raw".as_ref()) {
                                if fs::remove_file(&path).is_ok() {
                                    deleted_count += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Also clean up the raw directory
    let raw_dir = logs_dir.join("raw");
    if raw_dir.exists() {
        if let Ok(entries) = fs::read_dir(&raw_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            let modified_time: chrono::DateTime<Utc> = modified.into();
                            if modified_time < cutoff {
                                if fs::remove_file(&path).is_ok() {
                                    deleted_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

/// Get all containers with session logs
pub fn list_containers_with_logs(project_dir: &Path) -> Result<Vec<String>> {
    let logs_base = project_dir.join(".agentsandbox").join("session_logs");
    let mut containers = Vec::new();

    if logs_base.exists() {
        for entry in fs::read_dir(&logs_base)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    containers.push(name.to_string());
                }
            }
        }
    }

    containers.sort();
    Ok(containers)
}

pub fn load_image_agent_versions() -> Result<HashMap<String, String>> {
    let path = get_image_versions_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let data = fs::read_to_string(&path).context("Failed to read image agent versions")?;
    let versions = serde_json::from_str::<HashMap<String, String>>(&data)
        .context("Failed to parse image agent versions")?
        .into_iter()
        .map(|(k, v)| (k, v.trim().to_string()))
        .collect();
    Ok(versions)
}

pub fn save_image_agent_versions(versions: &HashMap<String, String>) -> Result<()> {
    let path = get_image_versions_path()?;
    let json = serde_json::to_string_pretty(versions)
        .context("Failed to serialize image agent versions")?;
    fs::write(&path, json).context("Failed to write image agent versions")
}
