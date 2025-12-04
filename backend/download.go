package backend

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

const (
	// MaxConcurrentDownloads is the number of parallel downloads
	MaxConcurrentDownloads = 10
)

// MediaItem represents a media item with metadata for download
type MediaItem struct {
	URL      string `json:"url"`
	Date     string `json:"date"`
	TweetID  int64  `json:"tweet_id"`
	Type     string `json:"type"`
	Username string `json:"username"`
	Content  string `json:"content,omitempty"` // Tweet text content (for text-only tweets)
}

// DownloadMediaFiles downloads media files from URLs to the output directory (legacy)
func DownloadMediaFiles(urls []string, outputDir string) (downloaded int, failed int, err error) {
	// Create output directory if it doesn't exist
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		return 0, len(urls), fmt.Errorf("failed to create output directory: %v", err)
	}

	client := &http.Client{
		Timeout: 60 * time.Second,
	}

	for _, mediaURL := range urls {
		filename := extractFilename(mediaURL)
		outputPath := filepath.Join(outputDir, filename)

		// Skip if file already exists
		if _, err := os.Stat(outputPath); err == nil {
			downloaded++
			continue
		}

		if err := downloadFile(client, mediaURL, outputPath); err != nil {
			failed++
			continue
		}
		downloaded++
	}

	return downloaded, failed, nil
}

// ProgressCallback is a function type for progress updates
type ProgressCallback func(current, total int)

// ItemStatusCallback is a function type for per-item status updates
type ItemStatusCallback func(tweetID int64, index int, status string) // status: "success", "failed", "skipped"

// downloadTask represents a single download task
type downloadTask struct {
	item       MediaItem
	outputPath string
	index      int
}

// DownloadMediaWithMetadata downloads media files with proper naming and categorization
func DownloadMediaWithMetadata(items []MediaItem, outputDir string, username string) (downloaded int, failed int, err error) {
	return DownloadMediaWithMetadataProgressAndStatus(items, outputDir, username, nil, nil, nil)
}

// DownloadMediaWithMetadataProgress downloads media files with progress callback and cancellation support
func DownloadMediaWithMetadataProgress(items []MediaItem, outputDir string, username string, progress ProgressCallback, ctx context.Context) (downloaded int, failed int, err error) {
	return DownloadMediaWithMetadataProgressAndStatus(items, outputDir, username, progress, nil, ctx)
}

// DownloadMediaWithMetadataProgressAndStatus downloads media files with progress and per-item status callbacks
func DownloadMediaWithMetadataProgressAndStatus(items []MediaItem, outputDir string, username string, progress ProgressCallback, itemStatus ItemStatusCallback, ctx context.Context) (downloaded int, failed int, err error) {
	if ctx == nil {
		ctx = context.Background()
	}

	// Create base output directory
	baseDir := filepath.Join(outputDir, username)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return 0, len(items), fmt.Errorf("failed to create output directory: %v", err)
	}

	total := len(items)
	if total == 0 {
		return 0, 0, nil
	}

	// Prepare all tasks first (sequential to handle tweet media count)
	tweetMediaCount := make(map[int64]int)
	tasks := make([]downloadTask, 0, total)

	for i, item := range items {
		// Determine subfolder based on type
		var subfolder string
		switch item.Type {
		case "photo":
			subfolder = "images"
		case "video":
			subfolder = "videos"
		case "gif", "animated_gif":
			subfolder = "gifs"
		case "text":
			subfolder = "texts"
		default:
			subfolder = "other"
		}

		// Create type subfolder
		typeDir := filepath.Join(baseDir, subfolder)
		if err := os.MkdirAll(typeDir, 0755); err != nil {
			continue
		}

		// Format timestamp from date
		timestamp := formatTimestamp(item.Date)

		// Get file extension
		ext := getExtension(item.URL, item.Type)

		// Increment counter for this tweet_id
		tweetMediaCount[item.TweetID]++
		mediaIndex := tweetMediaCount[item.TweetID]

		// Create filename: {username}_{timestamp}_{tweet_id}_{index}.{ext}
		filename := fmt.Sprintf("%s_%s_%d_%02d%s", username, timestamp, item.TweetID, mediaIndex, ext)
		outputPath := filepath.Join(typeDir, filename)

		tasks = append(tasks, downloadTask{
			item:       item,
			outputPath: outputPath,
			index:      i,
		})
	}

	// Counters for parallel downloads
	var downloadedCount int64
	var failedCount int64
	var completedCount int64

	// Create worker pool
	taskChan := make(chan downloadTask, len(tasks))
	var wg sync.WaitGroup

	// Start workers
	numWorkers := MaxConcurrentDownloads
	if numWorkers > len(tasks) {
		numWorkers = len(tasks)
	}

	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			client := &http.Client{
				Timeout: 60 * time.Second,
			}

			for task := range taskChan {
				// Check for cancellation
				select {
				case <-ctx.Done():
					return
				default:
				}

				var status string
				// Skip if file already exists
				if _, err := os.Stat(task.outputPath); err == nil {
					atomic.AddInt64(&downloadedCount, 1)
					status = "skipped"
				} else if task.item.Type == "text" {
					// For text tweets, write content to file
					if err := os.WriteFile(task.outputPath, []byte(task.item.Content), 0644); err != nil {
						atomic.AddInt64(&failedCount, 1)
						status = "failed"
					} else {
						atomic.AddInt64(&downloadedCount, 1)
						status = "success"
					}
				} else if err := downloadFileWithContext(ctx, client, task.item.URL, task.outputPath); err != nil {
					atomic.AddInt64(&failedCount, 1)
					status = "failed"
				} else {
					atomic.AddInt64(&downloadedCount, 1)
					status = "success"
				}

				// Emit per-item status
				if itemStatus != nil {
					itemStatus(task.item.TweetID, task.index, status)
				}

				// Update progress
				completed := atomic.AddInt64(&completedCount, 1)
				if progress != nil {
					progress(int(completed), total)
				}
			}
		}()
	}

	// Send tasks to workers
	for _, task := range tasks {
		select {
		case <-ctx.Done():
			close(taskChan)
			wg.Wait()
			return int(downloadedCount), int(failedCount) + (total - int(completedCount)), ctx.Err()
		case taskChan <- task:
		}
	}
	close(taskChan)

	// Wait for all workers to finish
	wg.Wait()

	return int(downloadedCount), int(failedCount), nil
}

