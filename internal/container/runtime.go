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

RUN if [ "$USER_UID" != "1000" ] || [ "$USER_GID" != "1000" ]; then \
        groupmod -g $USER_GID ubuntu || true && \
        usermod -u $USER_UID -g $USER_GID ubuntu || true; \
    fi

RUN echo "ubuntu ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Install AI agents (with cache busting args)
ARG CLAUDE_CACHE_BUST
ARG GEMINI_CACHE_BUST
ARG CODEX_CACHE_BUST
ARG QWEN_CACHE_BUST
ARG CURSOR_CACHE_BUST

USER ubuntu
WORKDIR /home/ubuntu

# Install Claude (example - adjust per actual installation method)
RUN curl -fsSL https://claude.ai/download/linux | bash || true

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

	if err := os.WriteFile(dockerfilePath, []byte(content), 0644); err != nil {
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
		"-v", fmt.Sprintf("%s:%s", currentDir, currentDir),
	}

	// Handle node_modules isolation for Node.js projects
	packageJSON := filepath.Join(currentDir, "package.json")
	if _, err := os.Stat(packageJSON); err == nil {
		nodeModules := filepath.Join(currentDir, "node_modules")
		args = append(args, "-v", nodeModules)
		fmt.Printf("Isolating node_modules with container volume: %s\n", nodeModules)
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
				args = append(args, "-v", fmt.Sprintf("%s:%s:ro", tempFile.Name(), envPath))
				fmt.Printf("Excluding %s from container mount\n", envPath)
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

	if attach {
		currentDir, _ := os.Getwd()
		return AttachToContainer(containerName, currentDir, agent, agentContinue, skipPermissionFlag, shellMode)
	}

	return nil
}

// BuildAgentCommand builds the command to run the agent in the container
func BuildAgentCommand(currentDir string, agent config.Agent, agentContinue bool, skipPermissionFlag string) string {
	escaped := strings.ReplaceAll(currentDir, "'", "'\\''")
	cmd := fmt.Sprintf("cd '%s' && export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\" && %s",
		escaped, agent.Command())

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
	var cmd *exec.Cmd

	if shellMode {
		// Just open a shell
		cmd = exec.Command("docker", "exec", "-it", containerName, "/bin/bash")
	} else {
		// Run the agent
		agentCmd := BuildAgentCommand(currentDir, agent, agentContinue, skipPermissionFlag)
		cmd = exec.Command("docker", "exec", "-it", containerName, "/bin/bash", "-c", agentCmd)
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
		paths := []string{
			filepath.Join(currentDir, "."+agentName),
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

// parseInt safely converts string to int
func parseInt(s string) int {
	var result int
	fmt.Sscanf(s, "%d", &result)
	return result
}

