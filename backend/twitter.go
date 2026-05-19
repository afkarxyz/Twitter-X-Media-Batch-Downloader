package backend

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
)

var activeExtractorCommands = struct {
	sync.Mutex
	byPID map[int]*exec.Cmd
}{
	byPID: make(map[int]*exec.Cmd),
}

func registerExtractorCommand(cmd *exec.Cmd) {
	if cmd == nil || cmd.Process == nil {
		return
	}

	activeExtractorCommands.Lock()
	activeExtractorCommands.byPID[cmd.Process.Pid] = cmd
	activeExtractorCommands.Unlock()
}

func unregisterExtractorCommand(cmd *exec.Cmd) {
	if cmd == nil || cmd.Process == nil {
		return
	}

	activeExtractorCommands.Lock()
	delete(activeExtractorCommands.byPID, cmd.Process.Pid)
	activeExtractorCommands.Unlock()
}

func runTrackedExtractorCommand(cmd *exec.Cmd) ([]byte, error) {
	var output bytes.Buffer
	cmd.Stdout = &output
	cmd.Stderr = &output

	hideWindow(cmd)
	if err := cmd.Start(); err != nil {
		return nil, err
	}

	registerExtractorCommand(cmd)
	defer unregisterExtractorCommand(cmd)

	err := cmd.Wait()
	return output.Bytes(), err
}

func getExecutableName() string {
	if runtime.GOOS == "windows" {
		return "xtractor.exe"
	}
	return "xtractor"
}

func getLegacyExecutableName() string {
	if runtime.GOOS == "windows" {
		return "extractor.exe"
	}
	return "extractor"
}

func KillAllExtractorProcesses() {
	activeExtractorCommands.Lock()
	commands := make([]*exec.Cmd, 0, len(activeExtractorCommands.byPID))
	for _, cmd := range activeExtractorCommands.byPID {
		commands = append(commands, cmd)
	}
	activeExtractorCommands.byPID = make(map[int]*exec.Cmd)
	activeExtractorCommands.Unlock()

	for _, cmd := range commands {
		if cmd != nil && cmd.Process != nil {
			_ = terminateCommandProcess(cmd)
		}
	}
}

func parseExtractorError(output string, username string) string {
	outputLower := strings.ToLower(output)

	lines := strings.Split(output, "\n")
	var errorLine string
	for _, line := range lines {
		lineLower := strings.ToLower(line)
		if strings.Contains(lineLower, "error:") || strings.Contains(lineLower, "exception") {
			errorLine = strings.TrimSpace(line)
			break
		}
	}
	if errorLine == "" {
		errorLine = strings.TrimSpace(output)
	}

	if len(errorLine) > 300 {
		errorLine = errorLine[:300] + "..."
	}

	var hint string
	if strings.Contains(outputLower, "unable to retrieve tweets from this timeline") {
		hint = " [Hint: End of timeline reached or rate limited - data already fetched has been saved]"
	} else if strings.Contains(outputLower, "rate limit") || strings.Contains(output, "429") {
		hint = " [Hint: Wait 5-15 minutes before retrying]"
	} else if strings.Contains(output, "401") || strings.Contains(outputLower, "unauthorized") {
		hint = " [Hint: Auth token may be invalid or expired]"
	} else if strings.Contains(output, "404") {
		hint = fmt.Sprintf(" [Hint: @%s may not exist or is suspended]", username)
	} else if strings.Contains(outputLower, "protected") || strings.Contains(output, "403") {
		hint = " [Hint: Protected account - need to follow and use auth token]"
	}

	return errorLine + hint
}

type TweetIDString int64

func (t TweetIDString) MarshalJSON() ([]byte, error) {
	return []byte(fmt.Sprintf(`"%d"`, t)), nil
}

func (t *TweetIDString) UnmarshalJSON(data []byte) error {

	var num int64
	if err := json.Unmarshal(data, &num); err == nil {
		*t = TweetIDString(num)
		return nil
	}

	var str string
	if err := json.Unmarshal(data, &str); err == nil {
		parsed, err := fmt.Sscanf(str, "%d", &num)
		if err != nil || parsed != 1 {
			return fmt.Errorf("invalid tweet_id string: %s", str)
		}
		*t = TweetIDString(num)
		return nil
	}
	return fmt.Errorf("tweet_id must be number or string")
}

type Author struct {
	ID   int64  `json:"id"`
	Name string `json:"name"`
	Nick string `json:"nick"`
}

