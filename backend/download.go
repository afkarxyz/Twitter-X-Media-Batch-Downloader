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
	MaxConcurrentDownloads = 10
)

type MediaItem struct {
	URL              string `json:"url"`
	Date             string `json:"date"`
	TweetID          int64  `json:"tweet_id"`
	Type             string `json:"type"`
	Username         string `json:"username"`
	Content          string `json:"content,omitempty"`
	OriginalFilename string `json:"original_filename,omitempty"`
}

func DownloadMediaFiles(urls []string, outputDir string, customProxy string) (downloaded int, failed int, err error) {

	if err := os.MkdirAll(outputDir, 0755); err != nil {
		return 0, len(urls), fmt.Errorf("failed to create output directory: %v", err)
	}

	client, err := CreateHTTPClient(customProxy, 60*time.Second)
	if err != nil {

		client = &http.Client{
			Timeout: 60 * time.Second,
		}
	}

	for _, mediaURL := range urls {
		filename := extractFilename(mediaURL)
		outputPath := filepath.Join(outputDir, filename)

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

type ProgressCallback func(current, total int)

type ItemStatusCallback func(tweetID int64, index int, status string) // status: "success", "failed", "skipped"

type downloadTask struct {
	item       MediaItem
	outputPath string
	index      int
}

func DownloadMediaWithMetadataProgressAndStatus(items []MediaItem, outputDir string, username string, progress ProgressCallback, itemStatus ItemStatusCallback, ctx context.Context, customProxy string) (downloaded int, skipped int, failed int, err error) {
	if ctx == nil {
		ctx = context.Background()
	}

	total := len(items)
	if total == 0 {
		return 0, 0, 0, nil
	}

	tweetMediaCount := make(map[string]map[int64]int)
	tasks := make([]downloadTask, 0, total)

	for i, item := range items {

		itemUsername := item.Username
		if itemUsername == "" {
			itemUsername = username
		}

		if tweetMediaCount[itemUsername] == nil {
			tweetMediaCount[itemUsername] = make(map[int64]int)
		}

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

		baseDir := filepath.Join(outputDir, itemUsername)
		if err := os.MkdirAll(baseDir, 0755); err != nil {
			continue
		}

		typeDir := filepath.Join(baseDir, subfolder)
		if err := os.MkdirAll(typeDir, 0755); err != nil {
			continue
		}

		timestamp := formatTimestamp(item.Date)

		ext := getExtension(item.URL, item.Type)

		tweetMediaCount[itemUsername][item.TweetID]++
		mediaIndex := tweetMediaCount[itemUsername][item.TweetID]

		filename := fmt.Sprintf("%s_%s_%d_%02d%s", itemUsername, timestamp, item.TweetID, mediaIndex, ext)
		outputPath := filepath.Join(typeDir, filename)

		tasks = append(tasks, downloadTask{
			item:       item,
			outputPath: outputPath,
			index:      i,
		})
	}

	var downloadedCount int64
	var skippedCount int64
	var failedCount int64
	var completedCount int64

	taskChan := make(chan downloadTask, len(tasks))
	var wg sync.WaitGroup

	numWorkers := MaxConcurrentDownloads
	if numWorkers > len(tasks) {
		numWorkers = len(tasks)
	}

	var sharedClient *http.Client
	client, err := CreateHTTPClient(customProxy, 60*time.Second)
	if err != nil {

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

				select {
				case <-ctx.Done():
					return
				default:
				}

				var status string
				markCompleted := func() {
					completed := atomic.AddInt64(&completedCount, 1)
					if progress != nil {
						progress(int(completed), total)
					}
				}

				if _, err := os.Stat(task.outputPath); err == nil {
					status = "skipped"

					if itemStatus != nil {
						itemStatus(task.item.TweetID, task.index, status)
					}
					atomic.AddInt64(&skippedCount, 1)
					markCompleted()
					continue
				} else if task.item.Type == "text" {

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

					tweetURL := fmt.Sprintf("https://x.com/i/status/%d", task.item.TweetID)

					originalFilename := ExtractOriginalFilename(task.item.URL)

					if err := EmbedMetadata(task.outputPath, task.item.Content, tweetURL, originalFilename); err != nil {

					}

					atomic.AddInt64(&downloadedCount, 1)
					status = "success"
				}

				if itemStatus != nil {
					itemStatus(task.item.TweetID, task.index, status)
				}

				markCompleted()
			}
		}()
	}

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

	wg.Wait()

	return int(downloadedCount), int(skippedCount), int(failedCount), nil
}

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

func formatTimestamp(dateStr string) string {

	formats := []string{
		"2006-01-02T15:04:05",
		"2006-01-02T15:04:05+00:00",
		"2006-01-02T15:04:05-07:00",
		time.RFC3339,
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

	return "00000000_000000"
}

func getExtension(mediaURL string, mediaType string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return ".jpg"
	}

	if format := parsedURL.Query().Get("format"); format != "" {
		return "." + format
	}

	path := parsedURL.Path
	ext := filepath.Ext(path)
	if ext != "" {
		return ext
	}

	switch mediaType {
	case "video":
		return ".mp4"
	case "gif", "animated_gif":
		return ".mp4"
	case "text":
		return ".txt"
	default:
		return ".jpg"
	}
}

func extractFilename(mediaURL string) string {
	parsedURL, err := url.Parse(mediaURL)
	if err != nil {
		return fmt.Sprintf("media_%d", time.Now().UnixNano())
	}

	path := parsedURL.Path

	base := filepath.Base(path)

	if strings.Contains(mediaURL, "pbs.twimg.com/media/") {

		format := parsedURL.Query().Get("format")
		if format == "" {
			format = "jpg"
		}

		if idx := strings.LastIndex(base, "."); idx > 0 {
			base = base[:idx]
		}

		return base + "." + format
	}

	if strings.Contains(mediaURL, "video.twimg.com") {
		return base
	}

	if base == "" || base == "." {
		return fmt.Sprintf("media_%d", time.Now().UnixNano())
	}

	return base
}

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
