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
	os.WriteFile(filepath.Join(os.TempDir(), "openfolder.txt"), []byte(path), 0644)
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return os.ErrInvalid
	}

	var cmd *exec.Cmd

	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("cmd", "/C", "start", "", path)
	case "darwin":
		cmd = exec.Command("open", path)
	case "linux":
		cmd = exec.Command("xdg-open", path)
	default:
		cmd = exec.Command("xdg-open", path)
	}

	hideWindow(cmd)
	return cmd.Run()
}

func SelectFolderDialog(ctx context.Context, defaultPath string) (string, error) {

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

	if selectedPath == "" {
		return "", nil
	}

	return selectedPath, nil
}

func CheckFolderExists(basePath, username string) bool {
	folderPath := filepath.Join(basePath, username)
	info, err := os.Stat(folderPath)
	if err != nil {
		return false
	}
	return info.IsDir()
}

func CheckFoldersExist(basePath string, usernames []string) map[string]bool {
	results := make(map[string]bool, len(usernames))
	if basePath == "" {
		return results
	}

	checked := make(map[string]bool, len(usernames))
	for _, username := range usernames {
		if _, alreadyChecked := checked[username]; alreadyChecked {
			continue
		}
		exists := CheckFolderExists(basePath, username)
		checked[username] = exists
		results[username] = exists
	}

	return results
}

func CheckGifsFolderExists(basePath, username string) bool {
	gifsPath := filepath.Join(basePath, username, "gifs")
	info, err := os.Stat(gifsPath)
	if err != nil {
		return false
	}
	return info.IsDir()
}

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

func GetFolderPath(basePath, username string) string {
	return filepath.Join(basePath, username)
}

func GetGifsFolderPath(basePath, username string) string {
	return filepath.Join(basePath, username, "gifs")
}