type UserInfo struct {
	ID              int64  `json:"id"`
	Name            string `json:"name"`
	Nick            string `json:"nick"`
	Location        string `json:"location"`
	Date            string `json:"date"`
	Verified        bool   `json:"verified"`
	Protected       bool   `json:"protected"`
	ProfileBanner   string `json:"profile_banner"`
	ProfileImage    string `json:"profile_image"`
	FavouritesCount int    `json:"favourites_count"`
	FollowersCount  int    `json:"followers_count"`
	FriendsCount    int    `json:"friends_count"`
	ListedCount     int    `json:"listed_count"`
	MediaCount      int    `json:"media_count"`
	StatusesCount   int    `json:"statuses_count"`
	Description     string `json:"description"`
	URL             string `json:"url"`
}

type CLIMediaItem struct {
	URL            string        `json:"url"`
	TweetID        TweetIDString `json:"tweet_id"`
	RetweetID      TweetIDString `json:"retweet_id"`
	QuoteID        TweetIDString `json:"quote_id"`
	ReplyID        TweetIDString `json:"reply_id"`
	ConversationID TweetIDString `json:"conversation_id"`
	Date           string        `json:"date"`
	Extension      string        `json:"extension"`
	Width          int           `json:"width"`
	Height         int           `json:"height"`
	Type           string        `json:"type"`
	Bitrate        int           `json:"bitrate"`
	Duration       float64       `json:"duration"`
	Author         UserInfo      `json:"author"`
	User           UserInfo      `json:"user"`
	Content        string        `json:"content"`
	FavoriteCount  int           `json:"favorite_count"`
	RetweetCount   int           `json:"retweet_count"`
	ReplyCount     int           `json:"reply_count"`
	QuoteCount     int           `json:"quote_count"`
	BookmarkCount  int           `json:"bookmark_count"`
	ViewCount      int           `json:"view_count"`
	Source         string        `json:"source"`
	Sensitive      bool          `json:"sensitive"`
}

type TweetMetadata struct {
	TweetID        TweetIDString `json:"tweet_id"`
	RetweetID      TweetIDString `json:"retweet_id,omitempty"`
	QuoteID        TweetIDString `json:"quote_id,omitempty"`
	ReplyID        TweetIDString `json:"reply_id,omitempty"`
	ConversationID TweetIDString `json:"conversation_id,omitempty"`
	Date           string        `json:"date"`
	Author         Author        `json:"author"`
	Content        string        `json:"content"`
	Lang           string        `json:"lang,omitempty"`
	Hashtags       []string      `json:"hashtags,omitempty"`
	FavoriteCount  int           `json:"favorite_count"`
	RetweetCount   int           `json:"retweet_count"`
	QuoteCount     int           `json:"quote_count,omitempty"`
	ReplyCount     int           `json:"reply_count,omitempty"`
	BookmarkCount  int           `json:"bookmark_count,omitempty"`
	ViewCount      int           `json:"view_count,omitempty"`
	Sensitive      bool          `json:"sensitive,omitempty"`
}

type CLIResponse struct {
	Media     []CLIMediaItem  `json:"media"`
	Metadata  []TweetMetadata `json:"metadata"`
	Cursor    string          `json:"cursor,omitempty"`
	Total     int             `json:"total,omitempty"`
	Completed bool            `json:"completed,omitempty"`
}

type TimelineEntry struct {
	URL              string        `json:"url"`
	Date             string        `json:"date"`
	TweetID          TweetIDString `json:"tweet_id"`
	Type             string        `json:"type"`
	IsRetweet        bool          `json:"is_retweet"`
	Extension        string        `json:"extension"`
	Width            int           `json:"width"`
	Height           int           `json:"height"`
	Content          string        `json:"content,omitempty"`
	ViewCount        int           `json:"view_count,omitempty"`
	BookmarkCount    int           `json:"bookmark_count,omitempty"`
	FavoriteCount    int           `json:"favorite_count,omitempty"`
	RetweetCount     int           `json:"retweet_count,omitempty"`
	ReplyCount       int           `json:"reply_count,omitempty"`
	Source           string        `json:"source,omitempty"`
	Verified         bool          `json:"verified,omitempty"`
	OriginalFilename string        `json:"original_filename,omitempty"`
	AuthorUsername   string        `json:"author_username,omitempty"`
}

