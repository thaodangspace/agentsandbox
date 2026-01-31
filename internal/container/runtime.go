package container

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/state"
)

const dockerfileTemplate = `FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install basic tools
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    python3 \
    python3-pip \
    nodejs \
    npm \
    sudo \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create user
ARG USERNAME=ubuntu
ARG USER_UID=1000
ARG USER_GID=1000

RUN set -e; \
    if ! getent group $USER_GID >/dev/null; then \
        groupadd --gid $USER_GID $USERNAME; \
    fi && \
    useradd --uid $USER_UID --gid $USER_GID -m -s /bin/bash $USERNAME && \
    echo "$USERNAME ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Install AI agents (with cache busting args)
ARG CLAUDE_CACHE_BUST
ARG GEMINI_CACHE_BUST
ARG CODEX_CACHE_BUST
ARG QWEN_CACHE_BUST
ARG CURSOR_CACHE_BUST

USER $USERNAME
WORKDIR /home/$USERNAME

# Install Claude (example - adjust per actual installation method)
RUN curl -fsSL https://claude.ai/install.sh | bash || true

WORKDIR /workspace
CMD ["/bin/bash"]
`

// CreateDockerfile creates a Dockerfile with the specified user info
func CreateDockerfile(username string, uid, gid int) (string, error) {
	tempDir := os.TempDir()
	dockerfilePath := filepath.Join(tempDir, "Dockerfile.agentsandbox")

	content := strings.Replace(dockerfileTemplate, "ARG USERNAME=ubuntu", fmt.Sprintf("ARG USERNAME=%s", username), 1)
	content = strings.Replace(content, "ARG USER_UID=1000", fmt.Sprintf("ARG USER_UID=%d", uid), 1)
	content = strings.Replace(content, "ARG USER_GID=1000", fmt.Sprintf("ARG USER_GID=%d", gid), 1)

	if err := os.WriteFile(dockerfilePath, []byte(content), 0o644); err != nil {
		return "", fmt.Errorf("failed to write Dockerfile: %w", err)
	}

	return dockerfilePath, nil
}

// BuildDockerImage builds the agentsandbox Docker image
func BuildDockerImage(username string) error {
	// Get host UID/GID
	uidCmd := exec.Command("id", "-u")
	uidOutput, err := uidCmd.Output()
	if err != nil {
		return fmt.Errorf("failed to get host UID: %w", err)
	}

	gidCmd := exec.Command("id", "-g")
	gidOutput, err := gidCmd.Output()
	if err != nil {
		return fmt.Errorf("failed to get host GID: %w", err)
	}

	uid := strings.TrimSpace(string(uidOutput))
	gid := strings.TrimSpace(string(gidOutput))

	// Create Dockerfile
	dockerfilePath, err := CreateDockerfile(username,
		parseInt(uid), parseInt(gid))
	if err != nil {
		return err
	}
	defer os.Remove(dockerfilePath)

	fmt.Println("Building Docker image...")
	cmd := exec.Command("docker", "build",
		"-t", "agentsandbox-image",
		"--build-arg", fmt.Sprintf("USERNAME=%s", username),
		"--build-arg", fmt.Sprintf("USER_UID=%s", uid),
		"--build-arg", fmt.Sprintf("USER_GID=%s", gid),
		"-f", dockerfilePath,
		".")

	cmd.Dir = filepath.Dir(dockerfilePath)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("Docker build failed: %w", err)
	}

	fmt.Println("Docker image built successfully")
	return nil
}

