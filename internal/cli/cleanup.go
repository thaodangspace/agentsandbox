package cli

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/container"
	"github.com/thaodangspace/agentsandbox/internal/state"
)

var cleanupCmd = &cobra.Command{
	Use:   "cleanup",
	Short: "Remove all containers created from this directory",
	RunE:  runCleanup,
}

func runCleanup(cmd *cobra.Command, args []string) error {
	currentDir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	if err := container.CleanupContainers(currentDir); err != nil {
		return fmt.Errorf("failed to cleanup containers: %w", err)
	}

	if err := state.ClearLastContainer(); err != nil {
		fmt.Printf("Warning: failed to clear last container state: %v\n", err)
	}

	fmt.Printf("Removed all Agent Sandbox containers for directory %s\n", currentDir)
	return nil
}

