package backend

import (
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
)

// ExtractOriginalFilename extracts the original filename from Twitter media URL
// Supports 3 formats:
// 1. animated_gif: https://video.twimg.com/tweet_video/GzstJyDbIAAPuX9.mp4 -> GzstJyDbIAAPuX9
// 2. photo: https://pbs.twimg.com/media/GynjhU0bYAAA_I4?format=jpg -> GynjhU0bYAAA_I4
// 3. video: https://video.twimg.com/amplify_video/.../Jpo0cDa7aNtjSv7o.mp4 -> Jpo0cDa7aNtjSv7o
func ExtractOriginalFilename(mediaURL string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return ""
	}

	path := parsedURL.Path
	parts := strings.Split(path, "/")

	// 1. animated_gif: video.twimg.com/tweet_video/XXX.mp4
	if strings.Contains(mediaURL, "/tweet_video/") {
		for i, part := range parts {
			if part == "tweet_video" && i+1 < len(parts) {
				filename := parts[i+1]
				// Remove extension
				if idx := strings.LastIndex(filename, "."); idx > 0 {
					filename = filename[:idx]
				}
				// Validate alphanumeric + underscore (Twitter filenames can contain _)
				if matched, _ := regexp.MatchString(`^[A-Za-z0-9_]+$`, filename); matched && len(filename) > 0 {
					return filename
				}
			}
		}
	}

	// 2. photo: pbs.twimg.com/media/XXX
	if strings.Contains(mediaURL, "pbs.twimg.com/media/") {
		if len(parts) > 0 {
			filename := parts[len(parts)-1]
			// Remove extension if present
			if idx := strings.LastIndex(filename, "."); idx > 0 {
				filename = filename[:idx]
			}
			// Validate alphanumeric + underscore (Twitter filenames can contain _)
			if matched, _ := regexp.MatchString(`^[A-Za-z0-9_]+$`, filename); matched && len(filename) > 0 {
				return filename
			}
		}
	}

	// 3. video: video.twimg.com/amplify_video/.../XXX.mp4 or video.twimg.com/ext_tw_video/.../XXX.mp4
	// Filename is the last segment (before extension)
	// Note: Skip if already handled by tweet_video above
	if strings.Contains(mediaURL, "video.twimg.com") && !strings.Contains(mediaURL, "/tweet_video/") {
		if len(parts) > 0 {
			filename := parts[len(parts)-1]
			// Remove extension
			if idx := strings.LastIndex(filename, "."); idx > 0 {
				filename = filename[:idx]
			}
			// Validate: alphanumeric + underscore with at least one letter (to exclude pure numeric IDs)
			if matched, _ := regexp.MatchString(`^[A-Za-z0-9_]+$`, filename); matched {
				hasLetter, _ := regexp.MatchString(`[A-Za-z]`, filename)
				if hasLetter && len(filename) >= 8 {
					return filename
				}
			}
		}
	}

	return ""
}

// EmbedMetadata embeds metadata into a media file
// Only supports JPG (images) and MP4 (videos)
func EmbedMetadata(filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	ext := strings.ToLower(filepath.Ext(filePath))
	
	switch ext {
	case ".jpg", ".jpeg":
		return embedImageMetadata(filePath, tweetContent, tweetURL, originalFilename)
	case ".mp4":
		return embedVideoMetadata(filePath, tweetContent, tweetURL, originalFilename)
	default:
		// For unsupported formats, skip metadata embedding
		return nil
	}
}

// embedImageMetadata embeds metadata into image files using exiftool or similar
// Since we don't want to add heavy dependencies, we'll use a simple approach:
// For JPEG: We can use exiftool if available, or skip if not
// For PNG: Limited support, skip for now
func embedImageMetadata(filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	// Try to use exiftool if available (common tool for metadata)
	exiftoolPath := findExifTool()
	if exiftoolPath == "" {
		// exiftool not found, skip metadata embedding for images
		// This is acceptable as it's an optional feature
		return nil
	}

	// Build metadata comment
	metadataComment := buildMetadataComment(tweetContent, tweetURL, originalFilename)

	// Use exiftool to add comment only (URL | filename)
	args := []string{
		"-overwrite_original",
		"-Comment=" + metadataComment,
		filePath,
	}

	cmd := exec.Command(exiftoolPath, args...)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		// Non-fatal: if exiftool fails, just skip metadata
		return fmt.Errorf("exiftool error (non-fatal): %v, output: %s", err, string(output))
	}

	_ = output // Suppress unused variable warning
	return nil
}

// embedVideoMetadata embeds metadata into video/GIF files using ExifTool
func embedVideoMetadata(filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	// Use ExifTool for video metadata (works well for MP4)
	exiftoolPath := findExifTool()
	if exiftoolPath == "" {
		// ExifTool not available, skip metadata embedding (non-fatal)
		return nil
	}

	return embedVideoMetadataWithExifTool(exiftoolPath, filePath, tweetContent, tweetURL, originalFilename)
}

// embedVideoMetadataWithExifTool embeds metadata using ExifTool (preferred for MP4)
func embedVideoMetadataWithExifTool(exiftoolPath string, filePath string, tweetContent string, tweetURL string, originalFilename string) error {
	// Build metadata comment
	metadataComment := buildMetadataComment(tweetContent, tweetURL, originalFilename)

	// Use exiftool to add comment only (URL | filename)
	args := []string{
		"-overwrite_original",
		"-Comment=" + metadataComment,
		filePath,
	}

	cmd := exec.Command(exiftoolPath, args...)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		// Non-fatal: if exiftool fails, skip metadata (file still downloaded)
		return fmt.Errorf("exiftool error (non-fatal): %v, output: %s", err, string(output))
	}

	_ = output // Suppress unused variable warning
	return nil
}


// buildMetadataComment builds a formatted metadata comment string
func buildMetadataComment(tweetContent string, tweetURL string, originalFilename string) string {
	var parts []string
	
	if tweetURL != "" {
		parts = append(parts, tweetURL)
	}
	if originalFilename != "" {
		parts = append(parts, originalFilename)
	}
	
	// If no original filename, just return URL (don't add empty part)
	if len(parts) == 0 {
		return ""
	}
	
	return strings.Join(parts, " | ")
}

// findExifTool searches for exiftool, prioritizing the installed version in .twitterxmediabatchdownloader
func findExifTool() string {
	// First, check if exiftool is installed in .twitterxmediabatchdownloader
	if IsExifToolInstalled() {
		return GetExifToolPath()
	}

	// Fallback: check common system locations
	commonPaths := []string{
		"exiftool",
		"/usr/bin/exiftool",
		"/usr/local/bin/exiftool",
		"C:\\Program Files\\exiftool\\exiftool.exe",
		"C:\\exiftool\\exiftool.exe",
	}

	for _, path := range commonPaths {
		if runtime.GOOS == "windows" {
			// On Windows, try .exe extension
			if _, err := exec.LookPath(path); err == nil {
				return path
			}
		} else {
			// On Unix, check if executable
			if _, err := os.Stat(path); err == nil {
				return path
			}
		}
	}

	// Try to find in PATH
	if path, err := exec.LookPath("exiftool"); err == nil {
		return path
	}

	return ""
}

