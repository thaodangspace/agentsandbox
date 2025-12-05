package container

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/thaodangspace/agentsandbox/internal/config"
)

// Sanitize removes or replaces characters that aren't safe for container names
func Sanitize(s string) string {
	// Replace spaces and underscores with hyphens
	s = strings.ReplaceAll(s, " ", "-")
	s = strings.ReplaceAll(s, "_", "-")

	// Remove or replace other problematic characters
	reg := regexp.MustCompile(`[^a-zA-Z0-9\-]`)
	s = reg.ReplaceAllString(s, "")

	// Convert to lowercase
	s = strings.ToLower(s)

	// Limit length
	if len(s) > 50 {
		s = s[:50]
	}

	// Remove trailing hyphens
	s = strings.TrimRight(s, "-")

	return s
}

// GetCurrentBranch returns the current git branch name
func GetCurrentBranch(dir string) string {
	cmd := exec.Command("git", "rev-parse", "--abbrev-ref", "HEAD")
	cmd.Dir = dir
	output, err := cmd.Output()
	if err != nil {
		return "unknown"
	}

	branch := strings.TrimSpace(string(output))
	return Sanitize(branch)
}

// GenerateContainerName generates a unique container name
func GenerateContainerName(dir string, agent config.Agent) string {
	// Get directory name
	dirName := filepath.Base(dir)
	dirName = Sanitize(dirName)

	// Get branch name
	branchName := GetCurrentBranch(dir)

	// Get agent name
	agentName := Sanitize(agent.Command())

	// Generate timestamp
	timestamp := time.Now().Unix()

	// Format: agent-{agent}-{dir}-{branch}-{timestamp}
	return fmt.Sprintf("agent-%s-%s-%s-%d", agentName, dirName, branchName, timestamp)
}

// ParseContainerName parses a container name and extracts the agent
func ParseContainerName(name string) (config.Agent, error) {
	if !strings.HasPrefix(name, "agent-") {
		return "", fmt.Errorf("invalid container name format")
	}

	// Extract agent
	agent, ok := config.FromContainerName(name)
	if !ok {
		return "", fmt.Errorf("could not extract agent from container name")
	}

	return agent, nil
}

