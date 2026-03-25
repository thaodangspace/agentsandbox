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
