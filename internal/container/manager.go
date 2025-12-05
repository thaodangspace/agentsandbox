package container

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/thaodangspace/agentsandbox/internal/config"
)

// CheckDockerAvailability checks if Docker is installed and running
func CheckDockerAvailability() error {
	cmd := exec.Command("docker", "--version")
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("Docker is not available or not running: %w", err)
	}
	return nil
}

// CleanupContainers removes all containers created from the current directory
func CleanupContainers(currentDir string) error {
	dirName := Sanitize(filepath.Base(currentDir))
	dirMarker := fmt.Sprintf("-%s-", dirName)

	cmd := exec.Command("docker", "ps", "-a", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if strings.HasPrefix(name, "agent-") && strings.Contains(name, dirMarker) {
			fmt.Printf("Removing container %s\n", name)
			rmCmd := exec.Command("docker", "rm", "-f", name)
			if err := rmCmd.Run(); err != nil {
				return fmt.Errorf("failed to remove container %s: %w", name, err)
			}
		}
	}

	return nil
}

// ListContainers returns a list of containers for the current directory
func ListContainers(currentDir string) ([]string, error) {
	dirName := Sanitize(filepath.Base(currentDir))
	dirMarker := fmt.Sprintf("-%s-", dirName)

	cmd := exec.Command("docker", "ps", "-a", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("failed to list containers: %w", err)
	}

	var containers []string
	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if strings.HasPrefix(name, "agent-") && strings.Contains(name, dirMarker) {
			containers = append(containers, name)
		}
	}

	return containers, nil
}

// FindExistingContainer finds an existing container for the given directory and agent
func FindExistingContainer(currentDir string, agent config.Agent) (string, error) {
	dirName := Sanitize(filepath.Base(currentDir))
	agentName := Sanitize(agent.Command())
	branchName := GetCurrentBranch(currentDir)

	cmd := exec.Command("docker", "ps", "-a", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to list containers: %w", err)
	}

	pattern := fmt.Sprintf("agent-%s-%s-%s-", agentName, dirName, branchName)
	var matching []string

	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if strings.HasPrefix(name, pattern) {
			matching = append(matching, name)
		}
	}

	if len(matching) == 0 {
		return "", nil
	}

	// Sort by timestamp (last part) and return the newest
	sort.Slice(matching, func(i, j int) bool {
		partsI := strings.Split(matching[i], "-")
		partsJ := strings.Split(matching[j], "-")
		if len(partsI) > 0 && len(partsJ) > 0 {
			return partsI[len(partsI)-1] > partsJ[len(partsJ)-1]
		}
		return false
	})

	return matching[0], nil
}

// ContainerInfo represents information about a running container
type ContainerInfo struct {
	Project   string
	Name      string
	Directory string
}

// ListAllContainers returns a list of all running agentsandbox containers
func ListAllContainers() ([]ContainerInfo, error) {
	cmd := exec.Command("docker", "ps", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("failed to list containers: %w", err)
	}

	var containers []ContainerInfo
	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if strings.HasPrefix(name, "agent-") {
			project := ExtractProjectName(name)
			dir, _ := GetContainerDirectory(name)
			containers = append(containers, ContainerInfo{
				Project:   project,
				Name:      name,
				Directory: dir,
			})
		}
	}

	return containers, nil
}

// ExtractProjectName extracts the project name from a container name
func ExtractProjectName(name string) string {
	if !strings.HasPrefix(name, "agent-") {
		return "unknown"
	}

	parts := strings.Split(name, "-")
	if len(parts) < 4 {
		return "unknown"
	}

	// Check if last part is a timestamp (all digits)
	lastPart := parts[len(parts)-1]
	isTimestamp := true
	for _, c := range lastPart {
		if c < '0' || c > '9' {
			isTimestamp = false
			break
		}
	}

	if !isTimestamp || len(lastPart) < 6 {
		return "unknown"
	}

	// Known agents
	agents := []string{"claude", "gemini", "codex", "qwen", "cursor"}
	if len(parts) > 1 {
		potentialAgent := parts[1]
		found := false
		for _, a := range agents {
			if a == potentialAgent {
				found = true
				break
			}
		}

		if found && len(parts) >= 4 {
			// Project parts are between agent (index 2) and timestamp (last-1)
			projectParts := parts[2 : len(parts)-2]
			if len(projectParts) > 0 {
				return strings.Join(projectParts, "-")
			}
		}
	}

	return "unknown"
}

// GetContainerDirectory returns the mounted directory of a container
func GetContainerDirectory(name string) (string, error) {
	cmd := exec.Command("docker", "inspect", "-f",
		"{{range .Mounts}}{{if and .RW (eq .Source .Destination)}}{{.Source}}\n{{end}}{{end}}",
		name)
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to inspect container: %w", err)
	}

	paths := strings.Split(string(output), "\n")
	for _, path := range paths {
		path = strings.TrimSpace(path)
		if path == "" {
			continue
		}

		// Skip config directories
		if strings.Contains(path, "/.claude") || strings.Contains(path, "/.config") {
			continue
		}

		// Check if it's a hidden directory
		baseName := filepath.Base(path)
		if strings.HasPrefix(baseName, ".") {
			continue
		}

		// This looks like a regular project directory
		return path, nil
	}

	return "", nil
}

// AutoRemoveOldContainers removes containers older than the specified minutes
func AutoRemoveOldContainers(minutes int) error {
	if minutes <= 0 {
		return nil
	}

	cutoff := time.Now().Add(-time.Duration(minutes) * time.Minute)

	cmd := exec.Command("docker", "ps", "-a", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if !strings.HasPrefix(name, "agent-") {
			continue
		}

		// Get container creation time
		inspectCmd := exec.Command("docker", "inspect", "-f", "{{.Created}}", name)
		inspectOutput, err := inspectCmd.Output()
		if err != nil {
			continue
		}

		createdStr := strings.TrimSpace(string(inspectOutput))
		created, err := time.Parse(time.RFC3339Nano, createdStr)
		if err != nil {
			continue
		}

		if created.After(cutoff) {
			continue
		}

		fmt.Printf("Auto removing old container %s\n", name)
		rmCmd := exec.Command("docker", "rm", "-f", name)
		if err := rmCmd.Run(); err != nil {
			return fmt.Errorf("failed to remove container %s: %w", name, err)
		}
	}

	return nil
}

// IsContainerRunning checks if a container is currently running
func IsContainerRunning(name string) (bool, error) {
	cmd := exec.Command("docker", "inspect", "-f", "{{.State.Running}}", name)
	output, err := cmd.Output()
	if err != nil {
		return false, nil
	}

	status := strings.TrimSpace(string(output))
	return status == "true", nil
}

// ContainerExists checks if a container exists
func ContainerExists(name string) (bool, error) {
	cmd := exec.Command("docker", "inspect", name)
	err := cmd.Run()
	return err == nil, nil
}

// LoadLastContainer loads the last used container name
func LoadLastContainer() (string, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}

	stateDir := filepath.Join(homeDir, ".config", "agentsandbox")
	lastFile := filepath.Join(stateDir, "last_container")

	data, err := os.ReadFile(lastFile)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}

	return strings.TrimSpace(string(data)), nil
}

// SaveLastContainer saves the last used container name
func SaveLastContainer(name string) error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	stateDir := filepath.Join(homeDir, ".config", "agentsandbox")
	if err := os.MkdirAll(stateDir, 0755); err != nil {
		return err
	}

	lastFile := filepath.Join(stateDir, "last_container")
	return os.WriteFile(lastFile, []byte(name), 0644)
}

