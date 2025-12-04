package backend

import (
	"context"
	"os/exec"
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
