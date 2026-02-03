package container

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
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
		vim \
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

func CreateDockerfile(username string, uid, gid int, languages []language.Language) (string, error) {
	tempDir := os.TempDir()
	dockerfilePath := filepath.Join(tempDir, "Dockerfile.agentsandbox")

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

	content := fmt.Sprintf(dockerfileBaseTemplate, languageSection)
	content = strings.Replace(content, "ARG USERNAME=ubuntu", fmt.Sprintf("ARG USERNAME=%s", username), 1)
	content = strings.Replace(content, "ARG USER_UID=1000", fmt.Sprintf("ARG USER_UID=%d", uid), 1)
	content = strings.Replace(content, "ARG USER_GID=1000", fmt.Sprintf("ARG USER_GID=%d", gid), 1)

	if err := os.WriteFile(dockerfilePath, []byte(content), 0o644); err != nil {
		return "", fmt.Errorf("failed to write Dockerfile: %w", err)
	}

	return dockerfilePath, nil
}

func BuildDockerImage(username string, languages []language.Language) (string, error) {
	tag := language.GenerateImageTag(languages)
	imageName := fmt.Sprintf("agentsandbox-image:%s", tag)

	checkCmd := exec.Command("docker", "image", "inspect", imageName)
	if err := checkCmd.Run(); err == nil {
		fmt.Printf("Using cached image: %s\n", imageName)
		return imageName, nil
	}

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

// validatePortMapping validates a port mapping string
// Accepts formats: PORT, HOST_PORT:CONTAINER_PORT, IP:HOST_PORT:CONTAINER_PORT
func validatePortMapping(portSpec string) error {
	if portSpec == "" {
		return fmt.Errorf("port mapping cannot be empty")
	}

	// Helper to validate port number is in valid range
	validatePort := func(portStr string, label string) error {
		port, err := strconv.Atoi(portStr)
		if err != nil {
			return fmt.Errorf("invalid %s port number '%s': %w", label, portStr, err)
		}
		if port < 1 || port > 65535 {
			return fmt.Errorf("%s port %d out of valid range (1-65535)", label, port)
		}
		return nil
	}

	// Try to detect IPv6 by checking for multiple colons
	// IPv6 addresses contain multiple colons, so we need special handling
	colonCount := strings.Count(portSpec, ":")

	// IPv6 format: [ipv6]:host:container or just ipv6 (which would have many colons)
	if strings.HasPrefix(portSpec, "[") {
		// Format: [IPv6]:host:container
		closeBracket := strings.Index(portSpec, "]")
		if closeBracket == -1 {
			return fmt.Errorf("invalid IPv6 format: missing closing bracket")
		}

		ipStr := portSpec[1:closeBracket]
		if net.ParseIP(ipStr) == nil {
			return fmt.Errorf("invalid IPv6 address '%s'", ipStr)
		}

		// Parse the port parts after the bracket
		remainder := portSpec[closeBracket+1:]
		if !strings.HasPrefix(remainder, ":") {
			return fmt.Errorf("invalid format: expected colon after IPv6 address")
		}

		portParts := strings.Split(remainder[1:], ":")
		if len(portParts) != 2 {
			return fmt.Errorf("invalid format: expected HOST:CONTAINER after IPv6 address")
		}

		if err := validatePort(portParts[0], "host"); err != nil {
			return err
		}
		return validatePort(portParts[1], "container")
	}

	parts := strings.Split(portSpec, ":")

	switch len(parts) {
	case 1:
		// Just container port (e.g., "8080")
		return validatePort(parts[0], "container")

	case 2:
		// HOST_PORT:CONTAINER_PORT (e.g., "8080:80")
		if err := validatePort(parts[0], "host"); err != nil {
			return err
		}
		return validatePort(parts[1], "container")

	case 3:
		// IP:HOST_PORT:CONTAINER_PORT (e.g., "127.0.0.1:8080:80")
		ip := net.ParseIP(parts[0])
		if ip == nil {
			return fmt.Errorf("invalid IP address '%s'", parts[0])
		}
		if err := validatePort(parts[1], "host"); err != nil {
			return err
		}
		return validatePort(parts[2], "container")

	default:
		// More than 3 colons - might be bare IPv6 (not supported by Docker without brackets)
		if colonCount > 3 {
			return fmt.Errorf("invalid port mapping format '%s': for IPv6 addresses, use format [IPv6]:HOST:CONTAINER", portSpec)
		}
		return fmt.Errorf("invalid port mapping format '%s': expected PORT, HOST:CONTAINER, or IP:HOST:CONTAINER", portSpec)
	}
}

// isPortAvailable checks if a port is available on the host
func isPortAvailable(port string) bool {
	listener, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		return false
	}
	listener.Close()
	return true
}

