package backend

import (
	"archive/zip"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

// ExifTool download URLs (SourceForge)
const (
	// Windows 64-bit
	exiftoolWindows64URL = "https://sourceforge.net/projects/exiftool/files/exiftool-13.43_64.zip/download"
	// Windows 32-bit
	exiftoolWindows32URL = "https://sourceforge.net/projects/exiftool/files/exiftool-13.43_32.zip/download"
	// Unix (Linux/macOS): tar.gz
	exiftoolUnixURL = "https://sourceforge.net/projects/exiftool/files/Image-ExifTool-13.43.tar.gz/download"
)

// ExifTool SHA256 hashes for verification
// Note: Hashes will be calculated during download if not provided
// For now, we'll skip hash verification for version 13.43 (can be added later if needed)
const (
	exiftoolWindows64Hash = "" // Hash not provided, will skip verification
	exiftoolWindows32Hash = "" // Hash not provided, will skip verification
	exiftoolUnixHash      = "" // Hash not provided, will skip verification
)

// GetExifToolPath returns the path to exiftool binary
func GetExifToolPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")

	switch runtime.GOOS {
	case "windows":
		return filepath.Join(baseDir, "exiftool.exe")
	default:
		// For Unix (Linux/macOS), exiftool is in Image-ExifTool-VERSION/exiftool
		// We need to find the folder dynamically since version may vary
		pattern := filepath.Join(baseDir, "Image-ExifTool-*")
		matches, err := filepath.Glob(pattern)
		if err == nil && len(matches) > 0 {
			// Use the first match (should only be one)
			exiftoolPath := filepath.Join(matches[0], "exiftool")
			if _, err := os.Stat(exiftoolPath); err == nil {
				return exiftoolPath
			}
		}
		// Fallback: return path that will be created after extraction
		return filepath.Join(baseDir, "exiftool")
	}
}

// IsExifToolInstalled checks if exiftool is available
func IsExifToolInstalled() bool {
	exiftoolPath := GetExifToolPath()
	if _, err := os.Stat(exiftoolPath); err == nil {
		// Test if it's executable by running version command
		cmd := exec.Command(exiftoolPath, "-ver")
		hideWindow(cmd)
		if err := cmd.Run(); err == nil {
			return true
		}
	}
	return false
}

// calculateSHA256 calculates SHA256 hash of a file
func calculateSHA256(filePath string) (string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", err
	}
	defer file.Close()

	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return "", err
	}

	return hex.EncodeToString(hash.Sum(nil)), nil
}

// verifyHash verifies the SHA256 hash of a downloaded file
func verifyHash(filePath, expectedHash string) error {
	actualHash, err := calculateSHA256(filePath)
	if err != nil {
		return fmt.Errorf("failed to calculate hash: %v", err)
	}

	if !strings.EqualFold(actualHash, expectedHash) {
		return fmt.Errorf("hash verification failed: expected %s, got %s", expectedHash, actualHash)
	}

	return nil
}

// is64Bit checks if the system is 64-bit
func is64Bit() bool {
	return runtime.GOARCH == "amd64" || runtime.GOARCH == "arm64"
}

// DownloadExifTool downloads exiftool binary for current platform
func DownloadExifTool(progressCallback func(downloaded, total int64)) error {
	var downloadURL string
	var expectedHash string

	switch runtime.GOOS {
	case "windows":
		if is64Bit() {
			downloadURL = exiftoolWindows64URL
			expectedHash = exiftoolWindows64Hash
		} else {
			downloadURL = exiftoolWindows32URL
			expectedHash = exiftoolWindows32Hash
		}
	case "linux", "darwin":
		// Linux and macOS use the same tar.gz archive
		downloadURL = exiftoolUnixURL
		expectedHash = exiftoolUnixHash
	default:
		return fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}

	// Create temp file for download
	tempFile, err := os.CreateTemp("", "exiftool-*")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	// Download file
	resp, err := http.Get(downloadURL)
	if err != nil {
		return fmt.Errorf("failed to download exiftool: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download exiftool: status %d", resp.StatusCode)
	}

	// Copy with progress
	total := resp.ContentLength
	var downloaded int64
	buf := make([]byte, 32*1024)

	for {
		n, err := resp.Body.Read(buf)
		if n > 0 {
			_, writeErr := tempFile.Write(buf[:n])
			if writeErr != nil {
				return fmt.Errorf("failed to write temp file: %v", writeErr)
			}
			downloaded += int64(n)
			if progressCallback != nil {
				progressCallback(downloaded, total)
			}
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("failed to download: %v", err)
		}
	}
	tempFile.Close()

	// Verify hash before extraction
	if expectedHash != "" {
		if err := verifyHash(tempPath, expectedHash); err != nil {
			return fmt.Errorf("hash verification failed: %v", err)
		}
	}

	// Extract exiftool binary
	exiftoolPath := GetExifToolPath()
	baseDir := filepath.Dir(exiftoolPath)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	switch runtime.GOOS {
	case "windows":
		return extractExifToolFromZip(tempPath, exiftoolPath)
	case "linux", "darwin":
		// For Linux/macOS, we need to extract and build
		// For simplicity, we'll extract the exiftool script from tar.gz
		return extractExifToolFromTarGz(tempPath, exiftoolPath)
	}

	return nil
}

