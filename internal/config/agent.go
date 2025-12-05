package config

import (
	"fmt"
	"strings"
)

// Agent represents an AI agent type
type Agent string

const (
	AgentClaude Agent = "claude"
	AgentGemini Agent = "gemini"
	AgentCodex  Agent = "codex"
	AgentQwen   Agent = "qwen"
	AgentCursor Agent = "cursor"
)

// Command returns the executable command name for the agent
func (a Agent) Command() string {
	switch a {
	case AgentClaude:
		return "claude"
	case AgentGemini:
		return "gemini"
	case AgentCodex:
		return "codex"
	case AgentQwen:
		return "qwen"
	case AgentCursor:
		return "cursor-agent"
	default:
		return string(a)
	}
}

// CacheArg returns the environment variable name for cache busting
func (a Agent) CacheArg() string {
	switch a {
	case AgentClaude:
		return "CLAUDE_CACHE_BUST"
	case AgentGemini:
		return "GEMINI_CACHE_BUST"
	case AgentCodex:
		return "CODEX_CACHE_BUST"
	case AgentQwen:
		return "QWEN_CACHE_BUST"
	case AgentCursor:
		return "CURSOR_CACHE_BUST"
	default:
		return fmt.Sprintf("%s_CACHE_BUST", strings.ToUpper(string(a)))
	}
}

// DisplayName returns the human-readable name for the agent
func (a Agent) DisplayName() string {
	switch a {
	case AgentClaude:
		return "Claude"
	case AgentGemini:
		return "Gemini"
	case AgentCodex:
		return "Codex"
	case AgentQwen:
		return "Qwen"
	case AgentCursor:
		return "Cursor"
	default:
		return string(a)
	}
}

// String implements the Stringer interface
func (a Agent) String() string {
	return a.DisplayName()
}

// FromContainerName extracts the agent type from a container name
func FromContainerName(name string) (Agent, bool) {
	if !strings.HasPrefix(name, "agent-") {
		return "", false
	}

	rest := strings.TrimPrefix(name, "agent-")
	agents := []Agent{AgentClaude, AgentGemini, AgentCodex, AgentQwen, AgentCursor}

	for _, agent := range agents {
		cmd := agent.Command()
		if strings.HasPrefix(rest, cmd+"-") {
			return agent, true
		}
	}

	return "", false
}

// ValidateAgent checks if the given string is a valid agent
func ValidateAgent(s string) (Agent, error) {
	agent := Agent(strings.ToLower(s))
	switch agent {
	case AgentClaude, AgentGemini, AgentCodex, AgentQwen, AgentCursor:
		return agent, nil
	default:
		return "", fmt.Errorf("invalid agent: %s (valid: claude, gemini, codex, qwen, cursor)", s)
	}
}

// AllAgents returns a list of all supported agents
func AllAgents() []Agent {
	return []Agent{AgentClaude, AgentGemini, AgentCodex, AgentQwen, AgentCursor}
}