// findAvailablePort finds an available port on the host
// It tries to find a port by letting the OS assign one
func findAvailablePort() string {
	listener, err := net.Listen("tcp", ":0")
	if err != nil {
		return ""
	}
	defer listener.Close()

	addr := listener.Addr().(*net.TCPAddr)
	return strconv.Itoa(addr.Port)
}

func CreateContainer(
	containerName string,
	currentDir string,
	additionalDir string,
	agent config.Agent,
	skipPermissionFlag string,
	shellMode bool,
	attach bool,
	ports []string,
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

	imageName, err := BuildDockerImage(username, languages)
	if err != nil {
		return err
	}

	args := []string{
		"run", "-d", "-it",
		"--name", containerName,
		"-v", fmt.Sprintf("%s:%s", currentDir, currentDir),
	}

	// If package.json exists, create an anonymous volume for node_modules
	// This excludes the host's node_modules and creates a container-specific one
	// The volume will be removed when the container is removed
	packageJSON := filepath.Join(currentDir, "package.json")
	if _, err := os.Stat(packageJSON); err == nil {
		args = append(args, "-v", fmt.Sprintf("%s/node_modules", currentDir))
		fmt.Println("Excluding host's node_modules (container will have its own ephemeral node_modules)")
	}

	settings, _ := config.LoadSettings()
	for _, envFile := range settings.EnvFiles {
		envPath := filepath.Join(currentDir, envFile)
		if _, err := os.Stat(envPath); err == nil {
			tempFile, err := os.CreateTemp("", "env-overlay-*")
			if err == nil {
				tempFile.Close()
				containerEnvPath := filepath.Join(currentDir, envFile)
				args = append(args, "-v", fmt.Sprintf("%s:%s:ro", tempFile.Name(), containerEnvPath))
				fmt.Printf("Excluding %s from container mount\n", envFile)
			}
		}
	}

	if additionalDir != "" {
		args = append(args, "-v", fmt.Sprintf("%s:%s:ro", additionalDir, additionalDir))
		fmt.Printf("Mounting additional directory read-only: %s\n", additionalDir)
	}

	// Port mapping
	if len(ports) > 0 {
		fmt.Println("Exposing ports:")
		for _, portSpec := range ports {
			if err := validatePortMapping(portSpec); err != nil {
				return fmt.Errorf("invalid port mapping '%s': %w", portSpec, err)
			}

			// If only container port is specified, try to use the same port on host first
			finalPortSpec := portSpec
			if !strings.Contains(portSpec, ":") {
				containerPort := portSpec
				if isPortAvailable(containerPort) {
					finalPortSpec = fmt.Sprintf("%s:%s", containerPort, containerPort)
					fmt.Printf("  %s (using same port on host)\n", finalPortSpec)
				} else {
					// Find an available port
					availablePort := findAvailablePort()
					if availablePort != "" {
						finalPortSpec = fmt.Sprintf("%s:%s", availablePort, containerPort)
						fmt.Printf("  %s (port %s was occupied, using %s instead)\n", finalPortSpec, containerPort, availablePort)
					} else {
						// Fall back to Docker's automatic port assignment
						finalPortSpec = containerPort
						fmt.Printf("  %s (Docker will assign an available host port)\n", finalPortSpec)
					}
				}
			} else {
				fmt.Printf("  %s\n", finalPortSpec)
			}

			args = append(args, "-p", finalPortSpec)
		}
	}

	args = append(args, imageName, "/bin/bash")

	cmd := exec.Command("docker", args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to create container: %w\nOutput: %s", err, string(output))
	}

	fmt.Printf("Container %s started successfully!\n", containerName)

	fmt.Println("\nCopying agent configurations from host to container...")
	if err := CopyAgentConfigsToContainer(containerName, agent); err != nil {
		fmt.Printf("Warning: failed to copy agent configs: %v\n", err)
	}

	agentCmd := BuildAgentCommand(currentDir, agent, false, skipPermissionFlag)
	if err := state.SaveContainerRunCommand(containerName, []string{agentCmd}); err != nil {
		fmt.Printf("Warning: failed to save container command: %v\n", err)
	}

	if attach {
		return AttachToContainer(containerName, currentDir, agent, false, skipPermissionFlag, shellMode)
	}

	return nil
}

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

