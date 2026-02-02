package cli

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	"github.com/spf13/cobra"
	"github.com/thaodangspace/agentsandbox/internal/logs"
	"github.com/thaodangspace/agentsandbox/internal/state"
)

var (
	logsCmd = &cobra.Command{
		Use:   "logs",
		Short: "Manage session logs",
	}

	logsListCmd = &cobra.Command{
		Use:   "list",
		Short: "List all session logs",
		RunE:  runLogsList,
	}

	logsViewCmd = &cobra.Command{
		Use:   "view <log-file>",
		Short: "View a session log as HTML",
		Args:  cobra.ExactArgs(1),
		RunE:  runLogsView,
	}

	logsCleanCmd = &cobra.Command{
		Use:   "clean",
		Short: "Clean up old session logs",
		RunE:  runLogsClean,
	}

	// Log flags
	containerFilter string
	outputPath      string
	openBrowser     bool
	daysOld         int
)

func init() {
	logsListCmd.Flags().StringVar(&containerFilter, "container", "", "Filter by container name")
	logsViewCmd.Flags().StringVar(&outputPath, "output", "", "Output HTML file path (default: same as log with .html extension)")
	logsViewCmd.Flags().BoolVar(&openBrowser, "open", false, "Open in browser after generating")
	logsCleanCmd.Flags().IntVar(&daysOld, "days", 30, "Keep logs newer than this many days")
	logsCleanCmd.Flags().StringVar(&containerFilter, "container", "", "Filter by container name")

	logsCmd.AddCommand(logsListCmd)
	logsCmd.AddCommand(logsViewCmd)
	logsCmd.AddCommand(logsCleanCmd)
}

func runLogsList(cmd *cobra.Command, args []string) error {
	currentDir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	var containers []string
	if containerFilter != "" {
		containers = []string{containerFilter}
	} else {
		containers, err = state.ListContainersWithLogs(currentDir)
		if err != nil {
			return fmt.Errorf("failed to list containers: %w", err)
		}
	}

	if len(containers) == 0 {
		fmt.Println("No session logs found.")
		return nil
	}

	for _, containerName := range containers {
		fmt.Printf("\nContainer: %s\n", containerName)
		logFiles, err := state.ListSessionLogs(containerName, currentDir)
		if err != nil {
			fmt.Printf("  Error listing logs: %v\n", err)
			continue
		}

		if len(logFiles) == 0 {
			fmt.Println("  No logs found")
		} else {
			for _, logFile := range logFiles {
				fmt.Printf("  %s\n", logFile)
			}
		}
	}

	return nil
}

func runLogsView(cmd *cobra.Command, args []string) error {
	logFile := args[0]

	// Parse log file
	events, err := logs.ParseRawLog(logFile)
	if err != nil {
		return fmt.Errorf("failed to parse log file: %w", err)
	}

	// Determine output path
	output := outputPath
	if output == "" {
		output = logFile[:len(logFile)-len(filepath.Ext(logFile))] + ".html"
	}

	// Generate HTML
	title := filepath.Base(logFile)
	if err := logs.WriteHTML(events, output, title); err != nil {
		return fmt.Errorf("failed to generate HTML: %w", err)
	}

	fmt.Printf("HTML log generated: %s\n", output)

	// Open in browser if requested
	if openBrowser {
		if err := openInBrowser(output); err != nil {
			fmt.Printf("Failed to open browser: %v\n", err)
		} else {
			fmt.Println("Opened in browser")
		}
	}

	return nil
}

func runLogsClean(cmd *cobra.Command, args []string) error {
	currentDir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	var containers []string
	if containerFilter != "" {
		containers = []string{containerFilter}
	} else {
		containers, err = state.ListContainersWithLogs(currentDir)
		if err != nil {
			return fmt.Errorf("failed to list containers: %w", err)
		}
	}

	if len(containers) == 0 {
		fmt.Println("No containers with logs found.")
		return nil
	}

	totalDeleted := 0
	for _, containerName := range containers {
		deleted, err := state.CleanupOldLogs(containerName, currentDir, daysOld)
		if err != nil {
			fmt.Printf("Warning: Failed to cleanup logs for %s: %v\n", containerName, err)
			continue
		}

		if deleted > 0 {
			fmt.Printf("Deleted %d old log files from container %s\n", deleted, containerName)
			totalDeleted += deleted
		}
	}

	if totalDeleted == 0 {
		fmt.Printf("No logs older than %d days found.\n", daysOld)
	} else {
		fmt.Printf("Total deleted: %d files\n", totalDeleted)
	}

	return nil
}

func openInBrowser(filepath string) error {
	var cmd *exec.Cmd

	switch runtime.GOOS {
	case "linux":
		cmd = exec.Command("xdg-open", filepath)
	case "darwin":
		cmd = exec.Command("open", filepath)
	case "windows":
		cmd = exec.Command("cmd", "/c", "start", filepath)
	default:
		return fmt.Errorf("unsupported platform")
	}

	return cmd.Start()
}

