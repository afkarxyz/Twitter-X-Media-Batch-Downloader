package main

import (
	"context"
	"encoding/json"
	"fmt"
	"path/filepath"
	"twitterxmediabatchdownloader/backend"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

// App struct
type App struct {
	ctx            context.Context
	downloadCtx    context.Context
	downloadCancel context.CancelFunc
}

// NewApp creates a new App application struct
func NewApp() *App {
	return &App{}
}

// startup is called when the app starts. The context is saved
// so we can call the runtime methods
func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	// Initialize database
	backend.InitDB()
	// Kill any leftover extractor processes from previous session
	backend.KillAllExtractorProcesses()
}

// shutdown is called when the app is closing
func (a *App) shutdown(ctx context.Context) {
	backend.CloseDB()
	// Kill any running extractor processes
	backend.KillAllExtractorProcesses()
}

// CleanupExtractorProcesses kills all running extractor processes
// Can be called from frontend when user wants to stop/reset
func (a *App) CleanupExtractorProcesses() {
	backend.KillAllExtractorProcesses()
}

// TimelineRequest represents the request structure for timeline extraction
type TimelineRequest struct {
	Username     string `json:"username"`
	AuthToken    string `json:"auth_token"`
	TimelineType string `json:"timeline_type"`
	BatchSize    int    `json:"batch_size"`
	Page         int    `json:"page"`
	MediaType    string `json:"media_type"`
	Retweets     bool   `json:"retweets"`
	Cursor       string `json:"cursor,omitempty"` // Resume from this cursor position
}

// DateRangeRequest represents the request structure for date range extraction
type DateRangeRequest struct {
	Username    string `json:"username"`
	AuthToken   string `json:"auth_token"`
	StartDate   string `json:"start_date"`
	EndDate     string `json:"end_date"`
	MediaFilter string `json:"media_filter"`
	Retweets    bool   `json:"retweets"`
}

// ExtractTimeline extracts media from user timeline
func (a *App) ExtractTimeline(req TimelineRequest) (string, error) {
	// Username not required for bookmarks only
	if req.Username == "" && req.TimelineType != "bookmarks" {
		return "", fmt.Errorf("username is required")
	}
	if req.AuthToken == "" {
		return "", fmt.Errorf("auth token is required")
	}

	backendReq := backend.TimelineRequest{
		Username:     req.Username,
		AuthToken:    req.AuthToken,
		TimelineType: req.TimelineType,
		BatchSize:    req.BatchSize,
		Page:         req.Page,
		MediaType:    req.MediaType,
		Retweets:     req.Retweets,
		Cursor:       req.Cursor,
	}

	response, err := backend.ExtractTimeline(backendReq)
	if err != nil {
		return "", fmt.Errorf("failed to extract timeline: %v", err)
	}

	jsonData, err := json.MarshalIndent(response, "", "  ")
	if err != nil {
		return "", fmt.Errorf("failed to encode response: %v", err)
	}

	return string(jsonData), nil
}

// ExtractDateRange extracts media based on date range
func (a *App) ExtractDateRange(req DateRangeRequest) (string, error) {
	if req.Username == "" {
		return "", fmt.Errorf("username is required")
	}
	if req.AuthToken == "" {
		return "", fmt.Errorf("auth token is required")
	}
	if req.StartDate == "" {
		return "", fmt.Errorf("start date is required")
	}
	if req.EndDate == "" {
		return "", fmt.Errorf("end date is required")
	}

	backendReq := backend.DateRangeRequest{
		Username:    req.Username,
		AuthToken:   req.AuthToken,
		StartDate:   req.StartDate,
		EndDate:     req.EndDate,
		MediaFilter: req.MediaFilter,
		Retweets:    req.Retweets,
	}

	response, err := backend.ExtractDateRange(backendReq)
	if err != nil {
		return "", fmt.Errorf("failed to extract date range: %v", err)
	}

	jsonData, err := json.MarshalIndent(response, "", "  ")
	if err != nil {
		return "", fmt.Errorf("failed to encode response: %v", err)
	}

	return string(jsonData), nil
}

