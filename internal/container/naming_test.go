package container

import (
	"strings"
	"testing"

	"github.com/thaodangspace/agentsandbox/internal/config"
)

func TestSanitize(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"simple", "myproject", "myproject"},
		{"with spaces", "my project", "my-project"},
		{"with underscores", "my_project", "my-project"},
		{"with special chars", "my@project#123", "myproject123"},
		{"mixed", "My_Project Name!", "my-project-name"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := Sanitize(tt.input); got != tt.want {
				t.Errorf("Sanitize() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestGenerateContainerName(t *testing.T) {
	name := GenerateContainerName("/home/user/myproject", config.AgentClaude)

	// Check it starts with agent-claude
	if !strings.HasPrefix(name, "agent-claude-") {
		t.Errorf("GenerateContainerName() = %v, should start with 'agent-claude-'", name)
	}

	// Check it contains the project name
	if !strings.Contains(name, "myproject") {
		t.Errorf("GenerateContainerName() = %v, should contain 'myproject'", name)
	}

	// Check it has a timestamp at the end
	parts := strings.Split(name, "-")
	if len(parts) < 4 {
		t.Errorf("GenerateContainerName() = %v, should have at least 4 parts", name)
	}
}

func TestExtractProjectName(t *testing.T) {
	tests := []struct {
		name          string
		containerName string
		want          string
	}{
		{"valid", "agent-claude-myproject-main-1234567890", "myproject"},
		{"multi part", "agent-gemini-my-project-feature-1234567890", "my-project"},
		{"invalid format", "invalid-name", "unknown"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ExtractProjectName(tt.containerName); got != tt.want {
				t.Errorf("ExtractProjectName() = %v, want %v", got, tt.want)
			}
		})
	}
}

