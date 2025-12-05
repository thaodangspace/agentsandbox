package cli

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/container"
	"github.com/thaodangspace/agentsandbox/internal/git"
)

var (
	// Global flags
	agentName      string
	continueFlag   bool
	addDir         string
	worktree       string
	shellMode      bool
	noClipboard    bool

	// Root command
	rootCmd = &cobra.Command{
		Use:   "agentsandbox",
		Short: "Agent Sandbox - Docker container manager for AI development agents",
		Long: `Agent Sandbox creates isolated Docker containers with AI development agents.
Compatible with Claude, Gemini, Codex, Qwen, and Cursor development agents.`,
		Version: "0.2.0",
		RunE:    runStart,
	}
)

func init() {
	rootCmd.PersistentFlags().StringVar(&agentName, "agent", "claude", "Agent to start in the container (claude, gemini, codex, qwen, cursor)")
	rootCmd.Flags().BoolVar(&continueFlag, "continue", false, "Resume the last created container")
	rootCmd.Flags().StringVar(&addDir, "add-dir", "", "Additional directory to mount read-only inside the container")
	rootCmd.Flags().StringVar(&worktree, "worktree", "", "Create and use a git worktree for the specified branch")
	rootCmd.Flags().BoolVar(&shellMode, "shell", false, "Attach to container shell without starting the agent")
	rootCmd.Flags().BoolVar(&noClipboard, "no-clipboard", false, "Disable clipboard image sharing between host and container")

	// Add subcommands
	rootCmd.AddCommand(listCmd)
	rootCmd.AddCommand(listAllCmd)
	rootCmd.AddCommand(cleanupCmd)
	rootCmd.AddCommand(logsCmd)
	rootCmd.AddCommand(startCmd)
	rootCmd.AddCommand(attachCmd)
}

// Execute runs the root command
func Execute() error {
	return rootCmd.Execute()
}

// runStart is the default action (start a new container)
func runStart(cmd *cobra.Command, args []string) error {
	// Validate agent
	agent, err := config.ValidateAgent(agentName)
	if err != nil {
		return err
	}

	// Get current directory
	currentDir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	// Handle worktree
	if worktree != "" {
		worktreePath, err := git.CreateWorktree(currentDir, worktree)
		if err != nil {
			return fmt.Errorf("failed to create worktree for branch %s: %w", worktree, err)
		}
		currentDir = worktreePath
		if err := os.Chdir(currentDir); err != nil {
			return fmt.Errorf("failed to change directory to worktree: %w", err)
		}
	}

	// Load settings
	settings, err := config.LoadSettings()
	if err != nil {
		fmt.Printf("Warning: failed to load settings: %v\n", err)
		settings = config.DefaultSettings()
	}

	// Check Docker availability
	if err := container.CheckDockerAvailability(); err != nil {
		return err
	}

	// Auto-remove old containers
	if err := container.AutoRemoveOldContainers(settings.AutoRemoveMinutes); err != nil {
		fmt.Printf("Warning: failed to auto-remove old containers: %v\n", err)
	}

	// Get skip permission flag
	skipPermissionFlag := settings.SkipPermissionFlags[agentName]

	// Handle continue flag
	if continueFlag {
		return handleContinue(agent, skipPermissionFlag)
	}

	// Check for existing container
	existing, err := container.FindExistingContainer(currentDir, agent)
	if err != nil {
		fmt.Printf("Warning: failed to check for existing container: %v\n", err)
	}

	if existing != "" {
		fmt.Printf("Found existing container: %s\n", existing)
		fmt.Println("Attaching to existing container instead of creating a new one...")
		return container.ResumeContainer(existing, agent, false, skipPermissionFlag, shellMode, true)
	}

	// Generate container name
	containerName := container.GenerateContainerName(currentDir, agent)

	fmt.Printf("Starting %s Agent Sandbox container: %s\n", agent.DisplayName(), containerName)
	fmt.Printf("Container %s started successfully!\n", containerName)
	fmt.Printf("To attach to the container manually, run: docker exec -it %s /bin/bash\n", containerName)

	// Create and start the container
	if err := container.CreateContainer(containerName, currentDir, addDir, agent, skipPermissionFlag, shellMode, true); err != nil {
		return fmt.Errorf("failed to create container: %w", err)
	}

	return nil
}

// handleContinue handles the --continue flag
func handleContinue(agent config.Agent, skipPermissionFlag string) error {
	containerName, err := container.LoadLastContainer()
	if err != nil {
		return fmt.Errorf("failed to load last container: %w", err)
	}

	if containerName == "" {
		return fmt.Errorf("no previous container found. Run without --continue to create a new container")
	}

	// Try to extract agent from container name
	if extractedAgent, ok := config.FromContainerName(containerName); ok {
		agent = extractedAgent
	}

	return container.ResumeContainer(containerName, agent, true, skipPermissionFlag, shellMode, true)
}

