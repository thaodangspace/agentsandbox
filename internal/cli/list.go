package cli

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/config"
	"github.com/thaodangspace/agentsandbox/internal/container"
)

var (
	listCmd = &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List containers for this directory and optionally attach",
		RunE:    runList,
	}

	listAllCmd = &cobra.Command{
		Use:     "list-all",
		Aliases: []string{"ps"},
		Short:   "List all running Agent Sandbox containers and optionally attach",
		RunE:    runListAll,
	}
)

func runList(cmd *cobra.Command, args []string) error {
	currentDir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	containers, err := container.ListContainers(currentDir)
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	if len(containers) == 0 {
		fmt.Printf("No Agent Sandbox containers found for directory %s\n", currentDir)

		// Show global containers
		global, _ := container.ListAllContainers()
		if len(global) > 0 {
			fmt.Println("\nCurrently running containers:")
			fmt.Printf("%-20s %s\n", "Project", "Container")
			fmt.Println(strings.Repeat("-", 80))
			for _, info := range global {
				fmt.Printf("%-20s %s\n", info.Project, info.Name)
			}
		} else {
			fmt.Println("No running Agent Sandbox containers found.")
		}
		return nil
	}

	// Display table
	fmt.Printf("\n%-5s %s\n", "No.", "Container")
	fmt.Println(strings.Repeat("-", 80))
	for i, name := range containers {
		fmt.Printf("%-5d %s\n", i+1, name)
	}

	// Prompt for selection
	fmt.Print("Select a container to attach (number, or press Enter to cancel): ")
	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	input = strings.TrimSpace(input)

	if input == "" {
		return nil
	}

	num, err := strconv.Atoi(input)
	if err != nil || num < 1 || num > len(containers) {
		fmt.Println("Invalid selection")
		return nil
	}

	// Prompt for attach mode
	fmt.Print("Choose attach mode:\n  1) Attach with agent\n  2) Attach to shell only\nEnter choice: ")
	modeInput, _ := reader.ReadString('\n')
	modeInput = strings.TrimSpace(modeInput)

	shellMode := false
	switch modeInput {
	case "1":
		shellMode = false
	case "2":
		shellMode = true
	default:
		fmt.Println("Invalid choice")
		return nil
	}

	selected := containers[num-1]
	agent, ok := config.FromContainerName(selected)
	if !ok {
		agent = config.AgentClaude
	}

	settings, _ := config.LoadSettings()
	skipPermissionFlag := settings.SkipPermissionFlags[string(agent)]

	return container.ResumeContainer(selected, agent, false, skipPermissionFlag, shellMode, true)
}

func runListAll(cmd *cobra.Command, args []string) error {
	containers, err := container.ListAllContainers()
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	if len(containers) == 0 {
		fmt.Println("No running Agent Sandbox containers found.")
		return nil
	}

	// Display table
	fmt.Printf("\n%-5s %-20s %-40s %s\n", "No.", "Project", "Container", "Directory")
	fmt.Println(strings.Repeat("-", 120))
	for i, info := range containers {
		fmt.Printf("%-5d %-20s %-40s %s\n", i+1, info.Project, info.Name, info.Directory)
	}

	// Prompt for selection
	fmt.Print("Select a container to attach (number, or press Enter to cancel): ")
	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	input = strings.TrimSpace(input)

	if input == "" {
		return nil
	}

	num, err := strconv.Atoi(input)
	if err != nil || num < 1 || num > len(containers) {
		fmt.Println("Invalid selection")
		return nil
	}

	// Prompt for attach mode
	fmt.Print("Choose attach mode:\n  1) Attach with agent\n  2) Attach to shell only\nEnter choice: ")
	modeInput, _ := reader.ReadString('\n')
	modeInput = strings.TrimSpace(modeInput)

	shellMode := false
	switch modeInput {
	case "1":
		shellMode = false
	case "2":
		shellMode = true
	default:
		fmt.Println("Invalid choice")
		return nil
	}

	selected := containers[num-1]

	// Change to the container's directory if available
	if selected.Directory != "" {
		if err := os.Chdir(selected.Directory); err != nil {
			fmt.Printf("Warning: failed to change directory to %s: %v\n", selected.Directory, err)
		}
	}

	agent, ok := config.FromContainerName(selected.Name)
	if !ok {
		agent = config.AgentClaude
	}

	settings, _ := config.LoadSettings()
	skipPermissionFlag := settings.SkipPermissionFlags[string(agent)]

	return container.ResumeContainer(selected.Name, agent, false, skipPermissionFlag, shellMode, true)
}