// OpenFolder opens a folder in the file explorer
func (a *App) OpenFolder(path string) error {
	if path == "" {
		return fmt.Errorf("path is required")
	}

	// Clean the path to use correct separators for the OS
	cleanPath := filepath.Clean(path)

	err := backend.OpenFolderInExplorer(cleanPath)
	if err != nil {
		return fmt.Errorf("failed to open folder: %v", err)
	}

	return nil
}

// SelectFolder opens a folder selection dialog and returns the selected path
func (a *App) SelectFolder(defaultPath string) (string, error) {
	return backend.SelectFolderDialog(a.ctx, defaultPath)
}

// GetDefaults returns the default configuration
func (a *App) GetDefaults() map[string]string {
	return map[string]string{
		"downloadPath": backend.GetDefaultDownloadPath(),
	}
}

// Quit closes the application
func (a *App) Quit() {
	panic("quit")
}

// DownloadMediaRequest represents the request for downloading media (legacy)
type DownloadMediaRequest struct {
	URLs        []string `json:"urls"`
	OutputDir   string   `json:"output_dir"`
	Username    string   `json:"username"`
	Proxy       string   `json:"proxy,omitempty"` // Optional proxy URL (e.g., http://proxy:port or socks5://proxy:port)
}

// MediaItemRequest represents a media item with metadata
type MediaItemRequest struct {
	URL              string                `json:"url"`
	Date             string                `json:"date"`
	TweetID          backend.TweetIDString `json:"tweet_id"`
	Type             string                `json:"type"`
	Content          string                `json:"content,omitempty"` // Tweet text content (for text-only tweets)
	OriginalFilename string                `json:"original_filename,omitempty"` // Original filename from API
	AuthorUsername   string                `json:"author_username,omitempty"`   // Username of tweet author (for bookmarks and likes)
}

// DownloadMediaWithMetadataRequest represents the request for downloading media with metadata
type DownloadMediaWithMetadataRequest struct {
	Items     []MediaItemRequest `json:"items"`
	OutputDir string             `json:"output_dir"`
	Username  string             `json:"username"`
	Proxy     string             `json:"proxy,omitempty"` // Optional proxy URL (e.g., http://proxy:port or socks5://proxy:port)
}

// DownloadMediaResponse represents the response for download operation
type DownloadMediaResponse struct {
	Success    bool   `json:"success"`
	Downloaded int    `json:"downloaded"`
	Skipped    int    `json:"skipped"`
	Failed     int    `json:"failed"`
	Message    string `json:"message"`
}

// DownloadMedia downloads media files from URLs (legacy)
func (a *App) DownloadMedia(req DownloadMediaRequest) (DownloadMediaResponse, error) {
	if len(req.URLs) == 0 {
		return DownloadMediaResponse{
			Success: false,
			Message: "No URLs provided",
		}, fmt.Errorf("no URLs provided")
	}

	outputDir := req.OutputDir
	if outputDir == "" {
		outputDir = backend.GetDefaultDownloadPath()
	}

	// Create subfolder for username if provided
	if req.Username != "" {
		outputDir = filepath.Join(outputDir, req.Username)
	}

	downloaded, failed, err := backend.DownloadMediaFiles(req.URLs, outputDir, req.Proxy)
	if err != nil {
		return DownloadMediaResponse{
			Success:    false,
			Downloaded: downloaded,
			Skipped:    0,
			Failed:     failed,
			Message:    err.Error(),
		}, err
	}

	return DownloadMediaResponse{
		Success:    true,
		Downloaded: downloaded,
		Skipped:    0,
		Failed:     failed,
		Message:    fmt.Sprintf("Downloaded %d files, %d failed", downloaded, failed),
	}, nil
}

// DownloadProgress represents download progress event data
type DownloadProgress struct {
	Current int `json:"current"`
	Total   int `json:"total"`
	Percent int `json:"percent"`
}

// DownloadItemStatus represents per-item download status event data
type DownloadItemStatus struct {
	TweetID int64  `json:"tweet_id"`
	Index   int    `json:"index"`
	Status  string `json:"status"` // "success", "failed", "skipped"
}

