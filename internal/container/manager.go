package container

import (
	"fmt"
	"github.com/thaodangspace/agentsandbox/internal/config"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
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
		if strings.HasPrefix(name, "agentsandbox-") && strings.Contains(name, dirMarker) {
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
		if strings.HasPrefix(name, "agentsandbox-") && strings.Contains(name, dirMarker) {
			containers = append(containers, name)
		}
	}

	return containers, nil
}

// FindExistingContainer finds an existing container for the given directory and agent
func FindExistingContainer(currentDir string, agent config.Agent) (string, error) {
	dirName := Sanitize(filepath.Base(currentDir))

	cmd := exec.Command("docker", "ps", "-a", "--format", "{{.Names}}")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to list containers: %w", err)
	}

	pattern := fmt.Sprintf("agentsandbox-%s", dirName)

	names := strings.Split(string(output), "\n")
	for _, name := range names {
		name = strings.TrimSpace(name)
		if name == pattern {
			return name, nil
		}
	}

	return "", nil
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
		if strings.HasPrefix(name, "agentsandbox-") {
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
	if !strings.HasPrefix(name, "agentsandbox-") {
		return "unknown"
	}

	// Format is agentsandbox-{project_dir}, so just strip the prefix
	return strings.TrimPrefix(name, "agentsandbox-")
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

// ImageInfo represents information about an agentsandbox Docker image
type ImageInfo struct {
	Name    string
	Tag     string
	ID      string
	Created string
	Size    string
}

// ListAgentSandboxImages returns a list of all agentsandbox Docker images
func ListAgentSandboxImages() ([]ImageInfo, error) {
	cmd := exec.Command("docker", "images", "--format", "{{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.CreatedAt}}\t{{.Size}}", "agentsandbox-image")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("failed to list images: %w", err)
	}

	var images []ImageInfo
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		parts := strings.Split(line, "\t")
		if len(parts) < 4 {
			continue
		}

		fullName := parts[0]
		nameParts := strings.SplitN(fullName, ":", 2)
		tag := ""
		if len(nameParts) == 2 {
			tag = nameParts[1]
		}

		images = append(images, ImageInfo{
			Name:    fullName,
			Tag:     tag,
			ID:      parts[1],
			Created: parts[2],
			Size:    parts[3],
		})
	}

	return images, nil
}

// CleanupImages removes all agentsandbox Docker images
func CleanupImages() error {
	images, err := ListAgentSandboxImages()
	if err != nil {
		return err
	}

	if len(images) == 0 {
		fmt.Println("No agentsandbox images to clean up")
		return nil
	}

	for _, img := range images {
		fmt.Printf("Removing image: %s\n", img.Name)
		rmCmd := exec.Command("docker", "rmi", img.Name)
		if err := rmCmd.Run(); err != nil {
			fmt.Printf("Warning: failed to remove image %s: %v\n", img.Name, err)
		}
	}

	fmt.Printf("Cleaned up %d image(s)\n", len(images))
	return nil
}

// CleanupUnusedImages removes agentsandbox images that are not in use by any container
func CleanupUnusedImages() error {
	// Get all container image IDs
	containerCmd := exec.Command("docker", "ps", "-a", "--format", "{{.Image}}")
	containerOutput, err := containerCmd.Output()
	if err != nil {
		return fmt.Errorf("failed to list container images: %w", err)
	}

	usedImages := make(map[string]bool)
	for _, img := range strings.Split(string(containerOutput), "\n") {
		img = strings.TrimSpace(img)
		if img != "" {
			usedImages[img] = true
		}
	}

	images, err := ListAgentSandboxImages()
	if err != nil {
		return err
	}

	removed := 0
	for _, img := range images {
		if usedImages[img.Name] {
			fmt.Printf("Skipping in-use image: %s\n", img.Name)
			continue
		}

		fmt.Printf("Removing unused image: %s\n", img.Name)
		rmCmd := exec.Command("docker", "rmi", img.Name)
		if err := rmCmd.Run(); err != nil {
			fmt.Printf("Warning: failed to remove image %s: %v\n", img.Name, err)
		} else {
			removed++
		}
	}

	if removed > 0 {
		fmt.Printf("Cleaned up %d unused image(s)\n", removed)
	} else {
		fmt.Println("No unused images to clean up")
	}
	return nil
}
