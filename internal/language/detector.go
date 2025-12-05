package language

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// Language represents a programming language detected in the project
type Language string

const (
	LanguageRust   Language = "rust"
	LanguageNodeJS Language = "nodejs"
	LanguagePython Language = "python"
	LanguageGo     Language = "go"
	LanguagePHP    Language = "php"
	LanguageRuby   Language = "ruby"
)

// Name returns the display name of the language
func (l Language) Name() string {
	switch l {
	case LanguageRust:
		return "Rust"
	case LanguageNodeJS:
		return "Node.js"
	case LanguagePython:
		return "Python"
	case LanguageGo:
		return "Go"
	case LanguagePHP:
		return "PHP"
	case LanguageRuby:
		return "Ruby"
	default:
		return string(l)
	}
}

// GlobalConfigPaths returns paths to global configuration directories
func (l Language) GlobalConfigPaths() []string {
	switch l {
	case LanguageRust:
		return []string{".cargo", ".rustup"}
	case LanguageNodeJS:
		return []string{".npm", ".npmrc", ".yarn"}
	case LanguagePython:
		return []string{".pip", ".cache/pip", ".pypirc"}
	case LanguageGo:
		return []string{"go", ".config/go"}
	case LanguagePHP:
		return []string{".composer"}
	case LanguageRuby:
		return []string{".gem", ".bundle"}
	default:
		return []string{}
	}
}

// Tool returns the primary tool/package manager for the language
func (l Language) Tool() string {
	switch l {
	case LanguageRust:
		return "cargo"
	case LanguageNodeJS:
		return "npm"
	case LanguagePython:
		return "pip"
	case LanguageGo:
		return "go"
	case LanguagePHP:
		return "composer"
	case LanguageRuby:
		return "bundle"
	default:
		return ""
	}
}

// InstallCmd returns the command to install the language toolchain
func (l Language) InstallCmd() string {
	switch l {
	case LanguageRust:
		return "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && ~/.cargo/bin/rustup component add rustfmt clippy"
	case LanguageNodeJS:
		return "curl -fsSL https://deb.nodesource.com/setup_22.x | sudo bash - && sudo apt-get install -y nodejs"
	case LanguagePython:
		return "sudo apt-get update && sudo apt-get install -y python3 python3-pip"
	case LanguageGo:
		return "wget https://go.dev/dl/go1.24.5.linux-amd64.tar.gz && sudo tar -C /usr/local -xzf go1.24.5.linux-amd64.tar.gz && rm go1.24.5.linux-amd64.tar.gz"
	case LanguagePHP:
		return "sudo apt-get update && sudo apt-get install -y php-cli unzip && curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer"
	case LanguageRuby:
		return "sudo apt-get update && sudo apt-get install -y ruby-full && sudo gem install bundler"
	default:
		return ""
	}
}

// DetectProjectLanguages detects which programming languages are used in a project
func DetectProjectLanguages(dir string) []Language {
	var languages []Language

	// Check for various project files
	checks := map[Language][]string{
		LanguageRust:   {"Cargo.toml"},
		LanguageNodeJS: {"package.json"},
		LanguagePython: {"requirements.txt", "pyproject.toml"},
		LanguageGo:     {"go.mod"},
		LanguagePHP:    {"composer.json"},
		LanguageRuby:   {"Gemfile"},
	}

	for lang, files := range checks {
		for _, file := range files {
			if _, err := os.Stat(filepath.Join(dir, file)); err == nil {
				languages = append(languages, lang)
				break
			}
		}
	}

	return languages
}

// EnsureLanguageTools checks for and installs missing language tools in the container
func EnsureLanguageTools(containerName string, languages []Language) error {
	for _, lang := range languages {
		tool := lang.Tool()
		if tool == "" {
			continue
		}

		// Check if tool exists
		checkCmd := exec.Command("docker", "exec", containerName, "bash", "-lc",
			fmt.Sprintf("command -v %s", tool))
		if err := checkCmd.Run(); err == nil {
			// Tool already exists
			continue
		}

		// Install the tool
		fmt.Printf("Installing toolchain for %s...\n", lang.Name())
		installCmd := exec.Command("docker", "exec", containerName, "bash", "-lc", lang.InstallCmd())
		installCmd.Stdout = os.Stdout
		installCmd.Stderr = os.Stderr

		if err := installCmd.Run(); err != nil {
			return fmt.Errorf("failed to install %s: %w", tool, err)
		}
	}

	return nil
}

// SyncNodeModulesFromHost copies node_modules from host to container for Node.js projects
func SyncNodeModulesFromHost(containerName string, projectDir string, languages []Language) error {
	// Check if Node.js is in the detected languages
	hasNodeJS := false
	for _, lang := range languages {
		if lang == LanguageNodeJS {
			hasNodeJS = true
			break
		}
	}

	if !hasNodeJS {
		return nil
	}

	hostNM := filepath.Join(projectDir, "node_modules")
	if info, err := os.Stat(hostNM); err != nil || !info.IsDir() {
		// No node_modules to copy
		return nil
	}

	fmt.Println("Syncing node_modules to container...")

	// Ensure target path exists in container
	mkdirCmd := fmt.Sprintf("sudo mkdir -p '%s' && sudo chown -R $(id -u):$(id -g) '%s'",
		hostNM, hostNM)
	mkdirExec := exec.Command("docker", "exec", containerName, "bash", "-lc", mkdirCmd)
	if err := mkdirExec.Run(); err != nil {
		return fmt.Errorf("failed to create node_modules path in container: %w", err)
	}

	// Copy node_modules to container
	src := filepath.Join(hostNM, ".") + string(filepath.Separator)
	dest := fmt.Sprintf("%s:%s", containerName, hostNM)
	cpCmd := exec.Command("docker", "cp", src, dest)
	if err := cpCmd.Run(); err != nil {
		return fmt.Errorf("failed to copy node_modules to container: %w", err)
	}

	fmt.Println("node_modules synced successfully")
	return nil
}

