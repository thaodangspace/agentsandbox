package clipboard

import (
	"crypto/md5"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const (
	maxImages     = 10
	checkInterval = 500 * time.Millisecond
)

// FeatureEnabled returns whether clipboard integration is currently enabled
// Currently disabled due to stability issues
func FeatureEnabled() bool {
	return false
}

// GetClipboardDir returns the clipboard directory path
func GetClipboardDir() (string, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(homeDir, ".config", "agentsandbox", "clipboard"), nil
}

// EnsureClipboardDir creates the clipboard directory if it doesn't exist
func EnsureClipboardDir() (string, error) {
	clipboardDir, err := GetClipboardDir()
	if err != nil {
		return "", err
	}

	if err := os.MkdirAll(clipboardDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create clipboard directory: %w", err)
	}

	return clipboardDir, nil
}

// GetWatcherPIDFile returns the path to the watcher PID file
func GetWatcherPIDFile() (string, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}

	configDir := filepath.Join(homeDir, ".config", "agentsandbox")
	if err := os.MkdirAll(configDir, 0755); err != nil {
		return "", err
	}

	return filepath.Join(configDir, "clipboard_watcher.pid"), nil
}

// SaveWatcherPID saves the watcher process PID
func SaveWatcherPID(pid int) error {
	pidFile, err := GetWatcherPIDFile()
	if err != nil {
		return err
	}

	return os.WriteFile(pidFile, []byte(fmt.Sprintf("%d", pid)), 0644)
}

// LoadWatcherPID loads the watcher process PID
func LoadWatcherPID() (int, error) {
	pidFile, err := GetWatcherPIDFile()
	if err != nil {
		return 0, err
	}

	if _, err := os.Stat(pidFile); os.IsNotExist(err) {
		return 0, nil
	}

	data, err := os.ReadFile(pidFile)
	if err != nil {
		return 0, err
	}

	var pid int
	if _, err := fmt.Sscanf(string(data), "%d", &pid); err != nil {
		return 0, nil
	}

	return pid, nil
}

// ClearWatcherPID removes the watcher PID file
func ClearWatcherPID() error {
	pidFile, err := GetWatcherPIDFile()
	if err != nil {
		return err
	}

	if err := os.Remove(pidFile); err != nil && !os.IsNotExist(err) {
		return err
	}

	return nil
}

// IsProcessRunning checks if a process is running on Linux
func IsProcessRunning(pid int) bool {
	_, err := os.Stat(fmt.Sprintf("/proc/%d", pid))
	return err == nil
}

// Watch starts watching the clipboard for images
// This is a simplified implementation that can be enhanced
func Watch(clipboardDir string) error {
	// Check if xclip is available
	if _, err := exec.LookPath("xclip"); err != nil {
		return fmt.Errorf("xclip is not installed: %w", err)
	}

	// Check if DISPLAY is set
	if os.Getenv("DISPLAY") == "" {
		return fmt.Errorf("DISPLAY environment variable is not set")
	}

	fmt.Printf("Clipboard watcher started, monitoring for images in: %s\n", clipboardDir)

	lastHash := ""
	ticker := time.NewTicker(checkInterval)
	defer ticker.Stop()

	for range ticker.C {
		// Check if clipboard contains image data
		targetsCmd := exec.Command("xclip", "-selection", "clipboard", "-t", "TARGETS", "-o")
		output, err := targetsCmd.Output()
		if err != nil {
			continue
		}

		targets := string(output)
		if !strings.Contains(targets, "image/") {
			continue
		}

		// Determine format
		format := "png"
		mimeType := "image/png"
		if strings.Contains(targets, "image/png") {
			format = "png"
			mimeType = "image/png"
		} else if strings.Contains(targets, "image/jpeg") {
			format = "jpg"
			mimeType = "image/jpeg"
		}

		// Get clipboard content
		imageCmd := exec.Command("xclip", "-selection", "clipboard", "-t", mimeType, "-o")
		imageData, err := imageCmd.Output()
		if err != nil {
			continue
		}

		// Compute hash
		hash := fmt.Sprintf("%x", md5.Sum(imageData))

		// Only save if this is a new image
		if hash != lastHash {
			timestamp := time.Now().Format("20060102-150405")
			filename := fmt.Sprintf("clipboard-%s.%s", timestamp, format)
			filePath := filepath.Join(clipboardDir, filename)

			// Save the image
			if err := os.WriteFile(filePath, imageData, 0644); err != nil {
				fmt.Printf("Failed to save clipboard image: %v\n", err)
				continue
			}

			fmt.Printf("Saved clipboard image: %s\n", filename)

			// Create symlinks
			latestLink := filepath.Join(clipboardDir, "latest."+format)
			os.Remove(latestLink)
			os.Symlink(filename, latestLink)

			genericLink := filepath.Join(clipboardDir, "latest")
			os.Remove(genericLink)
			os.Symlink(filename, genericLink)
			_ = genericLink

			lastHash = hash

			// Cleanup old images
			if err := cleanupOldImages(clipboardDir); err != nil {
				fmt.Printf("Warning: failed to cleanup old images: %v\n", err)
			}
		}
	}

	return nil
}

// cleanupOldImages removes old clipboard images, keeping only maxImages
func cleanupOldImages(clipboardDir string) error {
	entries, err := os.ReadDir(clipboardDir)
	if err != nil {
		return err
	}

	var imageFiles []os.DirEntry
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		name := entry.Name()
		if strings.HasPrefix(name, "clipboard-") &&
			(strings.HasSuffix(name, ".png") || strings.HasSuffix(name, ".jpg") || strings.HasSuffix(name, ".jpeg")) {
			imageFiles = append(imageFiles, entry)
		}
	}

	if len(imageFiles) <= maxImages {
		return nil
	}

	// Sort by modification time (oldest first)
	type fileInfo struct {
		name    string
		modTime time.Time
	}

	var files []fileInfo
	for _, entry := range imageFiles {
		info, err := entry.Info()
		if err != nil {
			continue
		}
		files = append(files, fileInfo{name: entry.Name(), modTime: info.ModTime()})
	}

	// Sort oldest first
	for i := 0; i < len(files)-1; i++ {
		for j := i + 1; j < len(files); j++ {
			if files[i].modTime.After(files[j].modTime) {
				files[i], files[j] = files[j], files[i]
			}
		}
	}

	// Delete oldest files
	toDelete := len(files) - maxImages
	for i := 0; i < toDelete; i++ {
		filePath := filepath.Join(clipboardDir, files[i].name)
		if err := os.Remove(filePath); err != nil && !os.IsNotExist(err) {
			return err
		}
	}

	return nil
}

// StartWatcher starts the clipboard watcher as a background process
func StartWatcher() error {
	// Check if already running
	pid, err := LoadWatcherPID()
	if err == nil && pid > 0 && IsProcessRunning(pid) {
		// Already running
		return nil
	}

	// Clear stale PID
	ClearWatcherPID()

	_, err = EnsureClipboardDir()
	if err != nil {
		return err
	}

	// Start watcher in background (would need to be a separate binary or goroutine)
	// For now, this is a placeholder
	fmt.Println("Clipboard watcher functionality available but currently disabled")

	return nil
}

