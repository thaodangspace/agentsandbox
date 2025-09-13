use clap::Parser;

#[path = "../src/cli.rs"]
mod cli;

use cli::{Agent, Cli, Commands};

#[test]
fn parse_continue_flag() {
    let cli = Cli::parse_from(["agentsandbox", "--continue"]);
    assert!(cli.continue_);
    assert!(cli.add_dir.is_none());
}

#[test]
fn parse_cleanup_subcommand() {
    let cli = Cli::parse_from(["agentsandbox", "cleanup"]);
    assert!(matches!(cli.command, Some(Commands::Cleanup)));
}

#[test]
fn parse_ls_subcommand() {
    let cli = Cli::parse_from(["agentsandbox", "ls"]);
    assert!(matches!(cli.command, Some(Commands::Ls)));
    assert!(cli.add_dir.is_none());
}

#[test]
fn parse_ps_subcommand() {
    let cli = Cli::parse_from(["agentsandbox", "ps"]);
    assert!(matches!(cli.command, Some(Commands::Ps)));
}

#[test]
fn continue_with_cleanup_subcommand_parses() {
    // We allow specifying --continue with a subcommand; the caller can decide semantics
    let cli = Cli::parse_from(["agentsandbox", "--continue", "cleanup"]);
    assert!(cli.continue_);
    assert!(matches!(cli.command, Some(Commands::Cleanup)));
}

#[test]
fn parse_add_dir() {
    let cli = Cli::parse_from(["agentsandbox", "--add_dir", "/tmp/foo"]);
    assert_eq!(
        cli.add_dir.as_deref(),
        Some(std::path::Path::new("/tmp/foo"))
    );
}

#[test]
fn default_agent_is_claude() {
    let cli = Cli::parse_from(["agentsandbox"]);
    assert!(matches!(cli.agent, Agent::Claude));
}

#[test]
fn parse_agent_option() {
    let cli = Cli::parse_from(["agentsandbox", "--agent", "qwen"]);
    assert!(matches!(cli.agent, Agent::Qwen));
}

#[test]
fn parse_shell_flag() {
    let cli = Cli::parse_from(["agentsandbox", "--shell"]);
    assert!(cli.shell);
}

#[test]
fn parse_worktree_option() {
    let cli = Cli::parse_from(["agentsandbox", "--worktree", "feature"]);
    assert_eq!(cli.worktree.as_deref(), Some("feature"));
}
