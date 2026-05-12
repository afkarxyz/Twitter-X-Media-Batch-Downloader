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

const (
	exiftoolWindows64URL = "https://sourceforge.net/projects/exiftool/files/exiftool-13.43_64.zip/download"

	exiftoolWindows32URL = "https://sourceforge.net/projects/exiftool/files/exiftool-13.43_32.zip/download"

	exiftoolUnixURL = "https://sourceforge.net/projects/exiftool/files/Image-ExifTool-13.43.tar.gz/download"
)

const (
	exiftoolWindows64Hash = ""
	exiftoolWindows32Hash = ""
	exiftoolUnixHash      = ""
)

func GetExifToolPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")

	switch runtime.GOOS {
	case "windows":
		return filepath.Join(baseDir, "exiftool.exe")
	default:

		pattern := filepath.Join(baseDir, "Image-ExifTool-*")
		matches, err := filepath.Glob(pattern)
		if err == nil && len(matches) > 0 {

			exiftoolPath := filepath.Join(matches[0], "exiftool")
			if _, err := os.Stat(exiftoolPath); err == nil {
				return exiftoolPath
			}
		}

		return filepath.Join(baseDir, "exiftool")
	}
}

func IsExifToolInstalled() bool {
	exiftoolPath := GetExifToolPath()
	if _, err := os.Stat(exiftoolPath); err == nil {

		cmd := exec.Command(exiftoolPath, "-ver")
		hideWindow(cmd)
		if err := cmd.Run(); err == nil {
			return true
		}
	}
	return false
}

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

func is64Bit() bool {
	return runtime.GOARCH == "amd64" || runtime.GOARCH == "arm64"
}

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

		downloadURL = exiftoolUnixURL
		expectedHash = exiftoolUnixHash
	default:
		return fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}

	tempFile, err := os.CreateTemp("", "exiftool-*")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	resp, err := http.Get(downloadURL)
	if err != nil {
		return fmt.Errorf("failed to download exiftool: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download exiftool: status %d", resp.StatusCode)
	}

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

	if expectedHash != "" {
		if err := verifyHash(tempPath, expectedHash); err != nil {
			return fmt.Errorf("hash verification failed: %v", err)
		}
	}

	exiftoolPath := GetExifToolPath()
	baseDir := filepath.Dir(exiftoolPath)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	switch runtime.GOOS {
	case "windows":
		return extractExifToolFromZip(tempPath, exiftoolPath)
	case "linux", "darwin":

		return extractExifToolFromTarGz(tempPath, exiftoolPath)
	}

	return nil
}

func extractExifToolFromZip(zipPath, destPath string) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open zip: %v", err)
	}
	defer r.Close()

	var exiftoolFile *zip.File
	for _, f := range r.File {
		name := filepath.Base(f.Name)

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

	libDir := filepath.Join(filepath.Dir(destPath), "exiftool_files")
	for _, f := range r.File {

		if strings.Contains(f.Name, "exiftool_files") && !strings.HasSuffix(f.Name, "/") {

			parts := strings.Split(f.Name, "exiftool_files/")
			if len(parts) < 2 {
				continue
			}
			relPath := parts[1]
			if relPath == "" {
				continue
			}

			targetPath := filepath.Join(libDir, relPath)

			if err := os.MkdirAll(filepath.Dir(targetPath), 0755); err != nil {
				continue
			}

			if f.FileInfo().IsDir() {
				os.MkdirAll(targetPath, 0755)
				continue
			}

			rc, err := f.Open()
			if err != nil {
				continue
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

func extractExifToolFromTarGz(tarGzPath, destPath string) error {

	baseDir := filepath.Dir(destPath)

	cmd := exec.Command("tar", "-xzf", tarGzPath, "-C", baseDir)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to extract tar.gz: %v, output: %s", err, string(output))
	}

	var extractedDir string
	var exiftoolScript string

	pattern := filepath.Join(baseDir, "Image-ExifTool-*")
	matches, err := filepath.Glob(pattern)
	if err == nil && len(matches) > 0 {

		extractedDir = matches[0]
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	} else {

		extractedDir = filepath.Join(baseDir, "Image-ExifTool-13.43")
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	}

	if _, err := os.Stat(exiftoolScript); err != nil {

		extractedDir = filepath.Join(baseDir, "exiftool")
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
		if _, err := os.Stat(exiftoolScript); err != nil {
			return fmt.Errorf("exiftool script not found in extracted archive (searched in: %s and %s)", pattern, extractedDir)
		}
	}

	if err := os.Chmod(exiftoolScript, 0755); err != nil {
		return fmt.Errorf("failed to make exiftool executable: %v", err)
	}

	return nil
}
