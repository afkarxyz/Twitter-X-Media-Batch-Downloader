package backend

import (
	"archive/tar"
	"archive/zip"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/ulikunitz/xz"
)

// FFmpeg download URLs
const (
	ffmpegWindowsURL = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
	ffmpegLinuxURL   = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
	ffmpegMacOSURL   = "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
)

// GetFFmpegPath returns the path to ffmpeg binary
func GetFFmpegPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")

	switch runtime.GOOS {
	case "windows":
		return filepath.Join(baseDir, "ffmpeg.exe")
	default:
		return filepath.Join(baseDir, "ffmpeg")
	}
}

// IsFFmpegInstalled checks if ffmpeg is available
func IsFFmpegInstalled() bool {
	ffmpegPath := GetFFmpegPath()
	if _, err := os.Stat(ffmpegPath); err == nil {
		return true
	}
	return false
}

// DownloadFFmpeg downloads ffmpeg binary for current platform
func DownloadFFmpeg(progressCallback func(downloaded, total int64)) error {
	var downloadURL string

	switch runtime.GOOS {
	case "windows":
		downloadURL = ffmpegWindowsURL
	case "linux":
		downloadURL = ffmpegLinuxURL
	case "darwin":
		downloadURL = ffmpegMacOSURL
	default:
		return fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}

	// Create temp file for download
	tempFile, err := os.CreateTemp("", "ffmpeg-*")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	// Download file
	resp, err := http.Get(downloadURL)
	if err != nil {
		return fmt.Errorf("failed to download ffmpeg: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download ffmpeg: status %d", resp.StatusCode)
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

	// Extract ffmpeg binary
	ffmpegPath := GetFFmpegPath()
	baseDir := filepath.Dir(ffmpegPath)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	switch runtime.GOOS {
	case "windows", "darwin":
		return extractFromZip(tempPath, ffmpegPath)
	case "linux":
		return extractFromTarXz(tempPath, ffmpegPath)
	}

	return nil
}

// extractFromZip extracts ffmpeg from zip archive
func extractFromZip(zipPath, destPath string) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open zip: %v", err)
	}
	defer r.Close()

	for _, f := range r.File {
		// Look for ffmpeg binary
		name := filepath.Base(f.Name)
		if name == "ffmpeg" || name == "ffmpeg.exe" {
			rc, err := f.Open()
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

			// Make executable on Unix
			if runtime.GOOS != "windows" {
				os.Chmod(destPath, 0755)
			}

			return nil
		}
	}

	return fmt.Errorf("ffmpeg binary not found in archive")
}

// extractFromTarXz extracts ffmpeg from tar.xz archive
func extractFromTarXz(tarXzPath, destPath string) error {
	file, err := os.Open(tarXzPath)
	if err != nil {
		return fmt.Errorf("failed to open tar.xz: %v", err)
	}
	defer file.Close()

	xzReader, err := xz.NewReader(file)
	if err != nil {
		return fmt.Errorf("failed to create xz reader: %v", err)
	}

	tarReader := tar.NewReader(xzReader)

	for {
		header, err := tarReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("failed to read tar: %v", err)
		}

		// Look for ffmpeg binary
		name := filepath.Base(header.Name)
		if name == "ffmpeg" && header.Typeflag == tar.TypeReg {
			out, err := os.Create(destPath)
			if err != nil {
				return fmt.Errorf("failed to create output file: %v", err)
			}
			defer out.Close()

			if _, err := io.Copy(out, tarReader); err != nil {
				return fmt.Errorf("failed to extract file: %v", err)
			}

			os.Chmod(destPath, 0755)
			return nil
		}
	}

	return fmt.Errorf("ffmpeg binary not found in archive")
}

// ConvertMP4ToGIF converts an MP4 file to GIF using ffmpeg
// quality: "fast" for simple conversion, "better" for optimized palette
// resolution: "original", "high" (800px), "medium" (600px), "low" (400px)
func ConvertMP4ToGIF(inputPath, outputPath, quality, resolution string) error {
	ffmpegPath := GetFFmpegPath()

	if !IsFFmpegInstalled() {
		return fmt.Errorf("ffmpeg not installed")
	}

	var args []string

	if quality == "fast" {
		// Fast mode: simple conversion with resolution scaling
		var scaleFilter string
		switch resolution {
		case "high":
			scaleFilter = "scale=800:-1"
		case "medium":
			scaleFilter = "scale=600:-1"
		case "low":
			scaleFilter = "scale=400:-1"
		default: // original - no scaling
			scaleFilter = ""
		}

		if scaleFilter != "" {
			args = []string{
				"-i", inputPath,
				"-vf", scaleFilter,
				"-loop", "0",
				"-y",
				outputPath,
			}
		} else {
			args = []string{
				"-i", inputPath,
				"-loop", "0",
				"-y",
				outputPath,
			}
		}
	} else {
		// Better mode: optimized palette with dithering
		var filter string

		switch resolution {
		case "high":
			filter = "scale=800:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=800:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		case "medium":
			filter = "scale=600:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=600:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		case "low":
			filter = "scale=400:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=400:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		default: // original
			filter = "palettegen=stats_mode=full[palette];[0:v][palette]paletteuse=dither=sierra2_4a"
		}

		// Set FPS based on resolution
		fps := "15"
		if resolution == "medium" {
			fps = "10"
		} else if resolution == "low" {
			fps = "8"
		}

		args = []string{
			"-i", inputPath,
			"-lavfi", filter,
			"-r", fps,
			"-y",
			outputPath,
		}
	}

	cmd := exec.Command(ffmpegPath, args...)
	hideWindow(cmd) // Hide console window on Windows
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("ffmpeg error: %v, output: %s", err, string(output))
	}

	return nil
}

// ConvertGIFsInFolder converts all MP4 files in gifs folder to actual GIF format
func ConvertGIFsInFolder(folderPath, quality, resolution string, deleteOriginal bool) (converted int, failed int, err error) {
	if !IsFFmpegInstalled() {
		return 0, 0, fmt.Errorf("ffmpeg not installed")
	}

	// Clean the path to handle cross-platform path separators
	cleanPath := filepath.Clean(folderPath)
	gifsFolder := filepath.Join(cleanPath, "gifs")
	if _, err := os.Stat(gifsFolder); os.IsNotExist(err) {
		return 0, 0, fmt.Errorf("gifs folder not found: %s", gifsFolder)
	}

	files, err := os.ReadDir(gifsFolder)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to read gifs folder: %v", err)
	}

	for _, file := range files {
		if file.IsDir() {
			continue
		}

		name := file.Name()
		if !strings.HasSuffix(strings.ToLower(name), ".mp4") {
			continue
		}

		inputPath := filepath.Join(gifsFolder, name)
		outputPath := filepath.Join(gifsFolder, strings.TrimSuffix(name, filepath.Ext(name))+".gif")

		if err := ConvertMP4ToGIF(inputPath, outputPath, quality, resolution); err != nil {
			failed++
			continue
		}

		if deleteOriginal {
			os.Remove(inputPath)
		}

		converted++
	}

	return converted, failed, nil
}