// downloadFileWithContext downloads a single file with context support for cancellation
func downloadFileWithContext(ctx context.Context, client *http.Client, url, outputPath string) error {
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return err
	}

	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	out, err := os.Create(outputPath)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	return err
}

// formatTimestamp converts date string to timestamp format
func formatTimestamp(dateStr string) string {
	// Try parsing various date formats
	formats := []string{
		"2006-01-02T15:04:05",       // ISO 8601 without timezone (from extractor)
		"2006-01-02T15:04:05+00:00", // ISO 8601 with timezone
		"2006-01-02T15:04:05-07:00", // ISO 8601 with timezone offset
		time.RFC3339,                // Standard RFC3339
		"2006-01-02T15:04:05.000Z",
		"2006-01-02T15:04:05Z",
		"2006-01-02 15:04:05",
		"Mon Jan 02 15:04:05 -0700 2006",
	}

	for _, format := range formats {
		if t, err := time.Parse(format, dateStr); err == nil {
			return t.Format("20060102_150405")
		}
	}

	// Fallback: use empty string to indicate parsing failed
	return "00000000_000000"
}

// getExtension determines file extension from URL and type
func getExtension(mediaURL string, mediaType string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return ".jpg"
	}

	// Check format query param for Twitter images
	if format := parsedURL.Query().Get("format"); format != "" {
		return "." + format
	}

	// Get extension from path
	path := parsedURL.Path
	ext := filepath.Ext(path)
	if ext != "" {
		return ext
	}

	// Default based on type
	switch mediaType {
	case "video":
		return ".mp4"
	case "gif", "animated_gif":
		return ".mp4" // Twitter GIFs are actually MP4
	case "text":
		return ".txt"
	default:
		return ".jpg"
	}
}

// extractFilename extracts filename from URL (legacy)
func extractFilename(mediaURL string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return fmt.Sprintf("media_%d", time.Now().UnixNano())
	}

	// Get the path part
	path := parsedURL.Path

	// Extract base filename
	base := filepath.Base(path)

	// Handle Twitter image URLs: /media/XXX -> XXX.jpg
	if strings.Contains(mediaURL, "pbs.twimg.com/media/") {
		// Get format from query params
		format := parsedURL.Query().Get("format")
		if format == "" {
			format = "jpg"
		}

		// Remove any existing extension
		if idx := strings.LastIndex(base, "."); idx > 0 {
			base = base[:idx]
		}

		return base + "." + format
	}

	// Handle Twitter video URLs
	if strings.Contains(mediaURL, "video.twimg.com") {
		return base
	}

	// Default: use base name or generate one
	if base == "" || base == "." {
		return fmt.Sprintf("media_%d", time.Now().UnixNano())
	}

	return base
}

// downloadFile downloads a single file from URL
func downloadFile(client *http.Client, url, outputPath string) error {
	resp, err := client.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	out, err := os.Create(outputPath)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	return err
}
