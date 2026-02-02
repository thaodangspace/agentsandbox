package cli

import (
	"fmt"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/container"
)

var attachCmd = &cobra.Command{
	Use:   "attach [container]",
	Short: "Attach to an existing container",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runAttach,
}

func runAttach(cmd *cobra.Command, args []string) error {
	var containerName string

	if len(args) > 0 {
		containerName = args[0]
	} else {
		// Load last container
		lastContainer, err := container.LoadLastContainer()
		if err != nil || lastContainer == "" {
			return fmt.Errorf("no container specified and no previous container found")
		}
		containerName = lastContainer
	}

	// Extract agent from container name
	agent, ok := config.FromContainerName(containerName)
	if !ok {
		agent = config.AgentClaude
	}

	// Load settings
	settings, _ := config.LoadSettings()
	skipPermissionFlag := settings.SkipPermissionFlags[string(agent)]

	return container.ResumeContainer(containerName, agent, false, skipPermissionFlag, shellMode, true)
}

