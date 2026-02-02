package container

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/language"
	"github.com/thaodangspace/agentsandbox/internal/state"
)

const dockerfileBaseTemplate = `FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install basic tools
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    sudo \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Language toolchains (inserted dynamically)
%s

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

# Add Go to PATH if installed
ENV PATH="/usr/local/go/bin:${PATH}"

# Install Claude (example - adjust per actual installation method)
RUN curl -fsSL https://claude.ai/install.sh | bash || true

CMD ["/bin/bash"]
`

// CreateDockerfile creates a Dockerfile with the specified user info and language toolchains
func CreateDockerfile(username string, uid, gid int, languages []language.Language) (string, error) {
	tempDir := os.TempDir()
	dockerfilePath := filepath.Join(tempDir, "Dockerfile.agentsandbox")

	// Generate language install commands
	var languageInstalls []string
	for _, lang := range languages {
		cmd := lang.DockerfileInstallCmd()
		if cmd != "" {
			languageInstalls = append(languageInstalls, cmd)
		}
	}
	languageSection := strings.Join(languageInstalls, "\n\n")
	if languageSection == "" {
		languageSection = "# No language toolchains detected"
	}

	// Generate Dockerfile content
	content := fmt.Sprintf(dockerfileBaseTemplate, languageSection)
	content = strings.Replace(content, "ARG USERNAME=ubuntu", fmt.Sprintf("ARG USERNAME=%s", username), 1)
	content = strings.Replace(content, "ARG USER_UID=1000", fmt.Sprintf("ARG USER_UID=%d", uid), 1)
	content = strings.Replace(content, "ARG USER_GID=1000", fmt.Sprintf("ARG USER_GID=%d", gid), 1)

	if err := os.WriteFile(dockerfilePath, []byte(content), 0o644); err != nil {
		return "", fmt.Errorf("failed to write Dockerfile: %w", err)
	}

	return dockerfilePath, nil
}

// BuildDockerImage builds the agentsandbox Docker image with detected language toolchains
// Returns the image name (including tag) for use in container creation
func BuildDockerImage(username string, languages []language.Language) (string, error) {
	// Generate image tag based on detected languages
	tag := language.GenerateImageTag(languages)
	imageName := fmt.Sprintf("agentsandbox-image:%s", tag)

	// Check if image already exists (cache hit)
	checkCmd := exec.Command("docker", "image", "inspect", imageName)
	if err := checkCmd.Run(); err == nil {
		fmt.Printf("Using cached image: %s\n", imageName)
		return imageName, nil
	}

	// Get host UID/GID
	uidCmd := exec.Command("id", "-u")
	uidOutput, err := uidCmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to get host UID: %w", err)
	}

	gidCmd := exec.Command("id", "-g")
	gidOutput, err := gidCmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to get host GID: %w", err)
	}

	uid := strings.TrimSpace(string(uidOutput))
	gid := strings.TrimSpace(string(gidOutput))

	// Create Dockerfile with language toolchains
	dockerfilePath, err := CreateDockerfile(username,
		parseInt(uid), parseInt(gid), languages)
	if err != nil {
		return "", err
	}
	defer os.Remove(dockerfilePath)

	fmt.Printf("Building Docker image: %s\n", imageName)
	if len(languages) > 0 {
		names := make([]string, len(languages))
		for i, l := range languages {
			names[i] = l.Name()
		}
		fmt.Printf("Including toolchains: %s\n", strings.Join(names, ", "))
	}

	cmd := exec.Command("docker", "build",
		"-t", imageName,
		"--build-arg", fmt.Sprintf("USERNAME=%s", username),
		"--build-arg", fmt.Sprintf("USER_UID=%s", uid),
		"--build-arg", fmt.Sprintf("USER_GID=%s", gid),
		"-f", dockerfilePath,
		".")

	cmd.Dir = filepath.Dir(dockerfilePath)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("Docker build failed: %w", err)
	}

	fmt.Println("Docker image built successfully")
	return imageName, nil
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

	// Detect project languages
	languages := language.DetectProjectLanguages(currentDir)
	if len(languages) > 0 {
		names := make([]string, len(languages))
		for i, l := range languages {
			names[i] = l.Name()
		}
		fmt.Printf("Detected languages: %s\n", strings.Join(names, ", "))
	}

	// Build the image (cached if same languages detected before)
	imageName, err := BuildDockerImage(username, languages)
	if err != nil {
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
		args = append(args, "-v", fmt.Sprintf("%s/node_modules", currentDir))
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
				containerEnvPath := filepath.Join(currentDir, envFile)
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


	// Final args
	args = append(args, imageName, "/bin/bash")

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

	if attach {
		currentDir, _ := os.Getwd()
		return AttachToContainer(containerName, currentDir, agent, agentContinue, skipPermissionFlag, shellMode)
	}

	return nil
}

// BuildAgentCommand builds the command to run the agent in the container
func BuildAgentCommand(currentDir string, agent config.Agent, agentContinue bool, skipPermissionFlag string) string {
	// Use host path in the container
	cmd := fmt.Sprintf("cd %s && export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\" && %s",
		currentDir, agent.Command())

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

	// Determine which configs to copy based on the agent
	var agentNames []string
	if agent == config.AgentClaude {
		// Special handling for Claude
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
		agentNames = []string{"claude"}
	} else {
		// Generic agents
		agentNames = []string{string(agent)}
	}

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

	// Check if source is a directory
	hostInfo, err := os.Stat(hostPath)
	if err != nil {
		return fmt.Errorf("failed to stat host path: %w", err)
	}

	// If source is a directory, remove existing destination in container first
	// This prevents docker cp from creating nested directories (e.g., .claude/.claude/)
	if hostInfo.IsDir() {
		rmCmd := exec.Command("docker", "exec", containerName, "sudo", "rm", "-rf", containerPath)
		_ = rmCmd.Run() // Ignore error if directory doesn't exist
	}

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
