package backend

import (
	"context"
	"errors"
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
	DefaultConcurrentDownloads = 10
	MaxConcurrentDownloads     = 50
	DefaultRetryAttempts       = 2
	MaxRetryAttempts           = 5
)

func NormalizeConcurrentDownloads(requested int) int {
	if requested <= 0 {
		return DefaultConcurrentDownloads
	}
	if requested > MaxConcurrentDownloads {
		return MaxConcurrentDownloads
	}
	return requested
}

func NormalizeRetryAttempts(requested int) int {
	if requested < 0 {
		return 0
	}
	if requested > MaxRetryAttempts {
		return MaxRetryAttempts
	}
	return requested
}

type DownloadOptions struct {
	ConcurrentDownloads   int
	SkipExistingFiles     bool
	DeleteIncompleteFiles bool
	RetryAttempts         int
	FilenameTemplate      string
	FolderTemplate        string
	AutoConvertGIFs       bool
	GIFQuality            string
	GIFResolution         string
}

func NormalizeDownloadOptions(options DownloadOptions) DownloadOptions {
	options.ConcurrentDownloads = NormalizeConcurrentDownloads(options.ConcurrentDownloads)
	options.RetryAttempts = NormalizeRetryAttempts(options.RetryAttempts)
	return options
}

type MediaItem struct {
	URL              string `json:"url"`
	Date             string `json:"date"`
	TweetID          int64  `json:"tweet_id"`
	Type             string `json:"type"`
	Username         string `json:"username"`
	AccountName      string `json:"account_name,omitempty"`
	Content          string `json:"content,omitempty"`
	OriginalFilename string `json:"original_filename,omitempty"`
}

func DownloadMediaFiles(urls []string, outputDir string, options DownloadOptions, customProxy string) (downloaded int, failed int, err error) {
	options = NormalizeDownloadOptions(options)

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

		if options.SkipExistingFiles {
			if _, err := os.Stat(outputPath); err == nil {
				downloaded++
				continue
			}
		}

		if err := downloadFileWithRetry(context.Background(), client, mediaURL, outputPath, options); err != nil {
			failed++
			continue
		}
		downloaded++
	}

	return downloaded, failed, nil
}

func fileExists(path string) (bool, error) {
	_, err := os.Stat(path)
	if err == nil {
		return true, nil
	}
	if errors.Is(err, os.ErrNotExist) {
		return false, nil
	}
	return false, err
}

func cleanupIncompleteDownload(outputPath string) {
	_ = os.Remove(outputPath)
}

func replaceFile(tempPath, outputPath string, allowOverwrite bool) error {
	if !allowOverwrite {
		return os.Rename(tempPath, outputPath)
	}

	exists, err := fileExists(outputPath)
	if err != nil {
		return err
	}
	if !exists {
		return os.Rename(tempPath, outputPath)
	}

	backupPath := fmt.Sprintf("%s.bak.%d", outputPath, time.Now().UnixNano())
	if err := os.Rename(outputPath, backupPath); err != nil {
		return err
	}

	if err := os.Rename(tempPath, outputPath); err != nil {
		restoreErr := os.Rename(backupPath, outputPath)
		if restoreErr != nil {
			return fmt.Errorf("failed to replace existing file: %v (restore failed: %v)", err, restoreErr)
		}
		return err
	}

	_ = os.Remove(backupPath)
	return nil
}

func writeFileAtomically(outputPath string, data []byte, allowOverwrite bool, keepPartialOnFailure bool) error {
	out, err := os.CreateTemp(filepath.Dir(outputPath), filepath.Base(outputPath)+".*.part")
	if err != nil {
		return err
	}
	tempPath := out.Name()
	cleanupTemp := !keepPartialOnFailure
	defer func() {
		if out != nil {
			_ = out.Close()
		}
		if cleanupTemp {
			_ = os.Remove(tempPath)
		}
	}()

	if _, err := out.Write(data); err != nil {
		return err
	}

	if err := out.Close(); err != nil {
		return err
	}
	out = nil

	if err := replaceFile(tempPath, outputPath, allowOverwrite); err != nil {
		return err
	}

	cleanupTemp = false
	return nil
}

