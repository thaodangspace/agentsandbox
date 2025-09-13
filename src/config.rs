use std::env;
use std::path::{Path, PathBuf};

pub fn get_claude_config_dir() -> Option<PathBuf> {
    if let Ok(home_env) = env::var("HOME") {
        let claude_dir = Path::new(&home_env).join(".claude");
        if claude_dir.exists() {
            return Some(claude_dir);
        }
    } else if let Some(home_dir) = home::home_dir() {
        let claude_dir = home_dir.join(".claude");
        if claude_dir.exists() {
            return Some(claude_dir);
        }
    }

    // Also check XDG config directory
    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        let claude_dir = Path::new(&xdg_config).join("claude");
        if claude_dir.exists() {
            return Some(claude_dir);
        }
    }

    None
}

pub fn get_claude_json_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Check current directory
    let current_dir_config = Path::new(".claude.json");
    if current_dir_config.exists() {
        paths.push(current_dir_config.to_path_buf());
    }

    // Check home directory via HOME env var first, falling back to system home
    if let Ok(home_env) = env::var("HOME") {
        let home_config = Path::new(&home_env).join(".claude.json");
        if home_config.exists() {
            paths.push(home_config);
        }
    } else if let Some(home_dir) = home::home_dir() {
        let home_config = home_dir.join(".claude.json");
        if home_config.exists() {
            paths.push(home_config);
        }
    }

    // Check .claude directory
    if let Some(claude_dir) = get_claude_config_dir() {
        let claude_config = claude_dir.join("config.json");
        if claude_config.exists() {
            paths.push(claude_config);
        }
    }

    // Check XDG config directory
    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        let xdg_config_file = Path::new(&xdg_config).join("claude").join("config.json");
        if xdg_config_file.exists() {
            paths.push(xdg_config_file);
        }
    }

    paths
}
