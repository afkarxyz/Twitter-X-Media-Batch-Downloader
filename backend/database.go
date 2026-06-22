package backend

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	_ "modernc.org/sqlite"
)

type AccountDB struct {
	ID           int64     `json:"id"`
	Username     string    `json:"username"`
	Name         string    `json:"name"`
	ProfileImage string    `json:"profile_image"`
	TotalMedia   int       `json:"total_media"`
	LastFetched  time.Time `json:"last_fetched"`
	ResponseJSON string    `json:"response_json"`
	MediaType    string    `json:"media_type"`
	Cursor       string    `json:"cursor"`
	Completed    bool      `json:"completed"`
}

type AccountListItem struct {
	ID             int64  `json:"id"`
	Username       string `json:"username"`
	Name           string `json:"name"`
	ProfileImage   string `json:"profile_image"`
	TotalMedia     int    `json:"total_media"`
	LastFetched    string `json:"last_fetched"`
	GroupName      string `json:"group_name"`
	GroupColor     string `json:"group_color"`
	MediaType      string `json:"media_type"`
	Cursor         string `json:"cursor"`
	Completed      bool   `json:"completed"`
	FollowersCount int    `json:"followers_count"`
	StatusesCount  int    `json:"statuses_count"`
}

var db *sql.DB
var dbInitMu sync.Mutex
var dbInitialized bool

func ensureDB() error {
	dbInitMu.Lock()
	defer dbInitMu.Unlock()

	if dbInitialized && db != nil {
		return nil
	}

	if err := initDBInternal(); err != nil {
		return err
	}

	dbInitialized = true
	return nil
}

type accountMetricsPayload struct {
	AccountInfo struct {
		FollowersCount int `json:"followers_count"`
		StatusesCount  int `json:"statuses_count"`
	} `json:"account_info"`
	Followers int `json:"followers"`
	Posts     int `json:"posts"`
}

func GetDBPath() string {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		homeDir = "."
	}
	return filepath.Join(homeDir, ".twitterxmediabatchdownloader", "accounts.db")
}

func InitDB() error {
	return ensureDB()
}

