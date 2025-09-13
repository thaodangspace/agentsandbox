use chrono::Local;
use std::path::Path;
use std::process::Command;

use crate::cli::Agent;

pub(crate) fn sanitize(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

pub fn generate_container_name(current_dir: &Path, agent: &Agent) -> String {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(sanitize)
        .unwrap_or_else(|| "unknown".to_string());

    let agent_name = sanitize(agent.command());

    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(current_dir)
        .output();
    let branch_name = branch_output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| sanitize(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or_else(|| "unknown".to_string());

    let timestamp = Local::now().format("%y%m%d%H%M").to_string();

    format!("csb-{agent_name}-{dir_name}-{branch_name}-{timestamp}")
}