// DownloadMediaWithMetadata downloads media files with proper naming and categorization
func (a *App) DownloadMediaWithMetadata(req DownloadMediaWithMetadataRequest) (DownloadMediaResponse, error) {
	if len(req.Items) == 0 {
		return DownloadMediaResponse{
			Success: false,
			Message: "No items provided",
		}, fmt.Errorf("no items provided")
	}

	outputDir := req.OutputDir
	if outputDir == "" {
		outputDir = backend.GetDefaultDownloadPath()
	}

		// Convert request items to backend items
		// For bookmarks and likes, use author_username from each item if available
		items := make([]backend.MediaItem, len(req.Items))
		for i, item := range req.Items {
			// Use original filename from API if available, otherwise extract from URL
			originalFilename := item.OriginalFilename
			if originalFilename == "" {
				// Fallback: extract from URL if not provided in API response
				originalFilename = backend.ExtractOriginalFilename(item.URL)
			}
			
			// For bookmarks and likes, use author_username from item, otherwise use req.Username
			username := req.Username
			if item.AuthorUsername != "" {
				username = item.AuthorUsername
			}
		
		items[i] = backend.MediaItem{
			URL:              item.URL,
			Date:             item.Date,
			TweetID:          int64(item.TweetID),
			Type:             item.Type,
			Username:         username,
			Content:          item.Content,
			OriginalFilename: originalFilename,
		}
	}

	// Create cancellable context
	a.downloadCtx, a.downloadCancel = context.WithCancel(context.Background())

	// Progress callback
	progressCallback := func(current, total int) {
		percent := 0
		if total > 0 {
			percent = (current * 100) / total
		}
		runtime.EventsEmit(a.ctx, "download-progress", DownloadProgress{
			Current: current,
			Total:   total,
			Percent: percent,
		})
	}

	// Per-item status callback
	itemStatusCallback := func(tweetID int64, index int, status string) {
		runtime.EventsEmit(a.ctx, "download-item-status", DownloadItemStatus{
			TweetID: tweetID,
			Index:   index,
			Status:  status,
		})
	}

	downloaded, skipped, failed, err := backend.DownloadMediaWithMetadataProgressAndStatus(items, outputDir, req.Username, progressCallback, itemStatusCallback, a.downloadCtx, req.Proxy)
	if err != nil {
		return DownloadMediaResponse{
			Success:    false,
			Downloaded: downloaded,
			Skipped:     skipped,
			Failed:     failed,
			Message:    err.Error(),
		}, err
	}

	// Clear cancel function
	a.downloadCancel = nil

	return DownloadMediaResponse{
		Success:    true,
		Downloaded: downloaded,
		Skipped:     skipped,
		Failed:     failed,
		Message:    fmt.Sprintf("Downloaded %d files, %d skipped, %d failed", downloaded, skipped, failed),
	}, nil
}

// StopDownload cancels the current download operation
func (a *App) StopDownload() bool {
	if a.downloadCancel != nil {
		a.downloadCancel()
		a.downloadCancel = nil
		return true
	}
	return false
}

// Database functions

// SaveAccountToDB saves account data to database
func (a *App) SaveAccountToDB(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string) error {
	return backend.SaveAccount(username, name, profileImage, totalMedia, responseJSON, mediaType)
}

// SaveAccountToDBWithStatus saves account data with cursor and completion status for resume capability
func (a *App) SaveAccountToDBWithStatus(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string, cursor string, completed bool) error {
	return backend.SaveAccountWithStatus(username, name, profileImage, totalMedia, responseJSON, mediaType, cursor, completed)
}

// GetAllAccountsFromDB returns all saved accounts
func (a *App) GetAllAccountsFromDB() ([]backend.AccountListItem, error) {
	return backend.GetAllAccounts()
}

// GetAccountFromDB returns account data by ID
func (a *App) GetAccountFromDB(id int64) (string, error) {
	acc, err := backend.GetAccountByID(id)
	if err != nil {
		return "", err
	}
	return acc.ResponseJSON, nil
}

// DeleteAccountFromDB deletes an account from database
func (a *App) DeleteAccountFromDB(id int64) error {
	return backend.DeleteAccount(id)
}

// ClearAllAccountsFromDB deletes all accounts from database
func (a *App) ClearAllAccountsFromDB() error {
	return backend.ClearAllAccounts()
}

