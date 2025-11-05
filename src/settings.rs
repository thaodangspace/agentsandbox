use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub auto_remove_minutes: Option<u64>,
    #[serde(default)]
    pub skip_permission_flags: HashMap<String, String>,
    #[serde(default = "default_env_files")]
    pub env_files: Vec<String>,
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u64,
}

impl Default for Settings {
    fn default() -> Self {
        let mut default_flags = HashMap::new();
        default_flags.insert(
            "claude".to_string(),
            "--dangerously-skip-permissions".to_string(),
        );
        default_flags.insert("gemini".to_string(), "--yolo".to_string());
        default_flags.insert("qwen".to_string(), "--yolo".to_string());
        // Codex can also require permissive flags in some environments
        default_flags.insert("codex".to_string(), "--yolo".to_string());

        Self {
            auto_remove_minutes: Some(60),
            skip_permission_flags: default_flags,
            env_files: default_env_files(),
            log_retention_days: default_log_retention_days(),
        }
    }
}

fn default_log_retention_days() -> u64 {
    30
}

fn default_env_files() -> Vec<String> {
    vec![
        ".env".to_string(),
        ".env.local".to_string(),
        ".env.development.local".to_string(),
        ".env.test.local".to_string(),
        ".env.production.local".to_string(),
    ]
}

fn settings_file_path() -> PathBuf {
    if let Ok(dir) = env::var("AGENTSANDBOX_CONFIG_HOME") {
        return PathBuf::from(dir).join("settings.json");
    }
    let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    home.join(".config")
        .join("agentsandbox")
        .join("settings.json")
}

pub fn load_settings() -> Result<Settings> {
    let path = settings_file_path();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(settings) = serde_json::from_str::<Settings>(&data) {
            return Ok(settings);
        }
    }
    Ok(Settings::default())
}