func waitForRetry(ctx context.Context, attempt int) error {
	timer := time.NewTimer(time.Duration(attempt) * 500 * time.Millisecond)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

func downloadFileWithRetry(ctx context.Context, client *http.Client, mediaURL, outputPath string, options DownloadOptions) error {
	options = NormalizeDownloadOptions(options)

	var lastErr error
	totalAttempts := options.RetryAttempts + 1
	for attempt := 1; attempt <= totalAttempts; attempt++ {
		keepPartialOnFailure := !options.DeleteIncompleteFiles && attempt == totalAttempts
		lastErr = downloadFileWithContext(ctx, client, mediaURL, outputPath, !options.SkipExistingFiles, keepPartialOnFailure)
		if lastErr == nil {
			return nil
		}
		if ctx.Err() != nil || errors.Is(lastErr, context.Canceled) {
			if options.DeleteIncompleteFiles {
				cleanupIncompleteDownload(outputPath)
			}
			return lastErr
		}
		if options.DeleteIncompleteFiles {
			cleanupIncompleteDownload(outputPath)
		}
		if attempt < totalAttempts {
			if err := waitForRetry(ctx, attempt); err != nil {
				if options.DeleteIncompleteFiles {
					cleanupIncompleteDownload(outputPath)
				}
				return err
			}
		}
	}

	return lastErr
}

func writeTextFileWithOptions(outputPath string, content string, options DownloadOptions) error {
	keepPartialOnFailure := !options.DeleteIncompleteFiles
	if err := writeFileAtomically(outputPath, []byte(content), !options.SkipExistingFiles, keepPartialOnFailure); err != nil {
		if options.DeleteIncompleteFiles {
			cleanupIncompleteDownload(outputPath)
		}
		return err
	}
	return nil
}

type ProgressCallback func(current, total int)

type ItemStatusCallback func(tweetID int64, index int, status string) // status: "success", "failed", "skipped"

type downloadTask struct {
	item       MediaItem
	outputPath string
	index      int
}

type pendingDownloadTask struct {
	task     downloadTask
	typeDir  string
	baseName string
	ext      string
}

func DownloadMediaWithMetadataProgressAndStatus(items []MediaItem, outputDir string, username string, progress ProgressCallback, itemStatus ItemStatusCallback, ctx context.Context, options DownloadOptions, customProxy string) (downloaded int, skipped int, failed int, err error) {
	if ctx == nil {
		ctx = context.Background()
	}
	options = NormalizeDownloadOptions(options)

	total := len(items)
	if total == 0 {
		return 0, 0, 0, nil
	}

	tweetMediaCount := make(map[string]map[int64]int)
	pendingTasks := make([]pendingDownloadTask, 0, total)
	filenameCounts := make(map[string]int)
	downloadDate := time.Now().Format("20060102")

	for i, item := range items {

		itemUsername := item.Username
		if itemUsername == "" {
			itemUsername = username
		}
		itemAccountName := item.AccountName
		if itemAccountName == "" {
			itemAccountName = itemUsername
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

		baseDir := filepath.Join(outputDir, renderFolder(options.FolderTemplate, itemUsername, itemAccountName, downloadDate))
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

		mediaID := item.OriginalFilename
		if mediaID == "" {
			mediaID = ExtractOriginalFilename(item.URL)
		}

		baseName := renderFilename(options.FilenameTemplate, itemUsername, itemAccountName, timestamp, item.TweetID, item.Type, mediaIndex, mediaID)
		filenameCounts[filenameCollisionKey(typeDir, baseName, ext)]++

		pendingTasks = append(pendingTasks, pendingDownloadTask{
			task: downloadTask{
				item:  item,
				index: i,
			},
			typeDir:  typeDir,
			baseName: baseName,
			ext:      ext,
		})
	}

	filenameIndexes := make(map[string]int, len(filenameCounts))
tasks := make([]downloadTask, 0, len(pendingTasks))

for _, pending := range pendingTasks {
	key := filenameCollisionKey(pending.typeDir, pending.baseName, pending.ext)

	filenameIndexes[key]++

	filename := formatDownloadFilename(
		pending.baseName,
		pending.ext,
		filenameCounts[key],
		filenameIndexes[key],
	)

	// If skipping existing files, first check the canonical filename.
	// This prevents existing files from being downloaded again as _01, _02, etc.
	if options.SkipExistingFiles {
		canonicalPath := filepath.Join(pending.typeDir, formatDownloadFilename(pending.baseName, pending.ext, 1, 1))
		if exists, err := fileExists(canonicalPath); err == nil && exists {
			pending.task.outputPath = canonicalPath
			tasks = append(tasks, pending.task)
			continue
		}
	}

	pending.task.outputPath = filepath.Join(pending.typeDir, filename)
	tasks = append(tasks, pending.task)
}

	var downloadedCount int64
	var skippedCount int64
	var failedCount int64
	var completedCount int64
	var conversionWG sync.WaitGroup

	taskChan := make(chan downloadTask, len(tasks))
	var wg sync.WaitGroup

	numWorkers := options.ConcurrentDownloads
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
				markCompleted := func() {
					completed := atomic.AddInt64(&completedCount, 1)
					if progress != nil {
						progress(int(completed), total)
					}
				}

				if ctx.Err() != nil {
					if itemStatus != nil {
						itemStatus(task.item.TweetID, task.index, "cancelled")
					}
					markCompleted()
					continue
				}

				var status string

				if options.SkipExistingFiles {
					exists, err := fileExists(task.outputPath)
					if err == nil && exists {
						status = "skipped"

						if itemStatus != nil {
							itemStatus(task.item.TweetID, task.index, status)
						}
						atomic.AddInt64(&skippedCount, 1)
						markCompleted()
						continue
					}
				}

				if task.item.Type == "text" {
					if ctx.Err() != nil {
						if itemStatus != nil {
							itemStatus(task.item.TweetID, task.index, "cancelled")
						}
						markCompleted()
						continue
					}

					if err := writeTextFileWithOptions(task.outputPath, task.item.Content, options); err != nil {
						atomic.AddInt64(&failedCount, 1)
						status = "failed"
					} else {
						atomic.AddInt64(&downloadedCount, 1)
						status = "success"
					}
				} else if err := downloadFileWithRetry(ctx, client, task.item.URL, task.outputPath, options); err != nil {
					if ctx.Err() != nil || errors.Is(err, context.Canceled) {
						if itemStatus != nil {
							itemStatus(task.item.TweetID, task.index, "cancelled")
						}
						markCompleted()
						continue
					}
					atomic.AddInt64(&failedCount, 1)
					status = "failed"
				} else {

					tweetURL := fmt.Sprintf("https://x.com/i/status/%d", task.item.TweetID)

					originalFilename := ExtractOriginalFilename(task.item.URL)

					if ctx.Err() == nil {
						if err := EmbedMetadata(ctx, task.outputPath, task.item.Content, tweetURL, originalFilename); err != nil && !errors.Is(err, context.Canceled) {

						}
					}

					atomic.AddInt64(&downloadedCount, 1)
					status = "success"

					if shouldAutoConvertGIF(task.item.Type, options) && ctx.Err() == nil {
						conversionWG.Add(1)
						go func(inputPath string) {
							defer conversionWG.Done()
							outputPath := strings.TrimSuffix(inputPath, filepath.Ext(inputPath)) + ".gif"
							_ = ConvertMP4ToGIFWithContext(ctx, inputPath, outputPath, normalizeGIFQuality(options.GIFQuality), normalizeGIFResolution(options.GIFResolution))
						}(task.outputPath)
					}
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
			return int(downloadedCount), int(skippedCount), int(failedCount), ctx.Err()
		case taskChan <- task:
		}
	}
	close(taskChan)

	wg.Wait()
	conversionWG.Wait()

	if err := ctx.Err(); err != nil {
		return int(downloadedCount), int(skippedCount), int(failedCount), err
	}

	return int(downloadedCount), int(skippedCount), int(failedCount), nil
}

func shouldAutoConvertGIF(mediaType string, options DownloadOptions) bool {
	if !options.AutoConvertGIFs || !IsFFmpegInstalled() {
		return false
	}
	switch mediaType {
	case "gif", "animated_gif":
		return true
	default:
		return false
	}
}

func normalizeGIFQuality(quality string) string {
	if quality == "better" {
		return quality
	}
	return "fast"
}

func normalizeGIFResolution(resolution string) string {
	switch resolution {
	case "original", "high", "medium", "low":
		return resolution
	default:
		return "original"
	}
}

func downloadFileWithContext(ctx context.Context, client *http.Client, url, outputPath string, allowOverwrite bool, keepPartialOnFailure bool) error {
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

	out, err := os.CreateTemp(filepath.Dir(outputPath), filepath.Base(outputPath)+".*.part")
	if err != nil {
		return err
	}
	tempPath := out.Name()
	cleanupTemp := !keepPartialOnFailure
	defer func() {
		if out != nil {
			_ = out.Close()
		}
		if cleanupTemp {
			_ = os.Remove(tempPath)
		}
	}()

	_, err = io.Copy(out, resp.Body)
	if err != nil {
		return err
	}

	if err := out.Close(); err != nil {
		return err
	}
	out = nil

	if ctx.Err() != nil {
		return ctx.Err()
	}

	if err := replaceFile(tempPath, outputPath, allowOverwrite); err != nil {
		return err
	}

	cleanupTemp = false
	return nil
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

func renderFilename(template, username, accountName, timestamp string, tweetID int64, mediaType string, index int, mediaID string) string {
	if strings.TrimSpace(template) == "" {
		template = "{handle}_{date}_{tweet_id}"
	}
	replacer := strings.NewReplacer(
		"{handle}", sanitizeFilenamePart(username),
		"{name}", sanitizeFilenamePart(accountName),
		"{username}", sanitizeFilenamePart(username),
		"{account_name}", sanitizeFilenamePart(accountName),
		"{date}", timestamp,
		"{tweet_id}", fmt.Sprintf("%d", tweetID),
		"{index}", fmt.Sprintf("%02d", index),
		"{media_id}", sanitizeFilenamePart(mediaID),
		"{type}", sanitizeFilenamePart(mediaType),
	)
	name := strings.TrimSpace(replacer.Replace(template))
	name = sanitizeFilenamePart(name)
	if name == "" {
		name = sanitizeFilenamePart(fmt.Sprintf("%s_%s_%d", username, timestamp, tweetID))
	}
	return name
}

func filenameCollisionKey(dir, baseName, ext string) string {
	return dir + "\x00" + baseName + ext
}

func formatDownloadFilename(baseName, ext string, duplicateCount, duplicateIndex int) string {
	if duplicateCount > 1 {
		return fmt.Sprintf("%s_%02d%s", baseName, duplicateIndex, ext)
	}
	return baseName + ext
}

func renderFolder(template, username, accountName, date string) string {
	if strings.TrimSpace(template) == "" {
		template = "{handle}"
	}
	replacer := strings.NewReplacer(
		"{handle}", sanitizeFilenamePart(username),
		"{name}", sanitizeFilenamePart(accountName),
		"{username}", sanitizeFilenamePart(username),
		"{account_name}", sanitizeFilenamePart(accountName),
		"{date}", date,
	)
	name := strings.TrimSpace(replacer.Replace(template))
	name = sanitizeFilenamePart(name)
	if name == "" {
		name = sanitizeFilenamePart(username)
	}
	return name
}

func sanitizeFilenamePart(s string) string {
	cleaned := strings.TrimSpace(strings.Map(func(r rune) rune {
		switch r {
		case '\\', '/', ':', '*', '?', '"', '<', '>', '|':
			return -1
		}
		if r >= 0 && r < 32 {
			return -1
		}
		return r
	}, s))
	cleaned = strings.TrimRight(cleaned, ". ")
	if isWindowsReservedName(cleaned) {
		return cleaned + "_"
	}
	return cleaned
}

func isWindowsReservedName(name string) bool {
	base := name
	if dotIndex := strings.IndexByte(base, '.'); dotIndex >= 0 {
		base = base[:dotIndex]
	}
	base = strings.ToUpper(strings.TrimSpace(base))
	switch base {
	case "CON", "PRN", "AUX", "NUL":
		return true
	}
	return len(base) == 4 &&
		((strings.HasPrefix(base, "COM") && base[3] >= '1' && base[3] <= '9') ||
			(strings.HasPrefix(base, "LPT") && base[3] >= '1' && base[3] <= '9'))
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
	return downloadFileWithRetry(context.Background(), client, url, outputPath, DownloadOptions{
		SkipExistingFiles:     false,
		DeleteIncompleteFiles: true,
		RetryAttempts:         0,
	})
}

func DownloadProfileImage(imageURL, outputDir, username, kind, customProxy string) (string, error) {
	if strings.TrimSpace(imageURL) == "" {
		return "", fmt.Errorf("no %s url available", kind)
	}

	baseDir := outputDir
	if username != "" {
		baseDir = filepath.Join(outputDir, username)
	}
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create output directory: %v", err)
	}

	ext := getExtension(imageURL, "photo")
	filename := fmt.Sprintf("%s_%s%s", sanitizeFilenamePart(username), kind, ext)
	outputPath := filepath.Join(baseDir, filename)

	client, err := CreateHTTPClient(customProxy, 60*time.Second)
	if err != nil {
		client = &http.Client{Timeout: 60 * time.Second}
	}

	if err := downloadFile(client, imageURL, outputPath); err != nil {
		return "", fmt.Errorf("failed to download %s: %v", kind, err)
	}
	return outputPath, nil
}
