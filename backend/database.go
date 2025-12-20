package backend

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	_ "github.com/mattn/go-sqlite3"
)

// AccountDB represents a saved account in the database
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

// AccountListItem represents a simplified account for listing
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

// GetDBPath returns the database file path
func GetDBPath() string {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		homeDir = "."
	}
	return filepath.Join(homeDir, ".twitterxmediabatchdownloader", "accounts.db")
}

// InitDB initializes the database connection
func InitDB() error {
	dbPath := GetDBPath()

	// Create directory if not exists
	dir := filepath.Dir(dbPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	var err error
	db, err = sql.Open("sqlite3", dbPath)
	if err != nil {
		return err
	}

	// Create tables
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
			UNIQUE(username, media_type)
		)
	`)
	if err != nil {
		return err
	}

	// Add group columns if they don't exist (migration for existing databases)
	db.Exec("ALTER TABLE accounts ADD COLUMN group_name TEXT DEFAULT ''")
	db.Exec("ALTER TABLE accounts ADD COLUMN group_color TEXT DEFAULT ''")
	db.Exec("ALTER TABLE accounts ADD COLUMN media_type TEXT DEFAULT 'all'")
	db.Exec("ALTER TABLE accounts ADD COLUMN cursor TEXT DEFAULT ''")
	db.Exec("ALTER TABLE accounts ADD COLUMN completed INTEGER DEFAULT 1")

	// Migration: Update unique constraint for existing databases
	// This allows same username with different media types
	db.Exec("CREATE UNIQUE INDEX IF NOT EXISTS idx_username_media_type ON accounts(username, media_type)")

	return nil
}

// CloseDB closes the database connection
func CloseDB() {
	if db != nil {
		db.Close()
	}
}

// SaveAccount saves or updates an account in the database
func SaveAccount(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string) error {
	return SaveAccountWithStatus(username, name, profileImage, totalMedia, responseJSON, mediaType, "", true)
}

// SaveAccountWithStatus saves or updates an account with cursor and completion status
func SaveAccountWithStatus(username, name, profileImage string, totalMedia int, responseJSON string, mediaType string, cursor string, completed bool) error {
	if db == nil {
		if err := InitDB(); err != nil {
			return err
		}
	}

	// Default to "all" if not specified
	if mediaType == "" {
		mediaType = "all"
	}

	completedInt := 0
	if completed {
		completedInt = 1
	}

	_, err := db.Exec(`
		INSERT INTO accounts (username, name, profile_image, total_media, last_fetched, response_json, media_type, cursor, completed)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(username, media_type) DO UPDATE SET
			name = excluded.name,
			profile_image = excluded.profile_image,
			total_media = excluded.total_media,
			last_fetched = excluded.last_fetched,
			response_json = excluded.response_json,
			cursor = excluded.cursor,
			completed = excluded.completed
	`, username, name, profileImage, totalMedia, time.Now(), responseJSON, mediaType, cursor, completedInt)

	return err
}

// GetAllAccounts returns all saved accounts
func GetAllAccounts() ([]AccountListItem, error) {
	if db == nil {
		if err := InitDB(); err != nil {
			return nil, err
		}
	}

	rows, err := db.Query(`
		SELECT id, username, name, profile_image, total_media, last_fetched, 
		       COALESCE(group_name, '') as group_name, COALESCE(group_color, '') as group_color,
		       COALESCE(media_type, 'all') as media_type,
		       COALESCE(cursor, '') as cursor, COALESCE(completed, 1) as completed,
		       COALESCE(response_json, '') as response_json
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
		var responseJSON string
		if err := rows.Scan(&acc.ID, &acc.Username, &acc.Name, &acc.ProfileImage, &acc.TotalMedia, &lastFetched, &acc.GroupName, &acc.GroupColor, &acc.MediaType, &acc.Cursor, &completedInt, &responseJSON); err != nil {
			continue
		}
		acc.LastFetched = lastFetched.Format("2006-01-02 15:04")
		acc.Completed = completedInt == 1

		// Extract followers_count and statuses_count from response_json
		if responseJSON != "" {
			var parsed map[string]interface{}
			if err := json.Unmarshal([]byte(responseJSON), &parsed); err == nil {
				if accountInfo, ok := parsed["account_info"].(map[string]interface{}); ok {
					if followers, ok := accountInfo["followers_count"].(float64); ok {
						acc.FollowersCount = int(followers)
					}
					if statuses, ok := accountInfo["statuses_count"].(float64); ok {
						acc.StatusesCount = int(statuses)
					}
				}
			}
		}

		accounts = append(accounts, acc)
	}

	return accounts, nil
}

// UpdateAccountGroup updates the group for an account
func UpdateAccountGroup(id int64, groupName, groupColor string) error {
	if db == nil {
		if err := InitDB(); err != nil {
			return err
		}
	}

	_, err := db.Exec("UPDATE accounts SET group_name = ?, group_color = ? WHERE id = ?", groupName, groupColor, id)
	return err
}

