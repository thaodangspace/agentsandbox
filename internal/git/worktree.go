package git

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// CreateWorktree creates a git worktree for the specified branch
func CreateWorktree(baseDir, branch string) (string, error) {
	// Get the git repository root
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	cmd.Dir = baseDir
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not a git repository: %w", err)
	}

	root := strings.TrimSpace(string(output))

	// Create worktrees directory
	worktreesDir := filepath.Join(root, ".agentsandbox-worktrees")
	if err := os.MkdirAll(worktreesDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create worktrees directory: %w", err)
	}

	worktreePath := filepath.Join(worktreesDir, branch)

	// Check if worktree already exists
	if _, err := os.Stat(worktreePath); err == nil {
		return worktreePath, nil
	}

	// Check if branch exists
	branchCmd := exec.Command("git", "rev-parse", "--verify", branch)
	branchCmd.Dir = root
	branchExists := branchCmd.Run() == nil

	// Create worktree
	var wtCmd *exec.Cmd
	if branchExists {
		// Branch exists, checkout existing branch
		wtCmd = exec.Command("git", "worktree", "add", "--force", worktreePath, branch)
	} else {
		// Create new branch
		wtCmd = exec.Command("git", "worktree", "add", "--force", "-b", branch, worktreePath)
	}

	wtCmd.Dir = root
	wtCmd.Stdout = os.Stdout
	wtCmd.Stderr = os.Stderr

	if err := wtCmd.Run(); err != nil {
		return "", fmt.Errorf("git worktree add failed: %w", err)
	}

	return worktreePath, nil
}

