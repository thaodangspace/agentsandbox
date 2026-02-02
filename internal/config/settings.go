package config

import (
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/spf13/viper"
)

// Settings represents the application settings
type Settings struct {
	SkipPermissionFlags  map[string]string `json:"skip_permission_flags" mapstructure:"skip_permission_flags"`
	EnvFiles             []string          `json:"env_files" mapstructure:"env_files"`
}

// DefaultSettings returns the default settings
func DefaultSettings() *Settings {
	return &Settings{
		SkipPermissionFlags: map[string]string{
			"claude": "--dangerously-skip-permissions",
			"gemini": "--yolo",
			"qwen":   "--yolo",
			"codex":  "--yolo",
			"cursor": "--yolo",
		},
		EnvFiles: []string{
			".env",
			".env.local",
			".env.development.local",
			".env.test.local",
			".env.production.local",
		},
	}
}

// LoadSettings loads settings from the configuration file
func LoadSettings() (*Settings, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return DefaultSettings(), nil
	}

	configDir := filepath.Join(homeDir, ".config", "agentsandbox")
	configFile := filepath.Join(configDir, "settings.json")

	// Check if config file exists
	if _, err := os.Stat(configFile); os.IsNotExist(err) {
		return DefaultSettings(), nil
	}

	// Read the config file
	data, err := os.ReadFile(configFile)
	if err != nil {
		return DefaultSettings(), nil
	}

	settings := DefaultSettings()
	if err := json.Unmarshal(data, settings); err != nil {
		return DefaultSettings(), nil
	}

	return settings, nil
}

// Save saves the settings to the configuration file
func (s *Settings) Save() error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	configDir := filepath.Join(homeDir, ".config", "agentsandbox")
	if err := os.MkdirAll(configDir, 0755); err != nil {
		return err
	}

	configFile := filepath.Join(configDir, "settings.json")
	data, err := json.MarshalIndent(s, "", "    ")
	if err != nil {
		return err
	}

	return os.WriteFile(configFile, data, 0644)
}

// GetConfigDir returns the application configuration directory
func GetConfigDir() (string, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(homeDir, ".config", "agentsandbox"), nil
}

// GetClaudeConfigDir finds the Claude configuration directory
func GetClaudeConfigDir() string {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return ""
	}

	// Try XDG_CONFIG_HOME first
	if xdgConfig := os.Getenv("XDG_CONFIG_HOME"); xdgConfig != "" {
		claudeDir := filepath.Join(xdgConfig, "claude")
		if _, err := os.Stat(claudeDir); err == nil {
			return claudeDir
		}
	}

	// Try ~/.config/claude
	configDir := filepath.Join(homeDir, ".config", "claude")
	if _, err := os.Stat(configDir); err == nil {
		return configDir
	}

	// Try ~/.claude
	dotDir := filepath.Join(homeDir, ".claude")
	if _, err := os.Stat(dotDir); err == nil {
		return dotDir
	}

	return ""
}

// SetupViper configures viper for settings management
func SetupViper() error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	configDir := filepath.Join(homeDir, ".config", "agentsandbox")
	viper.SetConfigName("settings")
	viper.SetConfigType("json")
	viper.AddConfigPath(configDir)

	// Set defaults
	defaults := DefaultSettings()
	viper.SetDefault("skip_permission_flags", defaults.SkipPermissionFlags)
	viper.SetDefault("env_files", defaults.EnvFiles)

	// Read config (ignore error if file doesn't exist)
	_ = viper.ReadInConfig()

	return nil
}