// GetAllGroups returns all unique groups
func GetAllGroups() ([]map[string]string, error) {
	if db == nil {
		if err := InitDB(); err != nil {
			return nil, err
		}
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

// ClearAllAccounts deletes all accounts from the database
func ClearAllAccounts() error {
	if db == nil {
		if err := InitDB(); err != nil {
			return err
		}
	}

	_, err := db.Exec("DELETE FROM accounts")
	return err
}

// GetAccountByID returns a specific account by ID
func GetAccountByID(id int64) (*AccountDB, error) {
	if db == nil {
		if err := InitDB(); err != nil {
			return nil, err
		}
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

	// Convert legacy format if needed
	if converted, err := ConvertLegacyToNewFormat(acc.ResponseJSON); err == nil {
		acc.ResponseJSON = converted
	}

	return &acc, nil
}

// DeleteAccount deletes an account from the database
func DeleteAccount(id int64) error {
	if db == nil {
		if err := InitDB(); err != nil {
			return err
		}
	}

	_, err := db.Exec("DELETE FROM accounts WHERE id = ?", id)
	return err
}

// LegacyMediaEntry represents media entry in old format
type LegacyMediaEntry struct {
	TweetID string `json:"tweet_id"`
	URL     string `json:"url"`
	Date    string `json:"date"`
	Type    string `json:"type"`
}

// LegacyAccountFormat represents the old saved account format
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

// ConvertLegacyToNewFormat converts old format to new TwitterResponse format
func ConvertLegacyToNewFormat(jsonStr string) (string, error) {
	// First check if it's already in new format (has account_info key)
	var check map[string]interface{}
	if err := json.Unmarshal([]byte(jsonStr), &check); err != nil {
		return jsonStr, err
	}

	// If already has account_info, it's new format - return as is
	if _, hasAccountInfo := check["account_info"]; hasAccountInfo {
		return jsonStr, nil
	}

	// Check if it's legacy format (has username and media_list)
	if _, hasUsername := check["username"]; !hasUsername {
		return jsonStr, nil
	}
	if _, hasMediaList := check["media_list"]; !hasMediaList {
		return jsonStr, nil
	}

	// Parse as legacy format
	var legacy LegacyAccountFormat
	if err := json.Unmarshal([]byte(jsonStr), &legacy); err != nil {
		return jsonStr, err
	}

	// Convert timeline entries
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

	// Build new format
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

	// Convert back to JSON
	newJSON, err := json.Marshal(newFormat)
	if err != nil {
		return jsonStr, err
	}

	return string(newJSON), nil
}

// ExportAccountToFile exports account JSON to a file
func ExportAccountToFile(id int64, outputDir string) (string, error) {
	acc, err := GetAccountByID(id)
	if err != nil {
		return "", err
	}

	// Create export directory if not exists
	exportDir := filepath.Join(outputDir, "twitterxmediabatchdownloader_backups")
	if err := os.MkdirAll(exportDir, 0755); err != nil {
		return "", err
	}

	// Use username (nick) for filename
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

// ExportAccountsToTXT exports selected accounts to TXT file (one username per line)
func ExportAccountsToTXT(ids []int64, outputDir string) (string, error) {
	if len(ids) == 0 {
		return "", fmt.Errorf("no accounts to export")
	}

	// Create export directory if not exists
	exportDir := filepath.Join(outputDir, "twitterxmediabatchdownloader_backups")
	if err := os.MkdirAll(exportDir, 0755); err != nil {
		return "", err
	}

	// Get all accounts by IDs
	var usernames []string
	for _, id := range ids {
		acc, err := GetAccountByID(id)
		if err != nil {
			continue // Skip if account not found
		}
		if acc.Username != "" {
			usernames = append(usernames, acc.Username)
		}
	}

	if len(usernames) == 0 {
		return "", fmt.Errorf("no valid usernames found")
	}

	// Create TXT content (one username per line)
	txtContent := ""
	for i, username := range usernames {
		if i > 0 {
			txtContent += "\n"
		}
		txtContent += username
	}

	// Filename: twitterxmediabatchdownloader_multiple.txt
	filePath := filepath.Join(exportDir, "twitterxmediabatchdownloader_multiple.txt")

	if err := os.WriteFile(filePath, []byte(txtContent), 0644); err != nil {
		return "", err
	}

	return filePath, nil
}

// ImportAccountFromFile imports account from JSON file (supports both old and new format)
func ImportAccountFromFile(filePath string) (string, error) {
	if db == nil {
		if err := InitDB(); err != nil {
			return "", err
		}
	}

	// Read file
	data, err := os.ReadFile(filePath)
	if err != nil {
		return "", err
	}

	jsonStr := string(data)

	// Convert legacy format if needed
	convertedJSON, err := ConvertLegacyToNewFormat(jsonStr)
	if err != nil {
		return "", err
	}

	// Parse the converted JSON to extract account info
	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(convertedJSON), &parsed); err != nil {
		return "", err
	}

	// Extract account info
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

	// Save to database with default media type "all" for imported files
	err = SaveAccount(username, name, profileImage, totalURLs, convertedJSON, "all")
	if err != nil {
		return "", err
	}

	return username, nil
}
