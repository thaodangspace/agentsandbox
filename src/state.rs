use anyhow::{Context, Result};
use chrono::{Local, Utc};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::env;
use std::path::PathBuf;

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

fn get_session_logs_root() -> Result<PathBuf> {
    let base = if let Some(path) = env::var_os("XDG_STATE_HOME") {
        PathBuf::from(path)
    } else {
        let home_dir = home::home_dir().context("Failed to get home directory")?;
        home_dir.join(".local").join("state")
    };

    let root = base.join("agentsandbox").join("session_logs");
    fs::create_dir_all(&root).context("Failed to create session log state directory")?;
    Ok(root)
}

fn ensure_session_logs_dir(container_name: &str) -> Result<PathBuf> {
    match get_session_logs_root() {
        Ok(root) => {
            let candidate = root.join(container_name);
            match fs::create_dir_all(&candidate) {
                Ok(()) => Ok(candidate),
                Err(state_err) => {
                    let fallback = get_container_dir(container_name)?.join("logs");
                    let candidate_display = candidate.display().to_string();
                    let state_err_msg = state_err.to_string();
                    fs::create_dir_all(&fallback).with_context(|| {
                        format!(
                            "Failed to create legacy session log directory after state directory creation failed at {}: {}",
                            candidate_display, state_err_msg
                        )
                    })?;
                    Ok(fallback)
                }
            }
        }
        Err(state_err) => {
            let fallback = get_container_dir(container_name)?.join("logs");
            let state_err_msg = state_err.to_string();
            fs::create_dir_all(&fallback).with_context(|| {
                format!(
                    "Failed to create legacy session log directory after state root resolution failed: {}",
                    state_err_msg
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

pub fn prepare_session_log(container_name: &str) -> Result<(PathBuf, String)> {
    let logs_dir = ensure_session_logs_dir(container_name)?;
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let container_path = format!("/tmp/session-{}-{}.log", container_name, timestamp);

    Ok((host_path, container_path))
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
