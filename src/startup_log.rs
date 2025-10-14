use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use unicode_width::UnicodeWidthStr;

use crate::cli::Agent;

static STARTUP_LOG: Lazy<Mutex<Option<StartupLog>>> = Lazy::new(|| Mutex::new(None));
static STARTUP_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StartupMode {
    Create,
    Resume,
}

struct StartupLog {
    mode: StartupMode,
    container_name: String,
    workspace: PathBuf,
    agent_label: String,
    agent_command: String,
    events: Vec<String>,
    warnings: Vec<String>,
}

pub struct StartupOutcome<'a> {
    pub attach: bool,
    pub shell: bool,
    pub agent_command: &'a str,
    pub agent_continue: bool,
}

impl StartupLog {
    fn new(mode: StartupMode, container_name: &str, workspace: &Path, agent: &Agent) -> Self {
        Self {
            mode,
            container_name: container_name.to_string(),
            workspace: workspace.to_path_buf(),
            agent_label: agent.to_string(),
            agent_command: agent.command().to_string(),
            events: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn info_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("📦 Container: {}", self.container_name),
            format!("🤖 Agent: {} ({})", self.agent_label, self.agent_command),
            format!("📂 Workspace: {}", self.workspace.display()),
        ];

        match self.mode {
            StartupMode::Create => lines.push("🚀 Launching new sandbox session".to_string()),
            StartupMode::Resume => lines.push("🔁 Resuming existing sandbox session".to_string()),
        }

        lines
    }

    fn footer_lines(&self, outcome: &StartupOutcome<'_>) -> Vec<String> {
        let mut lines = Vec::new();

        if outcome.attach {
            if outcome.shell {
                lines.push("🖥️  Opening interactive shell".to_string());
            } else {
                let mut launch = format!("🤖 Launching {}", outcome.agent_command);
                if outcome.agent_continue {
                    launch.push_str(" --continue");
                }
                lines.push(launch);
            }
            lines.push(
                "⏎ Use `exit` to leave or `ctrl+p ctrl+q` to detach without stopping.".to_string(),
            );
            lines.push(format!(
                "📝 Session logs live under ~/.config/agentsandbox/containers/{}/logs/",
                self.container_name
            ));
        } else {
            lines.push(
                "✅ Container is ready. Attach later with `agentsandbox --continue`.".to_string(),
            );
        }

        lines
    }
}

pub fn begin_session(mode: StartupMode, container_name: &str, workspace: &Path, agent: &Agent) {
    let mut guard = STARTUP_LOG.lock().unwrap();
    *guard = Some(StartupLog::new(mode, container_name, workspace, agent));

    STARTUP_ACTIVE.store(true, Ordering::Relaxed);

    if let Some(log) = guard.as_mut() {
        match mode {
            StartupMode::Create => log
                .events
                .push(format!("📦 Preparing container: {}", container_name)),
            StartupMode::Resume => log
                .events
                .push(format!("🔁 Preparing to resume: {}", container_name)),
        }
    }
}

pub fn event(message: impl Into<String>) {
    if !STARTUP_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if let Some(log) = STARTUP_LOG.lock().unwrap().as_mut() {
        log.events.push(message.into());
    }
}

pub fn warn(message: impl Into<String>) {
    if !STARTUP_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if let Some(log) = STARTUP_LOG.lock().unwrap().as_mut() {
        log.warnings.push(format!("⚠️  {}", message.into()));
    }
}

pub fn finalize(outcome: StartupOutcome<'_>) {
    if !STARTUP_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let mut guard = STARTUP_LOG.lock().unwrap();
    let log = guard.take();
    drop(guard);

    let Some(log) = log else {
        STARTUP_ACTIVE.store(false, Ordering::Relaxed);
        return;
    };

    let mut lines = log.info_lines();

    if !log.events.is_empty() {
        lines.push(String::new());
        lines.extend(log.events.iter().cloned());
    }

    if !log.warnings.is_empty() {
        lines.push(String::new());
        lines.extend(log.warnings.iter().cloned());
    }

    let footer = log.footer_lines(&outcome);
    if !footer.is_empty() {
        lines.push(String::new());
        lines.extend(footer);
    }

    let content_width = lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0);
    let title = " Agent Sandbox ";
    let title_width = UnicodeWidthStr::width(title);
    let inner_width = std::cmp::max(content_width, title_width);

    println!("╭─{:─^width$}─╮", title, width = inner_width);
    for line in lines {
        if line.is_empty() {
            println!("│ {:width$} │", "", width = inner_width);
        } else {
            println!("│ {:width$} │", line, width = inner_width);
        }
    }
    println!("╰{}╯", "─".repeat(inner_width + 2));
    STARTUP_ACTIVE.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Agent;
    use std::path::Path;

    #[test]
    fn info_lines_reflect_mode_and_metadata() {
        let workspace = Path::new("/tmp/workspace");
        let log = StartupLog::new(
            StartupMode::Create,
            "container-1",
            workspace,
            &Agent::Claude,
        );
        let info = log.info_lines();

        assert_eq!(
            info,
            vec![
                "📦 Container: container-1".to_string(),
                "🤖 Agent: Claude (claude)".to_string(),
                format!("📂 Workspace: {}", workspace.display()),
                "🚀 Launching new sandbox session".to_string(),
            ]
        );
    }

    #[test]
    fn footer_lines_cover_attach_and_detach_modes() {
        let workspace = Path::new("/tmp/workspace");
        let log = StartupLog::new(
            StartupMode::Resume,
            "container-2",
            workspace,
            &Agent::Claude,
        );

        let detached = log.footer_lines(&StartupOutcome {
            attach: false,
            shell: false,
            agent_command: "claude",
            agent_continue: false,
        });
        assert_eq!(
            detached,
            vec!["✅ Container is ready. Attach later with `agentsandbox --continue`.".to_string(),]
        );

        let shell_footer = log.footer_lines(&StartupOutcome {
            attach: true,
            shell: true,
            agent_command: "claude",
            agent_continue: false,
        });
        assert_eq!(
            shell_footer,
            vec![
                "🖥️  Opening interactive shell".to_string(),
                "⏎ Use `exit` to leave or `ctrl+p ctrl+q` to detach without stopping.".to_string(),
                "📝 Session logs live under ~/.config/agentsandbox/containers/container-2/logs/"
                    .to_string(),
            ]
        );

        let agent_footer = log.footer_lines(&StartupOutcome {
            attach: true,
            shell: false,
            agent_command: "claude",
            agent_continue: true,
        });
        assert_eq!(
            agent_footer,
            vec![
                "🤖 Launching claude --continue".to_string(),
                "⏎ Use `exit` to leave or `ctrl+p ctrl+q` to detach without stopping.".to_string(),
                "📝 Session logs live under ~/.config/agentsandbox/containers/container-2/logs/"
                    .to_string(),
            ]
        );
    }
}
