package cli

import (
	"github.com/spf13/cobra"
)

var startCmd = &cobra.Command{
	Use:   "start",
	Short: "Start a new agent sandbox container (alias for default behavior)",
	RunE:  runStart,
}

