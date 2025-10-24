use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agentsandbox")]
#[command(about = "Agent Sandbox - Docker container manager")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[arg(long, help = "Resume the last created container")]
    pub continue_: bool,

    #[arg(
        long = "add_dir",
        value_name = "DIR",
        help = "Additional directory to mount read-only inside the container"
    )]
    pub add_dir: Option<PathBuf>,

    #[arg(
        long = "worktree",
        value_name = "BRANCH",
        help = "Create and use a git worktree for the specified branch"
    )]
    pub worktree: Option<String>,

    #[arg(long, help = "Attach to container shell without starting the agent")]
    pub shell: bool,

    #[arg(long, help = "Disable clipboard image sharing between host and container")]
    pub no_clipboard: bool,

    #[arg(
        long,
        value_enum,
        default_value_t = Agent::Claude,
        help = "Agent to start in the container (claude, gemini, codex, qwen, cursor)",
    )]
    pub agent: Agent,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    #[command(about = "List containers for this directory and optionally attach")]
    Ls,
    #[command(about = "List all running Agent Sandbox containers and optionally attach")]
    Ps,
    #[command(about = "Remove all containers created from this directory")]
    Cleanup,
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum Agent {
    Claude,
    Gemini,
    Codex,
    Qwen,
    Cursor,
}

impl Agent {
    pub fn command(&self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Gemini => "gemini",
            Agent::Codex => "codex",
            Agent::Qwen => "qwen",
            Agent::Cursor => "cursor-agent",
        }
    }

    pub fn from_container_name(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("agent-")?;
        for agent in [
            Agent::Claude,
            Agent::Gemini,
            Agent::Codex,
            Agent::Qwen,
            Agent::Cursor,
        ] {
            let cmd = agent.command();
            if let Some(after) = rest.strip_prefix(cmd) {
                if after.starts_with('-') {
                    return Some(agent);
                }
            }
        }
        None
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Agent::Claude => "Claude",
            Agent::Gemini => "Gemini",
            Agent::Codex => "Codex",
            Agent::Qwen => "Qwen",
            Agent::Cursor => "Cursor",
        };
        write!(f, "{}", name)
    }
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
