#[path = "../src/cli.rs"]
mod cli;

use cli::Agent;

#[test]
fn test_agent_from_container_name() {
    assert_eq!(
        Agent::from_container_name("agent-claude-proj-main-1234567890"),
        Some(Agent::Claude)
    );
    assert_eq!(
        Agent::from_container_name("agent-gemini-proj-main-1234567890"),
        Some(Agent::Gemini)
    );
    assert_eq!(
        Agent::from_container_name("agent-codex-proj-main-1234567890"),
        Some(Agent::Codex)
    );
    assert_eq!(
        Agent::from_container_name("agent-qwen-proj-main-1234567890"),
        Some(Agent::Qwen)
    );
    assert_eq!(
        Agent::from_container_name("agent-cursor-agent-proj-main-1234567890"),
        Some(Agent::Cursor)
    );
    assert_eq!(Agent::from_container_name("unrelated"), None);
}

#[test]
fn test_agent_display() {
    assert_eq!(format!("{}", Agent::Claude), "Claude");
    assert_eq!(format!("{}", Agent::Gemini), "Gemini");
    assert_eq!(format!("{}", Agent::Codex), "Codex");
    assert_eq!(format!("{}", Agent::Qwen), "Qwen");
    assert_eq!(format!("{}", Agent::Cursor), "Cursor");
}