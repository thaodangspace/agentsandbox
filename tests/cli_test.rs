use clap::Parser;

#[path = "../src/cli.rs"]
mod cli;

use cli::{Agent, Cli, Commands};

#[test]
fn parse_continue_flag() {
    let cli = Cli::parse_from(["codesandbox", "--continue"]);
    assert!(cli.continue_);
    assert!(cli.add_dir.is_none());
}

#[test]
fn parse_cleanup_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "cleanup"]);
    assert!(matches!(cli.command, Some(Commands::Cleanup)));
}

#[test]
fn parse_ls_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "ls"]);
    assert!(matches!(cli.command, Some(Commands::Ls)));
    assert!(cli.add_dir.is_none());
}

#[test]
fn parse_ps_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "ps"]);
    assert!(matches!(cli.command, Some(Commands::Ps)));
}

#[test]
fn parse_serve_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "serve"]);
    assert!(matches!(
        cli.command,
        Some(Commands::Serve { daemon: false })
    ));
}

#[test]
fn parse_serve_daemon_flag() {
    let cli = Cli::parse_from(["codesandbox", "serve", "-d"]);
    assert!(matches!(
        cli.command,
        Some(Commands::Serve { daemon: true })
    ));
}

#[test]
fn parse_stop_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "stop"]);
    assert!(matches!(cli.command, Some(Commands::Stop)));
}

#[test]
fn parse_restart_subcommand() {
    let cli = Cli::parse_from(["codesandbox", "restart"]);
    assert!(matches!(
        cli.command,
        Some(Commands::Restart { daemon: false })
    ));
}

#[test]
fn parse_restart_daemon_flag() {
    let cli = Cli::parse_from(["codesandbox", "restart", "-d"]);
    assert!(matches!(
        cli.command,
        Some(Commands::Restart { daemon: true })
    ));
}

#[test]
fn continue_with_cleanup_subcommand_parses() {
    // We allow specifying --continue with a subcommand; the caller can decide semantics
    let cli = Cli::parse_from(["codesandbox", "--continue", "cleanup"]);
    assert!(cli.continue_);
    assert!(matches!(cli.command, Some(Commands::Cleanup)));
}

#[test]
fn parse_add_dir() {
    let cli = Cli::parse_from(["codesandbox", "--add_dir", "/tmp/foo"]);
    assert_eq!(
        cli.add_dir.as_deref(),
        Some(std::path::Path::new("/tmp/foo"))
    );
}

#[test]
fn default_agent_is_claude() {
    let cli = Cli::parse_from(["codesandbox"]);
    assert!(matches!(cli.agent, Agent::Claude));
}

#[test]
fn parse_agent_option() {
    let cli = Cli::parse_from(["codesandbox", "--agent", "qwen"]);
    assert!(matches!(cli.agent, Agent::Qwen));
}

#[test]
fn parse_shell_flag() {
    let cli = Cli::parse_from(["codesandbox", "--shell"]);
    assert!(cli.shell);
}

#[test]
fn parse_web_flag() {
    let cli = Cli::parse_from(["codesandbox", "--web"]);
    assert!(cli.web);
}

#[test]
fn parse_worktree_option() {
    let cli = Cli::parse_from(["codesandbox", "--worktree", "feature"]);
    assert_eq!(cli.worktree.as_deref(), Some("feature"));
}