type AccountInfo struct {
	Name           string `json:"name"`
	Nick           string `json:"nick"`
	Date           string `json:"date"`
	FollowersCount int    `json:"followers_count"`
	FriendsCount   int    `json:"friends_count"`
	ProfileImage   string `json:"profile_image"`
	StatusesCount  int    `json:"statuses_count"`
}

type ExtractMetadata struct {
	NewEntries int    `json:"new_entries"`
	Page       int    `json:"page"`
	BatchSize  int    `json:"batch_size"`
	HasMore    bool   `json:"has_more"`
	Cursor     string `json:"cursor,omitempty"`
	Completed  bool   `json:"completed,omitempty"`
}

type TwitterResponse struct {
	AccountInfo AccountInfo     `json:"account_info"`
	TotalURLs   int             `json:"total_urls"`
	Timeline    []TimelineEntry `json:"timeline"`
	Metadata    ExtractMetadata `json:"metadata"`
	Cursor      string          `json:"cursor,omitempty"`
	Completed   bool            `json:"completed,omitempty"`
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

func buildTwitterURL(username, timelineType string) string {

	if timelineType == "bookmarks" {
		return "https://x.com/i/bookmarks"
	}

	username = cleanUsername(username)

	baseURL := "https://x.com/" + username
	switch timelineType {
	case "media":
		return baseURL + "/media"
	case "timeline":
		return baseURL + "/timeline"
	case "tweets":
		return baseURL + "/tweets"
	case "with_replies":
		return baseURL + "/with_replies"
	case "likes":
		return baseURL + "/likes"
	default:
		return baseURL + "/timeline"
	}
}

func cleanUsername(username string) string {
	username = strings.TrimSpace(username)
	username = strings.TrimPrefix(username, "@")

	if strings.Contains(username, "x.com/") || strings.Contains(username, "twitter.com/") {
		parsed := username
		if !strings.HasPrefix(parsed, "http://") && !strings.HasPrefix(parsed, "https://") {
			parsed = "https://" + strings.TrimPrefix(parsed, "//")
		}
		if u, err := url.Parse(parsed); err == nil {
			segments := strings.Split(strings.Trim(u.Path, "/"), "/")

			if len(segments) > 0 && segments[0] != "" {
				firstSegment := strings.ToLower(segments[0])

				if firstSegment == "i" || firstSegment == "search" || firstSegment == "home" || firstSegment == "explore" || firstSegment == "settings" || firstSegment == "messages" || firstSegment == "notifications" {
					return username
				}
				return segments[0]
			}
		}
	}

	return username
}

func ensureURLScheme(raw string) string {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return raw
	}
	if strings.HasPrefix(raw, "http://") || strings.HasPrefix(raw, "https://") {
		return raw
	}
	if strings.HasPrefix(raw, "//") {
		return "https:" + raw
	}
	return "https://" + strings.TrimPrefix(raw, "//")
}

func buildSearchURL(username, startDate, endDate, mediaFilter string, includeRetweets bool) string {
	trimmed := strings.TrimSpace(username)
	lower := strings.ToLower(trimmed)
	if strings.Contains(lower, "search?q=") {
		return ensureURLScheme(trimmed)
	}

	handle := cleanUsername(trimmed)
	var parts []string
	if handle != "" {
		parts = append(parts, fmt.Sprintf("from:%s", handle))
	}
	if startDate != "" {
		parts = append(parts, fmt.Sprintf("since:%s", startDate))
	}
	if endDate != "" {
		parts = append(parts, fmt.Sprintf("until:%s", endDate))
	}

	switch strings.ToLower(strings.TrimSpace(mediaFilter)) {
	case "image", "images", "photo", "photos":
		parts = append(parts, "filter:images")
	case "video", "videos", "gif", "gifs":
		parts = append(parts, "filter:videos")
	case "text":
		parts = append(parts, "-filter:media")
	default:
		parts = append(parts, "filter:media")
	}

	if !includeRetweets {
		parts = append(parts, "-filter:retweets")
	}

	query := url.QueryEscape(strings.Join(parts, " "))
	return fmt.Sprintf("https://x.com/search?q=%s&src=typed_query&f=live", query)
}

