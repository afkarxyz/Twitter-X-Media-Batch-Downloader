package backend

import (
	"os"
	"path/filepath"
)

func GetDefaultDownloadPath() string {
	// Get user's home directory
	homeDir, err := os.UserHomeDir()
	if err != nil {
		// Fallback to Public Pictures if can't get home dir
		return "C:\\Users\\Public\\Pictures"
	}

	// Return path to user's Pictures folder
	return filepath.Join(homeDir, "Pictures")
}