func initDBInternal() error {
	dbPath := GetDBPath()

	dir := filepath.Dir(dbPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	var err error
	db, err = sql.Open("sqlite", dbPath)
	if err != nil {
		return err
	}

	if err := db.Ping(); err != nil {
		return err
	}

	_, err = db.Exec(`
		CREATE TABLE IF NOT EXISTS accounts (
			id INTEGER PRIMARY KEY AUTOINCREMENT,
			username TEXT NOT NULL,
			name TEXT,
			profile_image TEXT,
			total_media INTEGER DEFAULT 0,
			last_fetched DATETIME,
			response_json TEXT,
			group_name TEXT DEFAULT '',
			group_color TEXT DEFAULT '',
			media_type TEXT DEFAULT 'all',
			cursor TEXT DEFAULT '',
			completed INTEGER DEFAULT 1,
			followers_count INTEGER DEFAULT 0,
			statuses_count INTEGER DEFAULT 0,
			UNIQUE(username, media_type)
		)
	`)
	if err != nil {
		return err
	}

	db.Exec("CREATE UNIQUE INDEX IF NOT EXISTS idx_username_media_type ON accounts(username, media_type)")

	if err := runMigrations(); err != nil {
		return err
	}

	if err := backfillAccountMetrics(); err != nil {
		return err
	}

	return nil
}

func runMigrations() error {
	var version int
	if err := db.QueryRow("PRAGMA user_version").Scan(&version); err != nil {
		return err
	}

	migrations := []string{
		"ALTER TABLE accounts ADD COLUMN group_name TEXT DEFAULT ''",
		"ALTER TABLE accounts ADD COLUMN group_color TEXT DEFAULT ''",
		"ALTER TABLE accounts ADD COLUMN media_type TEXT DEFAULT 'all'",
		"ALTER TABLE accounts ADD COLUMN cursor TEXT DEFAULT ''",
		"ALTER TABLE accounts ADD COLUMN completed INTEGER DEFAULT 1",
		"ALTER TABLE accounts ADD COLUMN followers_count INTEGER DEFAULT 0",
		"ALTER TABLE accounts ADD COLUMN statuses_count INTEGER DEFAULT 0",
	}

	for i := version; i < len(migrations); i++ {
		if _, err := db.Exec(migrations[i]); err != nil {
			if !strings.Contains(err.Error(), "duplicate column name") {
				return fmt.Errorf("migration %d failed: %w", i, err)
			}
		}
	}

	if _, err := db.Exec(fmt.Sprintf("PRAGMA user_version = %d", len(migrations))); err != nil {
		return err
	}

	return nil
}

func CloseDB() {
	if db != nil {
		db.Close()
	}
}

func SaveAccount(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string) error {
	return SaveAccountWithStatus(username, name, profileImage, totalMedia, responseJSON, mediaType, "", true)
}

func SaveAccountWithStatus(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string, cursor string, completed bool) error {
	if err := ensureDB(); err != nil {
		return err
	}

	if mediaType == "" {
		mediaType = "all"
	}

	completedInt := 0
	if completed {
		completedInt = 1
	}

	followersCount, statusesCount := extractAccountMetrics(responseJSON)

	_, err := db.Exec(`
		INSERT INTO accounts (username, name, profile_image, total_media, last_fetched, response_json, media_type, cursor, completed, followers_count, statuses_count)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(username, media_type) DO UPDATE SET
			name = excluded.name,
			profile_image = excluded.profile_image,
			total_media = excluded.total_media,
			last_fetched = excluded.last_fetched,
			response_json = excluded.response_json,
			cursor = excluded.cursor,
			completed = excluded.completed,
			followers_count = excluded.followers_count,
			statuses_count = excluded.statuses_count
	`, username, name, profileImage, totalMedia, time.Now(), responseJSON, mediaType, cursor, completedInt, followersCount, statusesCount)

	return err
}

func GetAllAccounts() ([]AccountListItem, error) {
	if err := ensureDB(); err != nil {
		return nil, err
	}

	rows, err := db.Query(`
		SELECT id, username, name, profile_image, total_media, last_fetched, 
		       COALESCE(group_name, '') as group_name, COALESCE(group_color, '') as group_color,
		       COALESCE(media_type, 'all') as media_type,
		       COALESCE(cursor, '') as cursor, COALESCE(completed, 1) as completed,
		       COALESCE(followers_count, 0) as followers_count,
		       COALESCE(statuses_count, 0) as statuses_count
		FROM accounts
		ORDER BY group_name ASC, last_fetched DESC
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var accounts []AccountListItem
	for rows.Next() {
		var acc AccountListItem
		var lastFetched time.Time
		var completedInt int
		if err := rows.Scan(&acc.ID, &acc.Username, &acc.Name, &acc.ProfileImage, &acc.TotalMedia, &lastFetched, &acc.GroupName, &acc.GroupColor, &acc.MediaType, &acc.Cursor, &completedInt, &acc.FollowersCount, &acc.StatusesCount); err != nil {
			continue
		}
		acc.LastFetched = lastFetched.Format("2006-01-02 15:04")
		acc.Completed = completedInt == 1

		accounts = append(accounts, acc)
	}

	return accounts, nil
}

func extractAccountMetrics(responseJSON string) (followersCount int, statusesCount int) {
	if responseJSON == "" {
		return 0, 0
	}

	var payload accountMetricsPayload
	if err := json.Unmarshal([]byte(responseJSON), &payload); err != nil {
		return 0, 0
	}

	followersCount = payload.AccountInfo.FollowersCount
	statusesCount = payload.AccountInfo.StatusesCount

	if followersCount == 0 {
		followersCount = payload.Followers
	}
	if statusesCount == 0 {
		statusesCount = payload.Posts
	}

	return followersCount, statusesCount
}

func backfillAccountMetrics() error {
	rows, err := db.Query(`
		SELECT id, COALESCE(response_json, '') as response_json
		FROM accounts
		WHERE COALESCE(response_json, '') != ''
		  AND (COALESCE(followers_count, 0) = 0 OR COALESCE(statuses_count, 0) = 0)
	`)
	if err != nil {
		return err
	}

	type metricUpdate struct {
		id             int64
		followersCount int
		statusesCount  int
	}
	var updates []metricUpdate
	for rows.Next() {
		var id int64
		var responseJSON string
		if err := rows.Scan(&id, &responseJSON); err != nil {
			continue
		}
		followersCount, statusesCount := extractAccountMetrics(responseJSON)
		updates = append(updates, metricUpdate{id, followersCount, statusesCount})
	}
	if err := rows.Err(); err != nil {
		rows.Close()
		return err
	}
	rows.Close()

	if len(updates) == 0 {
		return nil
	}

	stmt, err := db.Prepare(`
		UPDATE accounts
		SET followers_count = ?, statuses_count = ?
		WHERE id = ?
	`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, u := range updates {
		if _, err := stmt.Exec(u.followersCount, u.statusesCount, u.id); err != nil {
			return err
		}
	}

	return nil
}

func UpdateAccountGroup(id int64, groupName, groupColor string) error {
	if err := ensureDB(); err != nil {
		return err
	}

	_, err := db.Exec("UPDATE accounts SET group_name = ?, group_color = ? WHERE id = ?", groupName, groupColor, id)
	return err
}

func GetAllGroups() ([]map[string]string, error) {
	if err := ensureDB(); err != nil {
		return nil, err
	}

	rows, err := db.Query(`
		SELECT DISTINCT group_name, group_color 
		FROM accounts 
		WHERE group_name != '' 
		ORDER BY group_name
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var groups []map[string]string
	for rows.Next() {
		var name, color string
		if err := rows.Scan(&name, &color); err != nil {
			continue
		}
		groups = append(groups, map[string]string{"name": name, "color": color})
	}

	return groups, nil
}

func ClearAllAccounts() error {
	if err := ensureDB(); err != nil {
		return err
	}

	_, err := db.Exec("DELETE FROM accounts")
	return err
}

func GetAccountByID(id int64) (*AccountDB, error) {
	if err := ensureDB(); err != nil {
		return nil, err
	}

	var acc AccountDB
	var lastFetched time.Time
	var completedInt int
	err := db.QueryRow(`
		SELECT id, username, name, profile_image, total_media, last_fetched, response_json,
		       COALESCE(cursor, '') as cursor, COALESCE(completed, 1) as completed
		FROM accounts WHERE id = ?
	`, id).Scan(&acc.ID, &acc.Username, &acc.Name, &acc.ProfileImage, &acc.TotalMedia, &lastFetched, &acc.ResponseJSON, &acc.Cursor, &completedInt)

	if err != nil {
		return nil, err
	}
	acc.LastFetched = lastFetched
	acc.Completed = completedInt == 1

	if converted, err := ConvertLegacyToNewFormat(acc.ResponseJSON); err == nil {
		acc.ResponseJSON = converted
	}

	return &acc, nil
}

func GetAccountByUsernameAndMediaType(username, mediaType string) (*AccountDB, error) {
	if err := ensureDB(); err != nil {
		return nil, err
	}

	if mediaType == "" {
		mediaType = "all"
	}

	var acc AccountDB
	var lastFetched time.Time
	var completedInt int
	err := db.QueryRow(`
		SELECT id, username, name, profile_image, total_media, last_fetched, response_json,
		       COALESCE(media_type, 'all') as media_type,
		       COALESCE(cursor, '') as cursor, COALESCE(completed, 1) as completed
		FROM accounts
		WHERE LOWER(username) = LOWER(?) AND COALESCE(media_type, 'all') = ?
	`, username, mediaType).Scan(&acc.ID, &acc.Username, &acc.Name, &acc.ProfileImage, &acc.TotalMedia, &lastFetched, &acc.ResponseJSON, &acc.MediaType, &acc.Cursor, &completedInt)
	if err != nil {
		return nil, err
	}

	acc.LastFetched = lastFetched
	acc.Completed = completedInt == 1

	if converted, err := ConvertLegacyToNewFormat(acc.ResponseJSON); err == nil {
		acc.ResponseJSON = converted
	}

	return &acc, nil
}

func DeleteAccount(id int64) error {
	if err := ensureDB(); err != nil {
		return err
	}

	_, err := db.Exec("DELETE FROM accounts WHERE id = ?", id)
	return err
}

type LegacyMediaEntry struct {
	TweetID string `json:"tweet_id"`
	URL     string `json:"url"`
	Date    string `json:"date"`
	Type    string `json:"type"`
}

type LegacyAccountFormat struct {
	Username       string             `json:"username"`
	Nick           string             `json:"nick"`
	Followers      int                `json:"followers"`
	Following      int                `json:"following"`
	Posts          int                `json:"posts"`
	MediaType      string             `json:"media_type"`
	ProfileImage   string             `json:"profile_image"`
	FetchMode      string             `json:"fetch_mode"`
	FetchTimestamp string             `json:"fetch_timestamp"`
	GroupID        interface{}        `json:"group_id"`
	MediaList      []LegacyMediaEntry `json:"media_list"`
}

func ConvertLegacyToNewFormat(jsonStr string) (string, error) {

	var check map[string]interface{}
	if err := json.Unmarshal([]byte(jsonStr), &check); err != nil {
		return jsonStr, err
	}

	if _, hasAccountInfo := check["account_info"]; hasAccountInfo {
		return jsonStr, nil
	}

	if _, hasUsername := check["username"]; !hasUsername {
		return jsonStr, nil
	}
	if _, hasMediaList := check["media_list"]; !hasMediaList {
		return jsonStr, nil
	}

	var legacy LegacyAccountFormat
	if err := json.Unmarshal([]byte(jsonStr), &legacy); err != nil {
		return jsonStr, err
	}

	timeline := make([]map[string]interface{}, len(legacy.MediaList))
	for i, media := range legacy.MediaList {
		timeline[i] = map[string]interface{}{
			"url":        media.URL,
			"date":       media.Date,
			"tweet_id":   media.TweetID,
			"type":       media.Type,
			"is_retweet": false,
		}
	}

	newFormat := map[string]interface{}{
		"account_info": map[string]interface{}{
			"name":            legacy.Username,
			"nick":            legacy.Nick,
			"date":            "",
			"followers_count": legacy.Followers,
			"friends_count":   legacy.Following,
			"profile_image":   legacy.ProfileImage,
			"statuses_count":  legacy.Posts,
		},
		"total_urls": len(legacy.MediaList),
		"timeline":   timeline,
		"metadata": map[string]interface{}{
			"new_entries": len(legacy.MediaList),
			"page":        0,
			"batch_size":  0,
			"has_more":    false,
		},
	}

	newJSON, err := json.Marshal(newFormat)
	if err != nil {
		return jsonStr, err
	}

	return string(newJSON), nil
}

func ExportAccountToFile(id int64, outputDir string) (string, error) {
	acc, err := GetAccountByID(id)
	if err != nil {
		return "", err
	}

	exportDir := filepath.Join(outputDir, "twitterxmediabatchdownloader_backups")
	if err := os.MkdirAll(exportDir, 0755); err != nil {
		return "", err
	}

	filename := acc.Username
	if filename == "" {
		filename = acc.Name
	}

	filePath := filepath.Join(exportDir, filename+".json")

	if err := os.WriteFile(filePath, []byte(acc.ResponseJSON), 0644); err != nil {
		return "", err
	}

	return filePath, nil
}

func ExportAccountsToTXT(ids []int64, outputDir string) (string, error) {
	if len(ids) == 0 {
		return "", fmt.Errorf("no accounts to export")
	}

	exportDir := filepath.Join(outputDir, "twitterxmediabatchdownloader_backups")
	if err := os.MkdirAll(exportDir, 0755); err != nil {
		return "", err
	}

	var usernames []string
	for _, id := range ids {
		acc, err := GetAccountByID(id)
		if err != nil {
			continue
		}
		if acc.Username != "" {
			usernames = append(usernames, acc.Username)
		}
	}

	if len(usernames) == 0 {
		return "", fmt.Errorf("no valid usernames found")
	}

	txtContent := strings.Join(usernames, "\n")

	filePath := filepath.Join(exportDir, "twitterxmediabatchdownloader_multiple.txt")

	if err := os.WriteFile(filePath, []byte(txtContent), 0644); err != nil {
		return "", err
	}

	return filePath, nil
}

func ImportAccountFromFile(filePath string) (string, error) {
	if err := ensureDB(); err != nil {
		return "", err
	}

	data, err := os.ReadFile(filePath)
	if err != nil {
		return "", err
	}

	jsonStr := string(data)

	convertedJSON, err := ConvertLegacyToNewFormat(jsonStr)
	if err != nil {
		return "", err
	}

	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(convertedJSON), &parsed); err != nil {
		return "", err
	}

	accountInfo, ok := parsed["account_info"].(map[string]interface{})
	if !ok {
		return "", fmt.Errorf("invalid JSON format: missing account_info")
	}

	username, _ := accountInfo["name"].(string)
	name, _ := accountInfo["nick"].(string)
	profileImage, _ := accountInfo["profile_image"].(string)

	totalURLs := 0
	if total, ok := parsed["total_urls"].(float64); ok {
		totalURLs = int(total)
	}

	if username == "" {
		return "", fmt.Errorf("invalid JSON format: missing username")
	}

	err = SaveAccount(username, name, profileImage, totalURLs, convertedJSON, "all")
	if err != nil {
		return "", err
	}

	return username, nil
}