func convertMetadataToTimelineEntry(meta TweetMetadata) TimelineEntry {
	return TimelineEntry{
		URL:            "",
		Date:           meta.Date,
		TweetID:        meta.TweetID,
		Type:           "text",
		IsRetweet:      meta.RetweetID != 0,
		Extension:      "txt",
		Width:          0,
		Height:         0,
		Content:        meta.Content,
		ViewCount:      meta.ViewCount,
		BookmarkCount:  meta.BookmarkCount,
		FavoriteCount:  meta.FavoriteCount,
		RetweetCount:   meta.RetweetCount,
		ReplyCount:     meta.ReplyCount,
		AuthorUsername: meta.Author.Name,
	}
}

func convertToTimelineEntry(media CLIMediaItem) TimelineEntry {

	authorUsername := ""
	if media.Author.Name != "" {
		authorUsername = media.Author.Name
	} else if media.User.Name != "" {
		authorUsername = media.User.Name
	}

	entry := TimelineEntry{
		URL:            media.URL,
		TweetID:        media.TweetID,
		Date:           media.Date,
		Extension:      media.Extension,
		Width:          media.Width,
		Height:         media.Height,
		IsRetweet:      media.RetweetID != 0,
		Content:        media.Content,
		ViewCount:      media.ViewCount,
		BookmarkCount:  media.BookmarkCount,
		FavoriteCount:  media.FavoriteCount,
		RetweetCount:   media.RetweetCount,
		ReplyCount:     media.ReplyCount,
		Source:         media.Source,
		Verified:       media.Author.Verified,
		AuthorUsername: authorUsername,
	}

	if media.Type != "" {
		entry.Type = media.Type
	} else {
		switch strings.ToLower(media.Extension) {
		case "mp4", "webm":
			entry.Type = "video"
		case "gif":
			entry.Type = "gif"
		default:
			entry.Type = "photo"
		}
	}

	return entry
}

func getExtractorPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")
	return filepath.Join(baseDir, getExecutableName())
}

