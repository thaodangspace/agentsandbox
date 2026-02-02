package state

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"
)

// GetStateDir returns the state directory path
func GetStateDir() (string, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	stateDir := filepath.Join(homeDir, ".config", "agentsandbox")
	if err := os.MkdirAll(stateDir, 0755); err != nil {
		return "", err
	}
	return stateDir, nil
}

// SaveLastContainer saves the name of the last used container
func SaveLastContainer(name string) error {
	stateDir, err := GetStateDir()
	if err != nil {
		return err
	}

	lastFile := filepath.Join(stateDir, "last_container")
	return os.WriteFile(lastFile, []byte(name), 0644)
}

// LoadLastContainer loads the name of the last used container
func LoadLastContainer() (string, error) {
	stateDir, err := GetStateDir()
	if err != nil {
		return "", err
	}

	lastFile := filepath.Join(stateDir, "last_container")
	data, err := os.ReadFile(lastFile)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}

	return string(data), nil
}

// ClearLastContainer clears the last container state
func ClearLastContainer() error {
	stateDir, err := GetStateDir()
	if err != nil {
		return err
	}

	lastFile := filepath.Join(stateDir, "last_container")
	if err := os.Remove(lastFile); err != nil && !os.IsNotExist(err) {
		return err
	}

	return nil
}

// GetLogsDir returns the logs directory for a specific container
func GetLogsDir(containerName, currentDir string) (string, error) {
	stateDir, err := GetStateDir()
	if err != nil {
		return "", err
	}

	// Create a hash or sanitized version of the current directory
	dirHash := filepath.Base(currentDir)
	logsDir := filepath.Join(stateDir, "logs", dirHash, containerName)

	if err := os.MkdirAll(logsDir, 0755); err != nil {
		return "", err
	}

	return logsDir, nil
}

// PrepareSessionLog creates a new session log file
func PrepareSessionLog(containerName, currentDir string) (string, error) {
	logsDir, err := GetLogsDir(containerName, currentDir)
	if err != nil {
		return "", err
	}

	timestamp := time.Now().Format("20060102-150405")
	logFile := filepath.Join(logsDir, fmt.Sprintf("session-%s.jsonl", timestamp))

	// Create empty file
	f, err := os.Create(logFile)
	if err != nil {
		return "", err
	}
	defer f.Close()

	return logFile, nil
}

// ListContainersWithLogs returns a list of containers that have logs
func ListContainersWithLogs(currentDir string) ([]string, error) {
	stateDir, err := GetStateDir()
	if err != nil {
		return nil, err
	}

	dirHash := filepath.Base(currentDir)
	logsDir := filepath.Join(stateDir, "logs", dirHash)

	if _, err := os.Stat(logsDir); os.IsNotExist(err) {
		return []string{}, nil
	}

	entries, err := os.ReadDir(logsDir)
	if err != nil {
		return nil, err
	}

	var containers []string
	for _, entry := range entries {
		if entry.IsDir() {
			containers = append(containers, entry.Name())
		}
	}

	return containers, nil
}

// ListSessionLogs lists all session logs for a container
func ListSessionLogs(containerName, currentDir string) ([]string, error) {
	logsDir, err := GetLogsDir(containerName, currentDir)
	if err != nil {
		return nil, err
	}

	if _, err := os.Stat(logsDir); os.IsNotExist(err) {
		return []string{}, nil
	}

	entries, err := os.ReadDir(logsDir)
	if err != nil {
		return nil, err
	}

	var logs []string
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".jsonl" {
			logs = append(logs, filepath.Join(logsDir, entry.Name()))
		}
	}

	return logs, nil
}

// CleanupOldLogs removes log files older than the specified number of days
func CleanupOldLogs(containerName, currentDir string, days int) (int, error) {
	logsDir, err := GetLogsDir(containerName, currentDir)
	if err != nil {
		return 0, err
	}

	if _, err := os.Stat(logsDir); os.IsNotExist(err) {
		return 0, nil
	}

	entries, err := os.ReadDir(logsDir)
	if err != nil {
		return 0, err
	}

	cutoff := time.Now().AddDate(0, 0, -days)
	deleted := 0

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		info, err := entry.Info()
		if err != nil {
			continue
		}

		if info.ModTime().Before(cutoff) {
			logPath := filepath.Join(logsDir, entry.Name())
			if err := os.Remove(logPath); err == nil {
				deleted++
			}
		}
	}

	return deleted, nil
}

// ContainerRunCommand stores information about how a container was started
type ContainerRunCommand struct {
	Command   []string  `json:"command"`
	Timestamp time.Time `json:"timestamp"`
}

// SaveContainerRunCommand saves the command used to start a container
func SaveContainerRunCommand(containerName string, command []string) error {
	stateDir, err := GetStateDir()
	if err != nil {
		return err
	}

	commandFile := filepath.Join(stateDir, fmt.Sprintf("%s.command.json", containerName))
	cmd := ContainerRunCommand{
		Command:   command,
		Timestamp: time.Now(),
	}

	data, err := json.MarshalIndent(cmd, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(commandFile, data, 0644)
}

// LoadContainerRunCommand loads the command used to start a container
func LoadContainerRunCommand(containerName string) (*ContainerRunCommand, error) {
	stateDir, err := GetStateDir()
	if err != nil {
		return nil, err
	}

	commandFile := filepath.Join(stateDir, fmt.Sprintf("%s.command.json", containerName))
	data, err := os.ReadFile(commandFile)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var cmd ContainerRunCommand
	if err := json.Unmarshal(data, &cmd); err != nil {
		return nil, err
	}

	return &cmd, nil
}


