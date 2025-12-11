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
	URL             string `json:"url"`
	Date            string `json:"date"`
	TweetID         int64  `json:"tweet_id"`
	Type            string `json:"type"`
	Username        string `json:"username"`
	Content         string `json:"content,omitempty"` // Tweet text content (for text-only tweets)
	OriginalFilename string `json:"original_filename,omitempty"` // Original Twitter media filename (15 char alphanumeric)
}

// DownloadMediaFiles downloads media files from URLs to the output directory (legacy)
func DownloadMediaFiles(urls []string, outputDir string, customProxy string) (downloaded int, failed int, err error) {
	// Create output directory if it doesn't exist
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		return 0, len(urls), fmt.Errorf("failed to create output directory: %v", err)
	}

	// Create HTTP client with proxy support
	client, err := CreateHTTPClient(customProxy, 60*time.Second)
	if err != nil {
		// If proxy setup fails, use default client without proxy
		client = &http.Client{
			Timeout: 60 * time.Second,
		}
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

// DownloadMediaWithMetadataProgressAndStatus downloads media files with progress and per-item status callbacks
// Returns: downloaded count, skipped count, failed count, error
func DownloadMediaWithMetadataProgressAndStatus(items []MediaItem, outputDir string, username string, progress ProgressCallback, itemStatus ItemStatusCallback, ctx context.Context, customProxy string) (downloaded int, skipped int, failed int, err error) {
	if ctx == nil {
		ctx = context.Background()
	}

	total := len(items)
	if total == 0 {
		return 0, 0, 0, nil
	}

	// Prepare all tasks first (sequential to handle tweet media count)
	// For bookmarks and likes, each item may have different username, so we track per username
	tweetMediaCount := make(map[string]map[int64]int) // username -> tweet_id -> count
	tasks := make([]downloadTask, 0, total)

	for i, item := range items {
		// Use item.Username if available (for bookmarks/likes with different authors), otherwise use provided username
		itemUsername := item.Username
		if itemUsername == "" {
			itemUsername = username
		}

		// Initialize tweet media count for this username if needed
		if tweetMediaCount[itemUsername] == nil {
			tweetMediaCount[itemUsername] = make(map[int64]int)
		}

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

		// Create base directory for this username
		baseDir := filepath.Join(outputDir, itemUsername)
		if err := os.MkdirAll(baseDir, 0755); err != nil {
			continue
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

		// Increment counter for this username and tweet_id
		tweetMediaCount[itemUsername][item.TweetID]++
		mediaIndex := tweetMediaCount[itemUsername][item.TweetID]

		// Create filename: {username}_{timestamp}_{tweet_id}_{index}.{ext}
		filename := fmt.Sprintf("%s_%s_%d_%02d%s", itemUsername, timestamp, item.TweetID, mediaIndex, ext)
		outputPath := filepath.Join(typeDir, filename)

		tasks = append(tasks, downloadTask{
			item:       item,
			outputPath: outputPath,
			index:      i,
		})
	}

	// Counters for parallel downloads
	var downloadedCount int64
	var skippedCount int64
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

	// Create HTTP client once for all workers (shared client is more efficient)
	var sharedClient *http.Client
	client, err := CreateHTTPClient(customProxy, 60*time.Second)
	if err != nil {
		// If proxy setup fails, use default client without proxy
		sharedClient = &http.Client{
			Timeout: 60 * time.Second,
		}
	} else {
		sharedClient = client
	}

	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			client := sharedClient

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
					status = "skipped"
					// Emit status immediately for skipped files
					if itemStatus != nil {
						itemStatus(task.item.TweetID, task.index, status)
					}
					atomic.AddInt64(&skippedCount, 1)
					continue // Skip to next task
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
					// Embed metadata after successful download
					tweetURL := fmt.Sprintf("https://x.com/i/status/%d", task.item.TweetID)
					// Always extract original filename from URL (simpler approach)
					originalFilename := ExtractOriginalFilename(task.item.URL)
					
					// For debugging: if original filename is still empty for video, it means it's not in the URL
					// This is acceptable - video URLs from Twitter may not contain original filename
					
					// Embed metadata (non-fatal: if it fails, file is still downloaded)
					if err := EmbedMetadata(task.outputPath, task.item.Content, tweetURL, originalFilename); err != nil {
						// Log error but don't fail the download
						// Metadata embedding is optional
					}
					
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
			return int(downloadedCount), int(skippedCount), int(failedCount) + (total - int(completedCount)), ctx.Err()
		case taskChan <- task:
		}
	}
	close(taskChan)

	// Wait for all workers to finish
	wg.Wait()

	return int(downloadedCount), int(skippedCount), int(failedCount), nil
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
