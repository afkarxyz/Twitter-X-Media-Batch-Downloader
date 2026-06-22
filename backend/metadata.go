package backend

import (
	"context"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
)

var (
	filenameSafeRegex   = regexp.MustCompile(`^[A-Za-z0-9_]+$`)
	filenameLetterRegex = regexp.MustCompile(`[A-Za-z]`)
)

func ExtractOriginalFilename(mediaURL string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return ""
	}

	path := parsedURL.Path
	parts := strings.Split(path, "/")

	if strings.Contains(mediaURL, "/tweet_video/") {
		for i, part := range parts {
			if part == "tweet_video" && i+1 < len(parts) {
				filename := parts[i+1]

				if idx := strings.LastIndex(filename, "."); idx > 0 {
					filename = filename[:idx]
				}

				if filenameSafeRegex.MatchString(filename) && len(filename) > 0 {
					return filename
				}
			}
		}
	}

	if strings.Contains(mediaURL, "pbs.twimg.com/media/") {
		if len(parts) > 0 {
			filename := parts[len(parts)-1]

			if idx := strings.LastIndex(filename, "."); idx > 0 {
				filename = filename[:idx]
			}

			if filenameSafeRegex.MatchString(filename) && len(filename) > 0 {
				return filename
			}
		}
	}

	if strings.Contains(mediaURL, "video.twimg.com") && !strings.Contains(mediaURL, "/tweet_video/") {
		if len(parts) > 0 {
			filename := parts[len(parts)-1]

			if idx := strings.LastIndex(filename, "."); idx > 0 {
				filename = filename[:idx]
			}

			if filenameSafeRegex.MatchString(filename) {
				if filenameLetterRegex.MatchString(filename) && len(filename) >= 8 {
					return filename
				}
			}
		}
	}

	return ""
}

func EmbedMetadata(ctx context.Context, filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	ext := strings.ToLower(filepath.Ext(filePath))

	switch ext {
	case ".jpg", ".jpeg":
		return embedImageMetadata(ctx, filePath, tweetContent, tweetURL, originalFilename)
	case ".mp4":
		return embedVideoMetadata(ctx, filePath, tweetContent, tweetURL, originalFilename)
	default:

		return nil
	}
}

func embedImageMetadata(ctx context.Context, filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	if ctx == nil {
		ctx = context.Background()
	}

	exiftoolPath := findExifTool()
	if exiftoolPath == "" {

		return nil
	}

	metadataComment := buildMetadataComment(tweetContent, tweetURL, originalFilename)

	args := []string{
		"-overwrite_original",
		"-Comment=" + metadataComment,
		filePath,
	}

	cmd := exec.CommandContext(ctx, exiftoolPath, args...)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		if ctx.Err() != nil {
			return ctx.Err()
		}

		return fmt.Errorf("exiftool error (non-fatal): %v, output: %s", err, string(output))
	}

	_ = output
	return nil
}

func embedVideoMetadata(ctx context.Context, filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	if ctx == nil {
		ctx = context.Background()
	}

	exiftoolPath := findExifTool()
	if exiftoolPath == "" {

		return nil
	}

	return embedVideoMetadataWithExifTool(ctx, exiftoolPath, filePath, tweetContent, tweetURL, originalFilename)
}

func embedVideoMetadataWithExifTool(ctx context.Context, exiftoolPath string, filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	if ctx == nil {
		ctx = context.Background()
	}

	metadataComment := buildMetadataComment(tweetContent, tweetURL, originalFilename)

	args := []string{
		"-overwrite_original",
		"-Comment=" + metadataComment,
		filePath,
	}

	cmd := exec.CommandContext(ctx, exiftoolPath, args...)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		if ctx.Err() != nil {
			return ctx.Err()
		}

		return fmt.Errorf("exiftool error (non-fatal): %v, output: %s", err, string(output))
	}

	_ = output
	return nil
}

func buildMetadataComment(tweetContent string, tweetURL string, originalFilename string) string {
	var parts []string

	if tweetURL != "" {
		parts = append(parts, tweetURL)
	}
	if originalFilename != "" {
		parts = append(parts, originalFilename)
	}

	if len(parts) == 0 {
		return ""
	}

	return strings.Join(parts, " | ")
}

func findExifTool() string {

	if IsExifToolInstalled() {
		return GetExifToolPath()
	}

	commonPaths := []string{
		"exiftool",
		"/usr/bin/exiftool",
		"/usr/local/bin/exiftool",
		"C:\\Program Files\\exiftool\\exiftool.exe",
		"C:\\exiftool\\exiftool.exe",
	}

	for _, path := range commonPaths {
		if runtime.GOOS == "windows" {

			if _, err := exec.LookPath(path); err == nil {
				return path
			}
		} else {

			if _, err := os.Stat(path); err == nil {
				return path
			}
		}
	}

	if path, err := exec.LookPath("exiftool"); err == nil {
		return path
	}

	return ""
}
