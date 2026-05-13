package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"twitterxmediabatchdownloader/backend"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

type App struct {
	ctx            context.Context
	downloadCtx    context.Context
	downloadCancel context.CancelFunc
}

func NewApp() *App {
	return &App{}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx

	if err := backend.InitDB(); err != nil {
		runtime.LogErrorf(ctx, "failed to initialize database: %v", err)
	}

	backend.KillAllExtractorProcesses()
}

func (a *App) shutdown(ctx context.Context) {
	backend.CloseDB()

	backend.KillAllExtractorProcesses()
}

func (a *App) CleanupExtractorProcesses() {
	backend.KillAllExtractorProcesses()
}

type TimelineRequest struct {
	Username     string `json:"username"`
	AuthToken    string `json:"auth_token"`
	TimelineType string `json:"timeline_type"`
	BatchSize    int    `json:"batch_size"`
	Page         int    `json:"page"`
	MediaType    string `json:"media_type"`
	Retweets     bool   `json:"retweets"`
	Cursor       string `json:"cursor,omitempty"`
}

type DateRangeRequest struct {
	Username    string `json:"username"`
	AuthToken   string `json:"auth_token"`
	StartDate   string `json:"start_date"`
	EndDate     string `json:"end_date"`
	MediaFilter string `json:"media_filter"`
	Retweets    bool   `json:"retweets"`
}

