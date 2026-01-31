package cli

import (
	"fmt"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/container"
)

var copyConfigCmd = &cobra.Command{
	Use:   "copy-config [container-name]",
	Short: "Copy agent configuration files from host to container",
	Long: `Copy agent configuration files (like .claude/, .claude.json, etc.) 
from the host to the specified container. This is useful after installing 
an agent from within the container shell.`,
	Args: cobra.ExactArgs(1),
	RunE: runCopyConfig,
}

func init() {
	rootCmd.AddCommand(copyConfigCmd)
}

func runCopyConfig(cmd *cobra.Command, args []string) error {
	containerName := args[0]

	// Verify container exists
	exists, err := container.ContainerExists(containerName)
	if err != nil {
		return fmt.Errorf("failed to check if container exists: %w", err)
	}
	if !exists {
		return fmt.Errorf("container '%s' does not exist", containerName)
	}

	// Use the agent specified in the flag
	agent, err := config.ValidateAgent(agentName)
	if err != nil {
		return err
	}

	// Copy configs
	if err := container.CopyAgentConfigsToContainer(containerName, agent); err != nil {
		return fmt.Errorf("failed to copy configs: %w", err)
	}

	fmt.Println("\n✅ Successfully copied agent configurations to container!")
	return nil
}
