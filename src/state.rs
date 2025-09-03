use anyhow::{Context, Result};
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