func (a *App) ExtractTimeline(req TimelineRequest) (string, error) {

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

func (a *App) OpenFolder(path string) error {
	if path == "" {
		return fmt.Errorf("path is required")
	}

	cleanPath := filepath.Clean(path)

	err := backend.OpenFolderInExplorer(cleanPath)
	if err != nil {
		return fmt.Errorf("failed to open folder: %v", err)
	}

	return nil
}

func (a *App) SelectFolder(defaultPath string) (string, error) {
	return backend.SelectFolderDialog(a.ctx, defaultPath)
}

func (a *App) GetDefaults() map[string]string {
	return map[string]string{
		"downloadPath": backend.GetDefaultDownloadPath(),
	}
}

func (a *App) GetStoredAuthToken(slot string) (string, error) {
	return backend.GetStoredAuthToken(slot)
}

func (a *App) SetStoredAuthToken(slot, token string) error {
	return backend.SetStoredAuthToken(slot, token)
}

func (a *App) ClearStoredAuthToken(slot string) error {
	return backend.ClearStoredAuthToken(slot)
}

func (a *App) IsExtractorInstalled() bool {
	return backend.IsExtractorInstalled()
}

func (a *App) DownloadExtractor() error {
	return backend.DownloadExtractor(nil)
}

func (a *App) GetExtractorVersionStatus() backend.ExtractorVersionStatus {
	return backend.GetExtractorVersionStatus()
}

func (a *App) Quit() {
	panic("quit")
}

type DownloadMediaRequest struct {
	URLs      []string `json:"urls"`
	OutputDir string   `json:"output_dir"`
	Username  string   `json:"username"`
	Proxy     string   `json:"proxy,omitempty"`
}

type MediaItemRequest struct {
	URL              string                `json:"url"`
	Date             string                `json:"date"`
	TweetID          backend.TweetIDString `json:"tweet_id"`
	Type             string                `json:"type"`
	Content          string                `json:"content,omitempty"`
	OriginalFilename string                `json:"original_filename,omitempty"`
	AuthorUsername   string                `json:"author_username,omitempty"`
}

type DownloadMediaWithMetadataRequest struct {
	Items     []MediaItemRequest `json:"items"`
	OutputDir string             `json:"output_dir"`
	Username  string             `json:"username"`
	Proxy     string             `json:"proxy,omitempty"`
}

type DownloadMediaResponse struct {
	Success    bool   `json:"success"`
	Cancelled  bool   `json:"cancelled"`
	Downloaded int    `json:"downloaded"`
	Skipped    int    `json:"skipped"`
	Failed     int    `json:"failed"`
	Message    string `json:"message"`
}

func (a *App) DownloadMedia(req DownloadMediaRequest) (DownloadMediaResponse, error) {
	if len(req.URLs) == 0 {
		return DownloadMediaResponse{
			Success:   false,
			Cancelled: false,
			Message:   "No URLs provided",
		}, fmt.Errorf("no URLs provided")
	}

	outputDir := req.OutputDir
	if outputDir == "" {
		outputDir = backend.GetDefaultDownloadPath()
	}

	if req.Username != "" {
		outputDir = filepath.Join(outputDir, req.Username)
	}

	downloaded, failed, err := backend.DownloadMediaFiles(req.URLs, outputDir, req.Proxy)
	if err != nil {
		return DownloadMediaResponse{
			Success:    false,
			Cancelled:  false,
			Downloaded: downloaded,
			Skipped:    0,
			Failed:     failed,
			Message:    err.Error(),
		}, err
	}

	return DownloadMediaResponse{
		Success:    true,
		Cancelled:  false,
		Downloaded: downloaded,
		Skipped:    0,
		Failed:     failed,
		Message:    fmt.Sprintf("Downloaded %d files, %d failed", downloaded, failed),
	}, nil
}

type DownloadProgress struct {
	Current int `json:"current"`
	Total   int `json:"total"`
	Percent int `json:"percent"`
}

type DownloadItemStatus struct {
	TweetID int64  `json:"tweet_id"`
	Index   int    `json:"index"`
	Status  string `json:"status"`
}

func (a *App) DownloadMediaWithMetadata(req DownloadMediaWithMetadataRequest) (DownloadMediaResponse, error) {
	if len(req.Items) == 0 {
		return DownloadMediaResponse{
			Success:   false,
			Cancelled: false,
			Message:   "No items provided",
		}, fmt.Errorf("no items provided")
	}

	outputDir := req.OutputDir
	if outputDir == "" {
		outputDir = backend.GetDefaultDownloadPath()
	}

	items := make([]backend.MediaItem, len(req.Items))
	for i, item := range req.Items {

		originalFilename := item.OriginalFilename
		if originalFilename == "" {

			originalFilename = backend.ExtractOriginalFilename(item.URL)
		}

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

	a.downloadCtx, a.downloadCancel = context.WithCancel(context.Background())
	defer func() {
		a.downloadCtx = nil
		a.downloadCancel = nil
	}()

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

	itemStatusCallback := func(tweetID int64, index int, status string) {
		runtime.EventsEmit(a.ctx, "download-item-status", DownloadItemStatus{
			TweetID: tweetID,
			Index:   index,
			Status:  status,
		})
	}

	downloaded, skipped, failed, err := backend.DownloadMediaWithMetadataProgressAndStatus(items, outputDir, req.Username, progressCallback, itemStatusCallback, a.downloadCtx, req.Proxy)
	if err != nil {
		if errors.Is(err, context.Canceled) {
			return DownloadMediaResponse{
				Success:    false,
				Cancelled:  true,
				Downloaded: downloaded,
				Skipped:    skipped,
				Failed:     failed,
				Message:    "Download stopped",
			}, nil
		}

		return DownloadMediaResponse{
			Success:    false,
			Cancelled:  false,
			Downloaded: downloaded,
			Skipped:    skipped,
			Failed:     failed,
			Message:    err.Error(),
		}, err
	}

	return DownloadMediaResponse{
		Success:    true,
		Cancelled:  false,
		Downloaded: downloaded,
		Skipped:    skipped,
		Failed:     failed,
		Message:    fmt.Sprintf("Downloaded %d files, %d skipped, %d failed", downloaded, skipped, failed),
	}, nil
}

func (a *App) StopDownload() bool {
	if a.downloadCancel != nil {
		a.downloadCancel()
		a.downloadCancel = nil
		return true
	}
	return false
}

func (a *App) SaveAccountToDB(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string) error {
	return backend.SaveAccount(username, name, profileImage, totalMedia, responseJSON, mediaType)
}

func (a *App) SaveAccountToDBWithStatus(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string, cursor string, completed bool) error {
	return backend.SaveAccountWithStatus(username, name, profileImage, totalMedia, responseJSON, mediaType, cursor, completed)
}

func (a *App) GetAllAccountsFromDB() ([]backend.AccountListItem, error) {
	return backend.GetAllAccounts()
}

func (a *App) GetAccountFromDB(id int64) (string, error) {
	acc, err := backend.GetAccountByID(id)
	if err != nil {
		return "", err
	}
	return acc.ResponseJSON, nil
}

func (a *App) GetSavedAccountFromDB(username, mediaType string) (string, error) {
	acc, err := backend.GetAccountByUsernameAndMediaType(username, mediaType)
	if err != nil {
		return "", err
	}
	return acc.ResponseJSON, nil
}

func (a *App) DeleteAccountFromDB(id int64) error {
	return backend.DeleteAccount(id)
}

func (a *App) ClearAllAccountsFromDB() error {
	return backend.ClearAllAccounts()
}

func (a *App) ExportAccountJSON(id int64, outputDir string) (string, error) {
	return backend.ExportAccountToFile(id, outputDir)
}

func (a *App) ExportAccountsTXT(ids []int64, outputDir string) (string, error) {
	return backend.ExportAccountsToTXT(ids, outputDir)
}

func (a *App) UpdateAccountGroup(id int64, groupName, groupColor string) error {
	return backend.UpdateAccountGroup(id, groupName, groupColor)
}

func (a *App) GetAllGroups() ([]map[string]string, error) {
	return backend.GetAllGroups()
}

func (a *App) CheckFoldersExist(basePath string, usernames []string) map[string]bool {
	return backend.CheckFoldersExist(basePath, usernames)
}

func (a *App) IsFFmpegInstalled() bool {
	return backend.IsFFmpegInstalled()
}

func (a *App) DownloadFFmpeg() error {
	return backend.DownloadFFmpeg(nil)
}

func (a *App) GetFFmpegVersionStatus() backend.DependencyVersionStatus {
	return backend.GetFFmpegVersionStatus()
}

func (a *App) IsExifToolInstalled() bool {
	return backend.IsExifToolInstalled()
}

func (a *App) DownloadExifTool() error {
	return backend.DownloadExifTool(nil)
}

func (a *App) GetExifToolVersionStatus() backend.DependencyVersionStatus {
	return backend.GetExifToolVersionStatus()
}

type ConvertGIFsRequest struct {
	FolderPath     string `json:"folder_path"`
	Quality        string `json:"quality"`
	Resolution     string `json:"resolution"`
	DeleteOriginal bool   `json:"delete_original"`
}

type ConvertGIFsResponse struct {
	Success   bool   `json:"success"`
	Converted int    `json:"converted"`
	Failed    int    `json:"failed"`
	Message   string `json:"message"`
}

func (a *App) ConvertGIFs(req ConvertGIFsRequest) (ConvertGIFsResponse, error) {
	if !backend.IsFFmpegInstalled() {
		return ConvertGIFsResponse{
			Success: false,
			Message: "FFmpeg not installed. Please download it first.",
		}, nil
	}

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

type ImportAccountResponse struct {
	Success  bool   `json:"success"`
	Username string `json:"username"`
	Message  string `json:"message"`
}

func (a *App) CheckFolderExists(basePath, username string) bool {
	return backend.CheckFolderExists(basePath, username)
}

func (a *App) CheckGifsFolderExists(basePath, username string) bool {
	return backend.CheckGifsFolderExists(basePath, username)
}

func (a *App) CheckGifsFolderHasMP4(basePath, username string) bool {
	return backend.CheckGifsFolderHasMP4(basePath, username)
}

func (a *App) GetFolderPath(basePath, username string) string {
	return backend.GetFolderPath(basePath, username)
}

func (a *App) GetGifsFolderPath(basePath, username string) string {
	return backend.GetGifsFolderPath(basePath, username)
}

func (a *App) ImportAccountFromJSON() (ImportAccountResponse, error) {

	filePath, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Import Account JSON",
		Filters: []runtime.FileFilter{
			{DisplayName: "JSON Files", Pattern: "*.json"},
		},
	})
	if err != nil {
		return ImportAccountResponse{Success: false, Message: err.Error()}, err
	}

	if filePath == "" {
		return ImportAccountResponse{Success: false, Message: "Cancelled"}, nil
	}

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