func ExtractTimeline(req TimelineRequest) (*TwitterResponse, error) {

	exePath, err := requireExtractorPath()
	if err != nil {
		return nil, err
	}

	isTextOnly := req.MediaType == "text"
	wantsRetweets := req.Retweets

	timelineType := req.TimelineType
	if timelineType == "" {
		if isTextOnly {

			timelineType = "tweets"
		} else if wantsRetweets {

			timelineType = "tweets"
		} else {

			timelineType = "media"
		}
	}

	url := buildTwitterURL(req.Username, timelineType)

	args := []string{url}

	if req.AuthToken != "" {
		args = append(args, "--auth-token", req.AuthToken)
	} else {
		args = append(args, "--guest")
	}

	args = append(args, "--json", "--metadata")

	if req.BatchSize > 0 {
		args = append(args, "--limit", fmt.Sprintf("%d", req.BatchSize))
	}

	if timelineType == "tweets" || timelineType == "timeline" {
		if req.Retweets {
			args = append(args, "--retweets", "include")
		} else {
			args = append(args, "--retweets", "skip")
		}
	}

	if isTextOnly {
		args = append(args, "--text-tweets")
	}

	if req.MediaType != "" && req.MediaType != "all" && !isTextOnly {
		switch req.MediaType {
		case "image":
			args = append(args, "--type", "photo")
		case "video":
			args = append(args, "--type", "video")
		case "gif":
			args = append(args, "--type", "animated_gif")
		}
	}

	if req.Cursor != "" {
		args = append(args, "--cursor", req.Cursor)
	}

	cmd := exec.Command(exePath, args...)
	cmd.Env = append(os.Environ(),
		"PYTHONIOENCODING=utf-8",
		"PYTHONUTF8=1",
	)
	output, err := runTrackedExtractorCommand(cmd)

	if err != nil {
		outputStr := string(output)
		if strings.TrimSpace(outputStr) == "" {
			return nil, fmt.Errorf("xtractor process terminated before returning data")
		}
		errorMsg := parseExtractorError(outputStr, req.Username)
		return nil, fmt.Errorf("%s", errorMsg)
	}

	jsonStr := extractJSON(string(output))
	if jsonStr == "" {
		outputStr := string(output)
		if strings.TrimSpace(outputStr) == "" {
			return nil, fmt.Errorf("empty_response: Xtractor returned no data. The timeline may be empty or inaccessible")
		}
		return nil, fmt.Errorf("parse_error: Could not parse xtractor output. Raw output: %s", outputStr)
	}

	var cliResponse CLIResponse
	if err := json.Unmarshal([]byte(jsonStr), &cliResponse); err != nil {
		return nil, fmt.Errorf("json_error: Failed to parse JSON response: %v", err)
	}

	timeline := make([]TimelineEntry, 0)
	accountInfo := AccountInfo{
		Name: req.Username,
		Nick: req.Username,
	}

	mediaTweetIDs := make(map[int64]bool)
	for _, media := range cliResponse.Media {
		mediaTweetIDs[int64(media.TweetID)] = true
	}

	isBookmarks := req.TimelineType == "bookmarks"
	isLikes := req.TimelineType == "likes"
	if isBookmarks {
		accountInfo.Name = "bookmarks"
		accountInfo.Nick = "My Bookmarks"
	} else if isLikes {
		accountInfo.Name = "likes"
		accountInfo.Nick = "My Likes"
	}

	if isTextOnly {

		for _, meta := range cliResponse.Metadata {
			if !mediaTweetIDs[int64(meta.TweetID)] {
				timeline = append(timeline, convertMetadataToTimelineEntry(meta))
			}
		}

		if !isBookmarks && !isLikes {
			if len(cliResponse.Media) > 0 {
				user := cliResponse.Media[0].User
				accountInfo.Name = user.Name
				accountInfo.Nick = user.Nick
				accountInfo.Date = user.Date
				accountInfo.FollowersCount = user.FollowersCount
				accountInfo.FriendsCount = user.FriendsCount
				accountInfo.ProfileImage = user.ProfileImage
				accountInfo.StatusesCount = user.StatusesCount
			} else if len(cliResponse.Metadata) > 0 {
				firstMeta := cliResponse.Metadata[0]
				accountInfo.Name = firstMeta.Author.Name
				accountInfo.Nick = firstMeta.Author.Nick
			}
		} else {

			if len(cliResponse.Media) > 0 {
				user := cliResponse.Media[0].User
				accountInfo.Date = user.Date
				accountInfo.FollowersCount = user.FollowersCount
				accountInfo.FriendsCount = user.FriendsCount
				accountInfo.ProfileImage = user.ProfileImage
				accountInfo.StatusesCount = user.StatusesCount
			}
		}
	} else if len(cliResponse.Media) > 0 {

		timeline = make([]TimelineEntry, 0, len(cliResponse.Media))

		for _, media := range cliResponse.Media {
			timeline = append(timeline, convertToTimelineEntry(media))
		}

		user := cliResponse.Media[0].User
		if !isBookmarks && !isLikes {
			accountInfo.Name = user.Name
			accountInfo.Nick = user.Nick
		}
		accountInfo.Date = user.Date
		accountInfo.FollowersCount = user.FollowersCount
		accountInfo.FriendsCount = user.FriendsCount
		accountInfo.ProfileImage = user.ProfileImage
		accountInfo.StatusesCount = user.StatusesCount
	} else if len(cliResponse.Metadata) > 0 {

		timeline = make([]TimelineEntry, 0, len(cliResponse.Metadata))
		for _, meta := range cliResponse.Metadata {
			entry := TimelineEntry{
				URL:            "",
				TweetID:        meta.TweetID,
				Date:           meta.Date,
				Type:           "text",
				IsRetweet:      meta.RetweetID != 0,
				Extension:      "txt",
				Width:          0,
				Height:         0,
				Content:        meta.Content,
				ViewCount:      meta.ViewCount,
				BookmarkCount:  meta.BookmarkCount,
				FavoriteCount:  meta.FavoriteCount,
				RetweetCount:   meta.RetweetCount,
				ReplyCount:     meta.ReplyCount,
				AuthorUsername: meta.Author.Name,
			}
			timeline = append(timeline, entry)
		}

		if !isBookmarks && !isLikes {
			firstMeta := cliResponse.Metadata[0]
			accountInfo.Name = firstMeta.Author.Name
			accountInfo.Nick = firstMeta.Author.Nick
		}
	}

	hasMore := cliResponse.Cursor != "" && !cliResponse.Completed

	response := &TwitterResponse{
		AccountInfo: accountInfo,
		TotalURLs:   len(timeline),
		Timeline:    timeline,
		Metadata: ExtractMetadata{
			NewEntries: len(timeline),
			Page:       req.Page,
			BatchSize:  req.BatchSize,
			HasMore:    hasMore,
			Cursor:     cliResponse.Cursor,
			Completed:  cliResponse.Completed,
		},
		Cursor:    cliResponse.Cursor,
		Completed: cliResponse.Completed,
	}

	return response, nil
}