func AttachToContainer(
	containerName string,
	currentDir string,
	agent config.Agent,
	agentContinue bool,
	skipPermissionFlag string,
	shellMode bool,
) error {
	username := os.Getenv("USER")
	if username == "" {
		username = "ubuntu"
	}

	var args []string
	args = append(args,
		"exec",
		"-it",
		"--user", username,
		"-e", fmt.Sprintf("HOME=/home/%s", username),
	)

	if currentDir != "" {
		args = append(args, "-w", currentDir)
	}

	args = append(args, containerName, "/bin/bash", "-l")

	if shellMode {
		cmd := exec.Command("docker", args...)
		cmd.Stdin = os.Stdin
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		return cmd.Run()
	}

	agentCmd := BuildAgentCommand(currentDir, agent, agentContinue, skipPermissionFlag)
	args = append(args, "-c", agentCmd)

	cmd := exec.Command("docker", args...)

	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	return cmd.Run()
}

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

	sshDir := filepath.Join(homeDir, ".ssh")
	if _, err := os.Stat(sshDir); err == nil {
		containerSSHPath := fmt.Sprintf("/home/%s/.ssh", username)
		if err := copyConfigToContainer(containerName, sshDir, containerSSHPath, username); err != nil {
			fmt.Printf("Warning: failed to copy .ssh directory: %v\n", err)
		}
	}

	var agentNames []string
	if agent == config.AgentClaude {
		claudeConfig := config.GetClaudeConfigDir()
		if claudeConfig != "" {
			if err := copyConfigToContainer(containerName, claudeConfig, fmt.Sprintf("/home/%s/.claude", username), username); err != nil {
				fmt.Printf("Warning: failed to copy Claude config directory: %v\n", err)
			}
		}
		claudeJSON := filepath.Join(homeDir, ".claude.json")
		if _, err := os.Stat(claudeJSON); err == nil {
			if err := copyConfigToContainer(containerName, claudeJSON, fmt.Sprintf("/home/%s/.claude.json", username), username); err != nil {
				fmt.Printf("Warning: failed to copy .claude.json: %v\n", err)
			}
		}
		agentNames = []string{"claude"}
	} else {
		agentNames = []string{string(agent)}
	}

	for _, agentName := range agentNames {
		configDir := filepath.Join(homeDir, "."+agentName)
		if _, err := os.Stat(configDir); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.%s", username, agentName)
			if err := copyConfigToContainer(containerName, configDir, containerPath, username); err != nil {
				fmt.Printf("Warning: failed to copy %s config directory: %v\n", agentName, err)
			}
		}

		configJSON := filepath.Join(homeDir, "."+agentName+".json")
		if _, err := os.Stat(configJSON); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.%s.json", username, agentName)
			if err := copyConfigToContainer(containerName, configJSON, containerPath, username); err != nil {
				fmt.Printf("Warning: failed to copy .%s.json: %v\n", agentName, err)
			}
		}

		configPath := filepath.Join(homeDir, ".config", agentName)
		if _, err := os.Stat(configPath); err == nil {
			containerPath := fmt.Sprintf("/home/%s/.config/%s", username, agentName)
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

func copyConfigToContainer(containerName, hostPath, containerPath, username string) error {
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

	hostInfo, err := os.Stat(hostPath)
	if err != nil {
		return fmt.Errorf("failed to stat host path: %w", err)
	}

	if hostInfo.IsDir() {
		rmCmd := exec.Command("docker", "exec", containerName, "sudo", "rm", "-rf", containerPath)
		_ = rmCmd.Run()
	}

	cmd := exec.Command("docker", "cp", hostPath, fmt.Sprintf("%s:%s", containerName, containerPath))
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("docker cp failed: %w\nOutput: %s", err, string(output))
	}

	fmt.Printf("✓ Copied %s to container:%s\n", hostPath, containerPath)

	chownCmd := exec.Command("docker", "exec", containerName, "sudo", "chown", "-R", fmt.Sprintf("%s:%s", uid, gid), containerPath)
	chownOutput, err := chownCmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to set ownership: %w\nOutput: %s", err, string(chownOutput))
	}

	if filepath.Base(hostPath) == ".ssh" || strings.HasSuffix(containerPath, ".ssh") {
		chmodDirCmd := exec.Command("docker", "exec", containerName, "sudo", "chmod", "700", containerPath)
		chmodDirOutput, err := chmodDirCmd.CombinedOutput()
		if err != nil {
			return fmt.Errorf("failed to set .ssh directory permissions: %w\nOutput: %s", err, string(chmodDirOutput))
		}

		chmodFilesCmd := exec.Command("docker", "exec", containerName, "sudo", "find", containerPath, "-type", "f", "-exec", "chmod", "600", "{}", ";")
		chmodFilesOutput, err := chmodFilesCmd.CombinedOutput()
		if err != nil {
			return fmt.Errorf("failed to set .ssh file permissions: %w\nOutput: %s", err, string(chmodFilesOutput))
		}

		fmt.Printf("✓ Set strict SSH permissions (700 for directory, 600 for files)\n")
	} else {
		chmodCmd := exec.Command("docker", "exec", containerName, "sudo", "chmod", "-R", "u+rwX", containerPath)
		chmodOutput, err := chmodCmd.CombinedOutput()
		if err != nil {
			return fmt.Errorf("failed to set permissions: %w\nOutput: %s", err, string(chmodOutput))
		}
	}

	return nil
}

func parseInt(s string) int {
	var result int
	fmt.Sscanf(s, "%d", &result)
	return result
}
