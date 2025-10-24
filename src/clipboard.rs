use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Get the clipboard directory path (~/.config/agentsandbox/clipboard)
pub fn get_clipboard_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().context("Failed to get home directory")?;
    let clipboard_dir = home_dir
        .join(".config")
        .join("agentsandbox")
        .join("clipboard");
    Ok(clipboard_dir)
}

/// Ensure the clipboard directory exists
pub fn ensure_clipboard_dir() -> Result<PathBuf> {
    let clipboard_dir = get_clipboard_dir()?;
    fs::create_dir_all(&clipboard_dir)
        .with_context(|| format!("Failed to create clipboard directory at {:?}", clipboard_dir))?;
    Ok(clipboard_dir)
}

/// Get the path to the clipboard watcher PID file
pub fn get_watcher_pid_file() -> Result<PathBuf> {
    let home_dir = home::home_dir().context("Failed to get home directory")?;
    let config_dir = home_dir.join(".config").join("agentsandbox");
    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
    Ok(config_dir.join("clipboard_watcher.pid"))
}

/// Save the clipboard watcher PID
pub fn save_watcher_pid(pid: u32) -> Result<()> {
    let pid_file = get_watcher_pid_file()?;
    fs::write(&pid_file, pid.to_string()).context("Failed to save clipboard watcher PID")?;
    Ok(())
}

/// Load the clipboard watcher PID
pub fn load_watcher_pid() -> Result<Option<u32>> {
    let pid_file = get_watcher_pid_file()?;
    if !pid_file.exists() {
        return Ok(None);
    }

    let pid_str = fs::read_to_string(&pid_file)
        .context("Failed to read clipboard watcher PID")?
        .trim()
        .to_string();

    if pid_str.is_empty() {
        return Ok(None);
    }

    match pid_str.parse::<u32>() {
        Ok(pid) => Ok(Some(pid)),
        Err(_) => Ok(None),
    }
}

/// Clear the clipboard watcher PID file
pub fn clear_watcher_pid() -> Result<()> {
    let pid_file = get_watcher_pid_file()?;
    if pid_file.exists() {
        fs::remove_file(pid_file).context("Failed to remove clipboard watcher PID file")?;
    }
    Ok(())
}

/// Check if a process with the given PID is running
pub fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}