// extractExifToolFromZip extracts exiftool from Windows zip archive
func extractExifToolFromZip(zipPath, destPath string) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open zip: %v", err)
	}
	defer r.Close()

	// Look for exiftool(-k).exe or exiftool.exe in the zip
	var exiftoolFile *zip.File
	for _, f := range r.File {
		name := filepath.Base(f.Name)
		// ExifTool Windows zip contains exiftool(-k).exe
		if name == "exiftool(-k).exe" || name == "exiftool.exe" {
			exiftoolFile = f
			break
		}
	}

	if exiftoolFile == nil {
		return fmt.Errorf("exiftool.exe not found in archive")
	}

	rc, err := exiftoolFile.Open()
	if err != nil {
		return fmt.Errorf("failed to open file in zip: %v", err)
	}
	defer rc.Close()

	out, err := os.Create(destPath)
	if err != nil {
		return fmt.Errorf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, rc); err != nil {
		return fmt.Errorf("failed to extract file: %v", err)
	}

	// Also need to extract the lib directory for Windows (exiftool_files folder)
	libDir := filepath.Join(filepath.Dir(destPath), "exiftool_files")
	for _, f := range r.File {
		// Check if file is in exiftool_files directory
		if strings.Contains(f.Name, "exiftool_files") && !strings.HasSuffix(f.Name, "/") {
			// Extract relative path from exiftool_files
			parts := strings.Split(f.Name, "exiftool_files/")
			if len(parts) < 2 {
				continue
			}
			relPath := parts[1]
			if relPath == "" {
				continue
			}

			targetPath := filepath.Join(libDir, relPath)

			// Create directory structure
			if err := os.MkdirAll(filepath.Dir(targetPath), 0755); err != nil {
				continue
			}

			// Skip if it's a directory entry
			if f.FileInfo().IsDir() {
				os.MkdirAll(targetPath, 0755)
				continue
			}

			// Extract file
			rc, err := f.Open()
			if err != nil {
				continue // Skip on error
			}

			outFile, err := os.Create(targetPath)
			if err != nil {
				rc.Close()
				continue
			}

			io.Copy(outFile, rc)
			outFile.Close()
			rc.Close()
		}
	}

	return nil
}

// extractExifToolFromTarGz extracts exiftool from tar.gz archive (Linux/macOS)
func extractExifToolFromTarGz(tarGzPath, destPath string) error {
	// For Linux/macOS, ExifTool is a Perl script that requires the lib directory
	// We'll extract the entire folder and use exiftool directly from Image-ExifTool-VERSION/exiftool
	// This is simpler than implementing tar.gz extraction in Go

	baseDir := filepath.Dir(destPath)

	// Use system tar command to extract
	cmd := exec.Command("tar", "-xzf", tarGzPath, "-C", baseDir)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to extract tar.gz: %v, output: %s", err, string(output))
	}

	// Find the extracted exiftool script
	// The archive extracts to Image-ExifTool-VERSION/exiftool
	// We need to find the folder dynamically since version may vary
	var extractedDir string
	var exiftoolScript string

	// Use glob pattern to find Image-ExifTool-* directories
	pattern := filepath.Join(baseDir, "Image-ExifTool-*")
	matches, err := filepath.Glob(pattern)
	if err == nil && len(matches) > 0 {
		// Use the first match (should only be one)
		extractedDir = matches[0]
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	} else {
		// Fallback: try common version or alternative location
		extractedDir = filepath.Join(baseDir, "Image-ExifTool-13.43")
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	}

	// Check if exiftool script exists
	if _, err := os.Stat(exiftoolScript); err != nil {
		// Try alternative location
		extractedDir = filepath.Join(baseDir, "exiftool")
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
		if _, err := os.Stat(exiftoolScript); err != nil {
			return fmt.Errorf("exiftool script not found in extracted archive (searched in: %s and %s)", pattern, extractedDir)
		}
	}

	// Make exiftool executable (required for Unix systems)
	if err := os.Chmod(exiftoolScript, 0755); err != nil {
		return fmt.Errorf("failed to make exiftool executable: %v", err)
	}

	// For Unix, we keep the entire folder structure and use exiftool directly from it
	// The folder will remain in baseDir and GetExifToolPath() will find it via glob pattern
	// No need to copy or remove anything

	return nil
}

