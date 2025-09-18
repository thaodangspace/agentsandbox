use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
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

fn get_container_dir(container_name: &str) -> Result<PathBuf> {
    let dir = get_base_config_dir()?
        .join("containers")
        .join(container_name);
    fs::create_dir_all(&dir).context("Failed to create container state directory")?;
    Ok(dir)
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
    let logs_dir = get_container_dir(container_name)?.join("logs");
    fs::create_dir_all(&logs_dir).context("Failed to create session log directory")?;

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let mut file_name = format!("session-{}.log", timestamp);
    let mut host_path = logs_dir.join(&file_name);
    let mut counter = 1;

    while host_path.exists() {
        file_name = format!("session-{}-{}.log", timestamp, counter);
        host_path = logs_dir.join(&file_name);
        counter += 1;
    }

    let container_path = format!("/tmp/{}", file_name);
    Ok((host_path, container_path))
}