// CreateContainer creates and starts a new container
func CreateContainer(
	containerName string,
	currentDir string,
	additionalDir string,
	agent config.Agent,
	skipPermissionFlag string,
	shellMode bool,
	attach bool,
) error {
	username := os.Getenv("USER")
	if username == "" {
		username = "ubuntu"
	}

	// Build the image
	if err := BuildDockerImage(username); err != nil {
		return err
	}

	// Prepare docker run command
	args := []string{
		"run", "-d", "-it",
		"--name", containerName,
		"-v", fmt.Sprintf("%s:/workspace", currentDir),
	}

	// Handle node_modules isolation for Node.js projects
	packageJSON := filepath.Join(currentDir, "package.json")
	if _, err := os.Stat(packageJSON); err == nil {
		args = append(args, "-v", "/workspace/node_modules")
		fmt.Println("Isolating node_modules with container volume")
	}

	// Handle env file overlays
	settings, _ := config.LoadSettings()
	for _, envFile := range settings.EnvFiles {
		envPath := filepath.Join(currentDir, envFile)
		if _, err := os.Stat(envPath); err == nil {
			// Create empty temp file to overlay
			tempFile, err := os.CreateTemp("", "env-overlay-*")
			if err == nil {
				tempFile.Close()
				containerEnvPath := filepath.Join("/workspace", envFile)
				args = append(args, "-v", fmt.Sprintf("%s:%s:ro", tempFile.Name(), containerEnvPath))
				fmt.Printf("Excluding %s from container mount\n", envFile)
			}
		}
	}

	// Additional directory (read-only)
	if additionalDir != "" {
		args = append(args, "-v", fmt.Sprintf("%s:%s:ro", additionalDir, additionalDir))
		fmt.Printf("Mounting additional directory read-only: %s\n", additionalDir)
	}

	// Mount agent configs
	if err := mountAgentConfigs(&args, agent, currentDir, username); err != nil {
		fmt.Printf("Warning: failed to mount agent configs: %v\n", err)
	}

	// Final args
	args = append(args, "agentsandbox-image", "/bin/bash")

	// Run container
	cmd := exec.Command("docker", args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to create container: %w\nOutput: %s", err, string(output))
	}

	fmt.Printf("Container %s started successfully!\n", containerName)

	// Copy agent configs from host to container automatically
	fmt.Println("\nCopying agent configurations from host to container...")
	if err := CopyAgentConfigsToContainer(containerName, agent); err != nil {
		fmt.Printf("Warning: failed to copy agent configs: %v\n", err)
		// Don't fail - this is not critical, configs might already be mounted
	}

	// Save the command for later
	agentCmd := BuildAgentCommand(currentDir, agent, false, skipPermissionFlag)
	if err := state.SaveContainerRunCommand(containerName, []string{agentCmd}); err != nil {
		fmt.Printf("Warning: failed to save container command: %v\n", err)
	}

	if attach {
		return AttachToContainer(containerName, currentDir, agent, false, skipPermissionFlag, shellMode)
	}

	return nil
}

// ResumeContainer resumes an existing container
func ResumeContainer(
	containerName string,
	agent config.Agent,
	agentContinue bool,
	skipPermissionFlag string,
	shellMode bool,
	attach bool,
) error {
	fmt.Printf("Resuming container: %s\n", containerName)

	exists, err := ContainerExists(containerName)
	if err != nil || !exists {
		return fmt.Errorf("container '%s' does not exist", containerName)
	}

	running, err := IsContainerRunning(containerName)
	if err != nil {
		return err
	}

	if !running {
		fmt.Printf("Starting stopped container: %s\n", containerName)
		cmd := exec.Command("docker", "start", containerName)
		if err := cmd.Run(); err != nil {
			return fmt.Errorf("failed to start container: %w", err)
		}
		fmt.Printf("Container %s is running\n", containerName)
	} else {
		fmt.Println("Container is already running")
	}

	// Copy agent configs from host to container automatically
	fmt.Println("\nCopying agent configurations from host to container...")
	if err := CopyAgentConfigsToContainer(containerName, agent); err != nil {
		fmt.Printf("Warning: failed to copy agent configs: %v\n", err)
		// Don't fail - this is not critical, configs might already be mounted
	}

	if attach {
		currentDir, _ := os.Getwd()
		return AttachToContainer(containerName, currentDir, agent, agentContinue, skipPermissionFlag, shellMode)
	}

	return nil
}

