package backend

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func OpenFolderInExplorer(path string) error {
	var cmd *exec.Cmd

	switch runtime.GOOS {
	case "windows":
		// Use cmd /c start to open explorer - more reliable
		cmd = exec.Command("cmd", "/c", "start", "", path)
	case "darwin": // macOS
		cmd = exec.Command("open", path)
	case "linux":
		cmd = exec.Command("xdg-open", path)
	default:
		cmd = exec.Command("xdg-open", path)
	}

	hideWindow(cmd) // Hide console window on Windows
	return cmd.Run()
}

func SelectFolderDialog(ctx context.Context, defaultPath string) (string, error) {
	// If defaultPath is empty, use default download path
	if defaultPath == "" {
		defaultPath = GetDefaultDownloadPath()
	}

	options := wailsRuntime.OpenDialogOptions{
		Title:            "Select Download Folder",
		DefaultDirectory: defaultPath,
	}

	selectedPath, err := wailsRuntime.OpenDirectoryDialog(ctx, options)
	if err != nil {
		return "", err
	}

	// If user cancelled, selectedPath will be empty
	if selectedPath == "" {
		return "", nil
	}

	return selectedPath, nil
}

// CheckFolderExists checks if a folder exists at the given path
func CheckFolderExists(basePath, username string) bool {
	folderPath := filepath.Join(basePath, username)
	info, err := os.Stat(folderPath)
	if err != nil {
		return false
	}
	return info.IsDir()
}

// CheckGifsFolderExists checks if a gifs subfolder exists for the given username
func CheckGifsFolderExists(basePath, username string) bool {
	gifsPath := filepath.Join(basePath, username, "gifs")
	info, err := os.Stat(gifsPath)
	if err != nil {
		return false
	}
	return info.IsDir()
}

// CheckGifsFolderHasMP4 checks if the gifs folder has any MP4 files to convert
func CheckGifsFolderHasMP4(basePath, username string) bool {
	gifsPath := filepath.Join(basePath, username, "gifs")

	entries, err := os.ReadDir(gifsPath)
	if err != nil {
		return false
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			ext := filepath.Ext(entry.Name())
			if ext == ".mp4" || ext == ".MP4" {
				return true
			}
		}
	}
	return false
}

// GetFolderPath returns the full path for a username folder
func GetFolderPath(basePath, username string) string {
	return filepath.Join(basePath, username)
}

// GetGifsFolderPath returns the full path for a username's gifs folder
func GetGifsFolderPath(basePath, username string) string {
	return filepath.Join(basePath, username, "gifs")
}
