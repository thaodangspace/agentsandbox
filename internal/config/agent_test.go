package config

import (
	"testing"
)

func TestAgentCommand(t *testing.T) {
	tests := []struct {
		agent   Agent
		command string
	}{
		{AgentClaude, "claude"},
		{AgentGemini, "gemini"},
		{AgentCodex, "codex"},
		{AgentQwen, "qwen"},
		{AgentCursor, "cursor-agent"},
	}

	for _, tt := range tests {
		t.Run(string(tt.agent), func(t *testing.T) {
			if got := tt.agent.Command(); got != tt.command {
				t.Errorf("Agent.Command() = %v, want %v", got, tt.command)
			}
		})
	}
}

func TestFromContainerName(t *testing.T) {
	tests := []struct {
		name          string
		containerName string
		wantAgent     Agent
		wantOk        bool
	}{
		{"valid claude", "agent-claude-myproject-main-1234567890", AgentClaude, true},
		{"valid gemini", "agent-gemini-myproject-main-1234567890", AgentGemini, true},
		{"invalid prefix", "container-claude-myproject", "", false},
		{"no agent", "agent-myproject-main", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotAgent, gotOk := FromContainerName(tt.containerName)
			if gotAgent != tt.wantAgent {
				t.Errorf("FromContainerName() agent = %v, want %v", gotAgent, tt.wantAgent)
			}
			if gotOk != tt.wantOk {
				t.Errorf("FromContainerName() ok = %v, want %v", gotOk, tt.wantOk)
			}
		})
	}
}

func TestValidateAgent(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    Agent
		wantErr bool
	}{
		{"valid claude", "claude", AgentClaude, false},
		{"valid gemini", "gemini", AgentGemini, false},
		{"valid uppercase", "CLAUDE", AgentClaude, false},
		{"invalid", "invalid", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ValidateAgent(tt.input)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateAgent() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if got != tt.want {
				t.Errorf("ValidateAgent() = %v, want %v", got, tt.want)
			}
		})
	}
}

