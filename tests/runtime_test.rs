#[path = "../src/cli.rs"]
mod cli;

#[path = "../src/config.rs"]
mod config;

#[path = "../src/language.rs"]
mod language;

#[path = "../src/settings.rs"]
mod settings;

#[path = "../src/state.rs"]
mod state;

#[path = "../src/container/mod.rs"]
mod container;

use std::path::Path;

#[test]
fn build_command_includes_continue() {
    let cmd =
        container::build_agent_command(Path::new("/project"), &cli::Agent::Claude, true, None);
    assert!(cmd.contains("claude --continue"));
}

#[test]
fn build_command_without_continue() {
    let cmd =
        container::build_agent_command(Path::new("/project"), &cli::Agent::Claude, false, None);
    assert!(!cmd.contains("--continue"));
}