func ExtractDateRange(req DateRangeRequest) (*TwitterResponse, error) {

	exePath, err := requireExtractorPath()
	if err != nil {
		return nil, err
	}

	mediaFilter := strings.ToLower(strings.TrimSpace(req.MediaFilter))
	url := buildSearchURL(req.Username, req.StartDate, req.EndDate, mediaFilter, req.Retweets)

	args := []string{url}

	if req.AuthToken != "" {
		args = append(args, "--auth-token", req.AuthToken)
	} else {
		args = append(args, "--guest")
	}

	args = append(args, "--json", "--metadata")

	if req.Retweets {
		args = append(args, "--retweets", "include")
	} else {
		args = append(args, "--retweets", "skip")
	}

	isTextOnly := mediaFilter == "text"
	if isTextOnly {
		args = append(args, "--text-tweets")
	}

	cmd := exec.Command(exePath, args...)
	cmd.Env = append(os.Environ(),
		"PYTHONIOENCODING=utf-8",
		"PYTHONUTF8=1",
	)
	output, err := runTrackedExtractorCommand(cmd)

	if err != nil {
		outputStr := string(output)
		if strings.TrimSpace(outputStr) == "" {
			return nil, fmt.Errorf("xtractor process terminated before returning data")
		}
		errorMsg := parseExtractorError(outputStr, req.Username)
		return nil, fmt.Errorf("%s", errorMsg)
	}

	jsonStr := extractJSON(string(output))
	if jsonStr == "" {
		outputStr := string(output)
		if strings.TrimSpace(outputStr) == "" {
			return nil, fmt.Errorf("empty_response: Xtractor returned no data. The timeline may be empty or inaccessible")
		}
		return nil, fmt.Errorf("parse_error: Could not parse xtractor output. Raw output: %s", outputStr)
	}

	var cliResponse CLIResponse
	if err := json.Unmarshal([]byte(jsonStr), &cliResponse); err != nil {
		return nil, fmt.Errorf("json_error: Failed to parse JSON response: %v", err)
	}

	mediaTweetIDs := make(map[int64]bool)
	for _, media := range cliResponse.Media {
		mediaTweetIDs[int64(media.TweetID)] = true
	}

	timeline := make([]TimelineEntry, 0, len(cliResponse.Media)+len(cliResponse.Metadata))
	for _, media := range cliResponse.Media {
		timeline = append(timeline, convertToTimelineEntry(media))
	}

	if isTextOnly {
		for _, meta := range cliResponse.Metadata {
			if !mediaTweetIDs[int64(meta.TweetID)] {
				timeline = append(timeline, convertMetadataToTimelineEntry(meta))
			}
		}
	}

	accountInfo := AccountInfo{
		Name: req.Username,
		Nick: req.Username,
	}
	if len(cliResponse.Media) > 0 {
		user := cliResponse.Media[0].User
		accountInfo.Name = user.Name
		accountInfo.Nick = user.Nick
		accountInfo.Date = user.Date
		accountInfo.FollowersCount = user.FollowersCount
		accountInfo.FriendsCount = user.FriendsCount
		accountInfo.ProfileImage = user.ProfileImage
		accountInfo.StatusesCount = user.StatusesCount
	} else if len(cliResponse.Metadata) > 0 {
		firstMeta := cliResponse.Metadata[0]
		accountInfo.Name = firstMeta.Author.Name
		accountInfo.Nick = firstMeta.Author.Nick
	}

	hasMore := cliResponse.Cursor != "" && !cliResponse.Completed

	response := &TwitterResponse{
		AccountInfo: accountInfo,
		TotalURLs:   len(timeline),
		Timeline:    timeline,
		Metadata: ExtractMetadata{
			NewEntries: len(timeline),
			Page:       0,
			BatchSize:  0,
			HasMore:    hasMore,
			Cursor:     cliResponse.Cursor,
			Completed:  cliResponse.Completed,
		},
		Cursor:    cliResponse.Cursor,
		Completed: cliResponse.Completed,
	}

	return response, nil
}

func extractJSON(output string) string {

	start := strings.Index(output, "{")
	if start == -1 {
		return ""
	}

	depth := 0
	for i := start; i < len(output); i++ {
		if output[i] == '{' {
			depth++
		} else if output[i] == '}' {
			depth--
			if depth == 0 {
				return output[start : i+1]
			}
		}
	}

	return ""
}
