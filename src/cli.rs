use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "codesandbox")]
#[command(about = "Code Sandbox - Docker container manager")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[arg(
        long,
        help = "Resume the last created container",
    )]
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

    #[arg(long, help = "Open web UI instead of attaching in terminal")]
    pub web: bool,

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
    #[command(about = "List containers for this directory and optionally attach to one")]
    Ls,
    #[command(
        about = "List all running Code Sandbox containers and optionally attach or open their directory"
    )]
    Ps,
    #[command(about = "Start the Code Sandbox API server")]
    Serve {
        #[arg(short = 'd', long = "daemon", help = "Run server in the background")]
        daemon: bool,
    },
    #[command(about = "Stop the running Code Sandbox API server")]
    Stop,
    #[command(about = "Restart the Code Sandbox API server")]
    Restart {
        #[arg(short = 'd', long = "daemon", help = "Run server in the background")]
        daemon: bool,
    },
    #[command(about = "Remove all containers created from this directory")]
    Cleanup,
}

#[derive(ValueEnum, Clone, Debug)]
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