// BuildAgentCommand builds the command to run the agent in the container
func BuildAgentCommand(currentDir string, agent config.Agent, agentContinue bool, skipPermissionFlag string) string {
	// Always use /workspace in the container
	cmd := fmt.Sprintf("cd /workspace && export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\" && %s",
		agent.Command())

	if agentContinue {
		cmd += " --continue"
	}

	if skipPermissionFlag != "" {
		cmd += " " + skipPermissionFlag
	}

	return cmd
}

// AttachToContainer attaches to a running container
func AttachToContainer(
	containerName string,
	currentDir string,
	agent config.Agent,
	agentContinue bool,
	skipPermissionFlag string,
	shellMode bool,
) error {
	// Get the username to run as (same as host user)
	username := os.Getenv("USER")
	if username == "" {
		username = "ubuntu"
	}

	var cmd *exec.Cmd

	if shellMode {
		// Just open a shell as the mapped user (login shell for proper environment)
		cmd = exec.Command("docker", "exec", "-it", "--user", username, "-e", fmt.Sprintf("HOME=/home/%s", username), containerName, "/bin/bash", "-l")
	} else {
		// Run the agent as the mapped user (login shell for proper environment)
		agentCmd := BuildAgentCommand(currentDir, agent, agentContinue, skipPermissionFlag)
		cmd = exec.Command("docker", "exec", "-it", "--user", username, "-e", fmt.Sprintf("HOME=/home/%s", username), containerName, "/bin/bash", "-l", "-c", agentCmd)
	}

	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	return cmd.Run()
}

// mountAgentConfigs mounts agent configuration directories
func mountAgentConfigs(args *[]string, agent config.Agent, currentDir, username string) error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	// Claude config
	claudeConfig := config.GetClaudeConfigDir()
	if claudeConfig != "" {
		*args = append(*args, "-v", fmt.Sprintf("%s:/home/%s/.claude", claudeConfig, username))
		fmt.Printf("Mounting Claude config from: %s\n", claudeConfig)
	}

	// Other agent configs
	agentNames := []string{strings.ToLower(agent.Command())}
	for _, agentName := range agentNames {
		// Check project-level config first
		projectConfigPath := filepath.Join(currentDir, "."+agentName)
		if _, err := os.Stat(projectConfigPath); err == nil {
			// Mount project config to /workspace/.{agentName}
			containerPath := fmt.Sprintf("/workspace/.%s", agentName)
			*args = append(*args, "-v", fmt.Sprintf("%s:%s", projectConfigPath, containerPath))
			fmt.Printf("Mounting %s config from: %s -> %s\n", agentName, projectConfigPath, containerPath)
			continue
		}

		// Check user-level configs as fallback
		paths := []string{
			filepath.Join(homeDir, "."+agentName),
			filepath.Join(homeDir, ".config", agentName),
		}

		for _, path := range paths {
			if _, err := os.Stat(path); err == nil {
				containerPath := fmt.Sprintf("/home/%s/.%s", username, agentName)
				*args = append(*args, "-v", fmt.Sprintf("%s:%s", path, containerPath))
				fmt.Printf("Mounting %s config from: %s -> %s\n", agentName, path, containerPath)
				break
			}
		}
	}

	return nil
}