// ExportAccountJSON exports account to JSON file in specified directory
func (a *App) ExportAccountJSON(id int64, outputDir string) (string, error) {
	return backend.ExportAccountToFile(id, outputDir)
}

// ExportAccountsTXT exports selected accounts to TXT file in specified directory
func (a *App) ExportAccountsTXT(ids []int64, outputDir string) (string, error) {
	return backend.ExportAccountsToTXT(ids, outputDir)
}

// UpdateAccountGroup updates the group for an account
func (a *App) UpdateAccountGroup(id int64, groupName, groupColor string) error {
	return backend.UpdateAccountGroup(id, groupName, groupColor)
}

// GetAllGroups returns all unique groups
func (a *App) GetAllGroups() ([]map[string]string, error) {
	return backend.GetAllGroups()
}

// FFmpeg functions

// IsFFmpegInstalled checks if ffmpeg is available
func (a *App) IsFFmpegInstalled() bool {
	return backend.IsFFmpegInstalled()
}

// DownloadFFmpeg downloads ffmpeg binary
func (a *App) DownloadFFmpeg() error {
	return backend.DownloadFFmpeg(nil)
}

// IsExifToolInstalled checks if exiftool is available
func (a *App) IsExifToolInstalled() bool {
	return backend.IsExifToolInstalled()
}

// DownloadExifTool downloads exiftool binary
func (a *App) DownloadExifTool() error {
	return backend.DownloadExifTool(nil)
}

// ConvertGIFsRequest represents request for converting GIFs
type ConvertGIFsRequest struct {
	FolderPath     string `json:"folder_path"`
	Quality        string `json:"quality"`    // "fast" or "better"
	Resolution     string `json:"resolution"` // "original", "high", "medium", "low"
	DeleteOriginal bool   `json:"delete_original"`
}

// ConvertGIFsResponse represents response for GIF conversion
type ConvertGIFsResponse struct {
	Success   bool   `json:"success"`
	Converted int    `json:"converted"`
	Failed    int    `json:"failed"`
	Message   string `json:"message"`
}

// ConvertGIFs converts MP4 files in gifs folder to actual GIF format
func (a *App) ConvertGIFs(req ConvertGIFsRequest) (ConvertGIFsResponse, error) {
	if !backend.IsFFmpegInstalled() {
		return ConvertGIFsResponse{
			Success: false,
			Message: "FFmpeg not installed. Please download it first.",
		}, nil
	}

	// Default values if not provided
	quality := req.Quality
	if quality == "" {
		quality = "fast"
	}
	resolution := req.Resolution
	if resolution == "" {
		resolution = "high"
	}

	converted, failed, err := backend.ConvertGIFsInFolder(req.FolderPath, quality, resolution, req.DeleteOriginal)
	if err != nil {
		return ConvertGIFsResponse{
			Success: false,
			Message: err.Error(),
		}, err
	}

	return ConvertGIFsResponse{
		Success:   true,
		Converted: converted,
		Failed:    failed,
		Message:   fmt.Sprintf("Converted %d GIFs, %d failed", converted, failed),
	}, nil
}

// ImportAccountResponse represents the response for import operation
type ImportAccountResponse struct {
	Success  bool   `json:"success"`
	Username string `json:"username"`
	Message  string `json:"message"`
}

// ImportAccountFromJSON imports account from JSON file (supports both old and new format)
func (a *App) ImportAccountFromJSON() (ImportAccountResponse, error) {
	// Open file dialog
	filePath, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Import Account JSON",
		Filters: []runtime.FileFilter{
			{DisplayName: "JSON Files", Pattern: "*.json"},
		},
	})
	if err != nil {
		return ImportAccountResponse{Success: false, Message: err.Error()}, err
	}

	// User cancelled
	if filePath == "" {
		return ImportAccountResponse{Success: false, Message: "Cancelled"}, nil
	}

	// Import the file
	username, err := backend.ImportAccountFromFile(filePath)
	if err != nil {
		return ImportAccountResponse{Success: false, Message: err.Error()}, err
	}

	return ImportAccountResponse{
		Success:  true,
		Username: username,
		Message:  fmt.Sprintf("Successfully imported @%s", username),
	}, nil
}
