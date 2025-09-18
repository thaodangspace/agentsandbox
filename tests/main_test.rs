#[path = "../src/config.rs"]
mod config;

#[path = "../src/cli.rs"]
mod cli;

#[path = "../src/settings.rs"]
mod settings;

#[path = "../src/language.rs"]
mod language;

#[path = "../src/state.rs"]
mod state;

#[path = "../src/container/mod.rs"]
mod container;

use cli::Agent;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_generate_container_name_sanitizes_directory() {
    let tmp_dir = tempdir().expect("create temp dir");
    let project_dir = tmp_dir.path().join("My Project");
    fs::create_dir(&project_dir).expect("create project dir");

    let name = container::generate_container_name(&project_dir, &Agent::Claude);

    assert!(name.starts_with("agent-claude-my-project-"));
}