// CopyAgentConfigsToContainer copies agent configuration files from host to container
// This is useful after installing an agent from within the container shell
func CopyAgentConfigsToContainer(containerName string, agent config.Agent) error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return fmt.Errorf("failed to get home directory: %w", err)
	}

	username := os.Getenv("USER")
	if username == "" {
		username = "ubuntu"
	}

	fmt.Println("\nCopying agent configurations from host to container...")

	// Claude config
	claudeConfig := config.GetClaudeConfigDir()
	if claudeConfig != "" {
		if err := copyConfigToContainer(containerName, claudeConfig, fmt.Sprintf("/home/%s/.claude", username), username); err != nil {
			fmt.Printf("Warning: failed to copy Claude config directory: %v\n", err)
		}
	}

	// Check for .claude.json in home directory
	claudeJSON := filepath.Join(homeDir, ".claude.json")
	if _, err := os.Stat(claudeJSON); err == nil {
		if err := copyConfigToContainer(containerName, claudeJSON, fmt.Sprintf("/home/%s/.claude.json", username), username); err != nil {
			fmt.Printf("Warning: failed to copy .claude.json: %v\n", err)
		}
	}

	// Generic agent configs
	agentNames := []string{strings.ToLower(agent.Command()), "gemini", "cursor", "qwen", "codex"}
	for _, agentName := range agentNames {
		// Copy config directory
		configDir := filepath.Join(homeDir, "."+agentName)
		if _, err := os.Stat(configDir); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.%s", username, agentName)
			if err := copyConfigToContainer(containerName, configDir, containerPath, username); err != nil {
				fmt.Printf("Warning: failed to copy %s config directory: %v\n", agentName, err)
			}
		}

		// Copy config JSON file
		configJSON := filepath.Join(homeDir, "."+agentName+".json")
		if _, err := os.Stat(configJSON); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.%s.json", username, agentName)
			if err := copyConfigToContainer(containerName, configJSON, containerPath, username); err != nil {
				fmt.Printf("Warning: failed to copy .%s.json: %v\n", agentName, err)
			}
		}

		// Also check .config/{agentName}
		configPath := filepath.Join(homeDir, ".config", agentName)
		if _, err := os.Stat(configPath); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.config/%s", username, agentName)
			// Create .config directory first
			mkdirCmd := exec.Command("docker", "exec", containerName, "mkdir", "-p", fmt.Sprintf("/home/%s/.config", username))
			_ = mkdirCmd.Run()
			
			if err := copyConfigToContainer(containerName, configPath, containerPath, username); err != nil {
				fmt.Printf("Warning: failed to copy %s config from .config: %v\n", agentName, err)
			}
		}
	}

	fmt.Println("Configuration copy completed!")
	return nil
}

// copyConfigToContainer copies a file or directory from host to container using docker cp
func copyConfigToContainer(containerName, hostPath, containerPath, username string) error {
	// Get host UID/GID (these match the container user's UID/GID)
	uidCmd := exec.Command("id", "-u")
	uidOutput, err := uidCmd.Output()
	if err != nil {
		return fmt.Errorf("failed to get host UID: %w", err)
	}
	uid := strings.TrimSpace(string(uidOutput))

	gidCmd := exec.Command("id", "-g")
	gidOutput, err := gidCmd.Output()
	if err != nil {
		return fmt.Errorf("failed to get host GID: %w", err)
	}
	gid := strings.TrimSpace(string(gidOutput))

	// Copy the file/directory
	cmd := exec.Command("docker", "cp", hostPath, fmt.Sprintf("%s:%s", containerName, containerPath))
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("docker cp failed: %w\nOutput: %s", err, string(output))
	}

	fmt.Printf("✓ Copied %s to container:%s\n", hostPath, containerPath)

	// Set ownership using UID:GID (username may not exist in container, but UID/GID do)
	chownCmd := exec.Command("docker", "exec", containerName, "sudo", "chown", "-R", fmt.Sprintf("%s:%s", uid, gid), containerPath)
	chownOutput, err := chownCmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to set ownership: %w\nOutput: %s", err, string(chownOutput))
	}

	// Set permissions: u+rwX gives user read/write, and execute on directories
	chmodCmd := exec.Command("docker", "exec", containerName, "sudo", "chmod", "-R", "u+rwX", containerPath)
	chmodOutput, err := chmodCmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to set permissions: %w\nOutput: %s", err, string(chmodOutput))
	}

	return nil
}

// parseInt safely converts string to int
func parseInt(s string) int {
	var result int
	fmt.Sscanf(s, "%d", &result)
	return result
}
