package backend

import (
	"archive/zip"
	"encoding/json"
	"encoding/xml"
	"fmt"
	"html"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"time"
)

const (
	exiftoolBestReleaseAPIURL = "https://sourceforge.net/projects/exiftool/best_release.json"
	exiftoolRSSURL            = "https://sourceforge.net/projects/exiftool/rss?path=/"
	exiftoolFilesBaseURL      = "https://sourceforge.net/projects/exiftool/files"
	sourceForgeUserAgent      = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"
)

var (
	exiftoolVersionPattern       = regexp.MustCompile(`(?i)(?:exiftool|image-exiftool)-([0-9]+(?:\.[0-9]+)+)`)
	sourceForgeMirrorMetaPattern = regexp.MustCompile(`(?i)url=(https?://downloads\.sourceforge\.net/project/[^"'<> ]+)`)
	sourceForgeMirrorURLPattern  = regexp.MustCompile(`https?://downloads\.sourceforge\.net/project/[^"'<> ]+`)
)

type sourceForgeBestRelease struct {
	Release sourceForgeBestReleaseFile `json:"release"`
}

type sourceForgeBestReleaseFile struct {
	Filename string `json:"filename"`
	URL      string `json:"url"`
}

type sourceForgeRSS struct {
	Channel sourceForgeRSSChannel `xml:"channel"`
}

type sourceForgeRSSChannel struct {
	Items []sourceForgeRSSItem `xml:"item"`
}

type sourceForgeRSSItem struct {
	Title string `xml:"title"`
	Link  string `xml:"link"`
}

func GetExifToolPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")

	switch runtime.GOOS {
	case "windows":
		return filepath.Join(baseDir, "exiftool.exe")
	default:
		pattern := filepath.Join(baseDir, "Image-ExifTool-*")
		matches, err := filepath.Glob(pattern)
		if err == nil && len(matches) > 0 {
			exiftoolPath := filepath.Join(matches[0], "exiftool")
			if _, err := os.Stat(exiftoolPath); err == nil {
				return exiftoolPath
			}
		}

		return filepath.Join(baseDir, "exiftool")
	}
}

func IsExifToolInstalled() bool {
	_, err := getInstalledExifToolVersion()
	return err == nil
}

func GetExifToolVersionStatus() DependencyVersionStatus {
	status := DependencyVersionStatus{
		Installed: IsExifToolInstalled(),
	}

	if status.Installed {
		if installedVersion, err := getInstalledExifToolVersion(); err == nil {
			status.InstalledVersion = installedVersion
		}
	}

	if latestVersion, err := fetchLatestExifToolVersion(); err == nil {
		status.LatestVersion = latestVersion
	}

	return status
}

func getInstalledExifToolVersion() (string, error) {
	exiftoolPath := GetExifToolPath()
	if _, err := os.Stat(exiftoolPath); err != nil {
		return "", fmt.Errorf("exiftool not installed")
	}

	cmd := exec.Command(exiftoolPath, "-ver")
	hideWindow(cmd)
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to determine exiftool version: %v", err)
	}

	version := normalizeExifToolVersion(string(output))
	if version == "" {
		return "", fmt.Errorf("failed to determine exiftool version")
	}

	return version, nil
}

func normalizeExifToolVersion(version string) string {
	version = strings.TrimSpace(version)
	if version == "" {
		return ""
	}

	fields := strings.Fields(version)
	if len(fields) > 0 {
		version = fields[0]
	}

	if strings.HasPrefix(strings.ToLower(version), "v") {
		version = strings.TrimPrefix(strings.TrimPrefix(version, "v"), "V")
	}

	if version == "" {
		return ""
	}

	first := version[0]
	if first >= '0' && first <= '9' {
		return "v" + version
	}

	return version
}

func is64Bit() bool {
	return runtime.GOARCH == "amd64" || runtime.GOARCH == "arm64"
}

func DownloadExifTool(progressCallback func(downloaded, total int64)) error {
	latestVersion, err := fetchLatestExifToolVersion()
	if err != nil {
		return err
	}

	downloadURL, err := buildLatestExifToolDownloadURL(latestVersion)
	if err != nil {
		return err
	}

	tempFile, err := os.CreateTemp("", "exiftool-*")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to prepare temp file: %v", err)
	}

	if err := downloadSourceForgeFile(downloadURL, tempPath, progressCallback); err != nil {
		return err
	}

	exiftoolPath := GetExifToolPath()
	baseDir := filepath.Dir(exiftoolPath)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	switch runtime.GOOS {
	case "windows":
		return extractExifToolFromZip(tempPath, exiftoolPath)
	case "linux", "darwin":
		return extractExifToolFromTarGz(tempPath, exiftoolPath)
	}

	return nil
}

func fetchLatestExifToolVersion() (string, error) {
	version, err := fetchLatestExifToolVersionFromBestRelease()
	if err == nil {
		return version, nil
	}

	fallbackVersion, fallbackErr := fetchLatestExifToolVersionFromRSS()
	if fallbackErr == nil {
		return fallbackVersion, nil
	}

	return "", fmt.Errorf("failed to fetch latest exiftool version: %v; fallback failed: %v", err, fallbackErr)
}

func fetchLatestExifToolVersionFromBestRelease() (string, error) {
	client, err := CreateHTTPClient("", 30*time.Second)
	if err != nil {
		return "", fmt.Errorf("failed to configure exiftool release client: %v", err)
	}

	req, err := http.NewRequest(http.MethodGet, exiftoolBestReleaseAPIURL, nil)
	if err != nil {
		return "", fmt.Errorf("failed to create exiftool release request: %v", err)
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", sourceForgeUserAgent)

	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("failed to fetch exiftool release info: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("failed to fetch exiftool release info: status %d", resp.StatusCode)
	}

	var bestRelease sourceForgeBestRelease
	if err := json.NewDecoder(resp.Body).Decode(&bestRelease); err != nil {
		return "", fmt.Errorf("failed to decode exiftool release info: %v", err)
	}

	version := parseExifToolVersion(bestRelease.Release.Filename)
	if version == "" {
		version = parseExifToolVersion(bestRelease.Release.URL)
	}
	if version == "" {
		return "", fmt.Errorf("failed to parse exiftool version from best_release.json")
	}

	return version, nil
}

func fetchLatestExifToolVersionFromRSS() (string, error) {
	client, err := CreateHTTPClient("", 30*time.Second)
	if err != nil {
		return "", fmt.Errorf("failed to configure exiftool RSS client: %v", err)
	}

	req, err := http.NewRequest(http.MethodGet, exiftoolRSSURL, nil)
	if err != nil {
		return "", fmt.Errorf("failed to create exiftool RSS request: %v", err)
	}
	req.Header.Set("Accept", "application/rss+xml, application/xml, text/xml")
	req.Header.Set("User-Agent", sourceForgeUserAgent)

	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("failed to fetch exiftool RSS: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("failed to fetch exiftool RSS: status %d", resp.StatusCode)
	}

	var feed sourceForgeRSS
	if err := xml.NewDecoder(resp.Body).Decode(&feed); err != nil {
		return "", fmt.Errorf("failed to decode exiftool RSS: %v", err)
	}

	for _, item := range feed.Channel.Items {
		version := parseExifToolVersion(item.Title)
		if version == "" {
			version = parseExifToolVersion(item.Link)
		}
		if version != "" {
			return version, nil
		}
	}

	return "", fmt.Errorf("failed to find exiftool version in RSS feed")
}

func parseExifToolVersion(value string) string {
	matches := exiftoolVersionPattern.FindStringSubmatch(value)
	if len(matches) < 2 {
		return ""
	}

	return normalizeExifToolVersion(matches[1])
}

func buildLatestExifToolDownloadURL(version string) (string, error) {
	version = normalizeExifToolVersion(version)
	if version == "" {
		return "", fmt.Errorf("missing exiftool version")
	}
	version = strings.TrimPrefix(strings.TrimPrefix(version, "v"), "V")

	switch runtime.GOOS {
	case "windows":
		if is64Bit() {
			return fmt.Sprintf("%s/exiftool-%s_64.zip/download", exiftoolFilesBaseURL, version), nil
		}
		return fmt.Sprintf("%s/exiftool-%s_32.zip/download", exiftoolFilesBaseURL, version), nil
	case "linux", "darwin":
		return fmt.Sprintf("%s/Image-ExifTool-%s.tar.gz/download", exiftoolFilesBaseURL, version), nil
	default:
		return "", fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}
}

func downloadSourceForgeFile(downloadURL, destinationPath string, progressCallback func(downloaded, total int64)) error {
	client, err := CreateHTTPClient("", 10*time.Minute)
	if err != nil {
		return fmt.Errorf("failed to configure SourceForge download client: %v", err)
	}

	err = downloadSourceForgeURL(client, downloadURL, destinationPath, progressCallback, 0)
	if err == nil {
		return nil
	}

	if runtime.GOOS == "windows" {
		if fallbackErr := downloadSourceForgeFileWithCurl(downloadURL, destinationPath); fallbackErr == nil {
			return nil
		} else {
			return fmt.Errorf("%v; curl fallback failed: %v", err, fallbackErr)
		}
	}

	return err
}

func downloadSourceForgeURL(client *http.Client, downloadURL, destinationPath string, progressCallback func(downloaded, total int64), depth int) error {
	if depth > 4 {
		return fmt.Errorf("failed to resolve SourceForge mirror URL")
	}

	req, err := http.NewRequest(http.MethodGet, downloadURL, nil)
	if err != nil {
		return fmt.Errorf("failed to create SourceForge download request: %v", err)
	}
	req.Header.Set("User-Agent", sourceForgeUserAgent)

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("failed to download exiftool: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download exiftool: status %d", resp.StatusCode)
	}

	contentType := strings.ToLower(resp.Header.Get("Content-Type"))
	if strings.Contains(contentType, "text/html") {
		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return fmt.Errorf("failed to read SourceForge download page: %v", err)
		}

		resolvedURL := extractSourceForgeMirrorURL(string(body))
		if resolvedURL == "" {
			return fmt.Errorf("failed to resolve SourceForge mirror URL from download page")
		}

		return downloadSourceForgeURL(client, resolvedURL, destinationPath, progressCallback, depth+1)
	}

	destination, err := os.OpenFile(destinationPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0644)
	if err != nil {
		return fmt.Errorf("failed to create exiftool archive: %v", err)
	}
	defer destination.Close()

	total := resp.ContentLength
	var downloaded int64
	buf := make([]byte, 32*1024)

	for {
		n, err := resp.Body.Read(buf)
		if n > 0 {
			if _, writeErr := destination.Write(buf[:n]); writeErr != nil {
				return fmt.Errorf("failed to write exiftool archive: %v", writeErr)
			}
			downloaded += int64(n)
			if progressCallback != nil {
				progressCallback(downloaded, total)
			}
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("failed to download exiftool: %v", err)
		}
	}

	return nil
}

func downloadSourceForgeFileWithCurl(downloadURL, destinationPath string) error {
	curlPath, err := exec.LookPath("curl.exe")
	if err != nil {
		curlPath, err = exec.LookPath("curl")
		if err != nil {
			return fmt.Errorf("curl executable not found")
		}
	}

	cmd := exec.Command(curlPath, "-L", "-o", destinationPath, downloadURL)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to download via curl: %v, output: %s", err, string(output))
	}

	return nil
}

func extractSourceForgeMirrorURL(page string) string {
	page = html.UnescapeString(page)

	if matches := sourceForgeMirrorMetaPattern.FindStringSubmatch(page); len(matches) > 1 {
		return strings.TrimSpace(matches[1])
	}

	return strings.TrimSpace(sourceForgeMirrorURLPattern.FindString(page))
}

func extractExifToolFromZip(zipPath, destPath string) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open zip: %v", err)
	}
	defer r.Close()

	var exiftoolFile *zip.File
	for _, f := range r.File {
		name := filepath.Base(f.Name)

		if name == "exiftool(-k).exe" || name == "exiftool.exe" {
			exiftoolFile = f
			break
		}
	}

	if exiftoolFile == nil {
		return fmt.Errorf("exiftool.exe not found in archive")
	}

	rc, err := exiftoolFile.Open()
	if err != nil {
		return fmt.Errorf("failed to open file in zip: %v", err)
	}
	defer rc.Close()

	out, err := os.Create(destPath)
	if err != nil {
		return fmt.Errorf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, rc); err != nil {
		return fmt.Errorf("failed to extract file: %v", err)
	}

	libDir := filepath.Join(filepath.Dir(destPath), "exiftool_files")
	libDirAbs, err := filepath.Abs(libDir)
	if err != nil {
		return fmt.Errorf("failed to resolve libDir: %v", err)
	}
	for _, f := range r.File {
		if strings.Contains(f.Name, "exiftool_files") && !strings.HasSuffix(f.Name, "/") {
			parts := strings.Split(f.Name, "exiftool_files/")
			if len(parts) < 2 {
				continue
			}
			relPath := parts[1]
			if relPath == "" {
				continue
			}

			targetPath := filepath.Join(libDir, relPath)

			targetAbs, err := filepath.Abs(targetPath)
			if err != nil {
				continue
			}
			if !strings.HasPrefix(targetAbs+string(os.PathSeparator), libDirAbs+string(os.PathSeparator)) && targetAbs != libDirAbs {
				continue
			}

			if err := os.MkdirAll(filepath.Dir(targetPath), 0755); err != nil {
				continue
			}

			if f.FileInfo().IsDir() {
				os.MkdirAll(targetPath, 0755)
				continue
			}

			rc, err := f.Open()
			if err != nil {
				continue
			}

			outFile, err := os.Create(targetPath)
			if err != nil {
				rc.Close()
				continue
			}

			io.Copy(outFile, rc)
			outFile.Close()
			rc.Close()
		}
	}

	return nil
}

func extractExifToolFromTarGz(tarGzPath, destPath string) error {
	baseDir := filepath.Dir(destPath)

	cmd := exec.Command("tar", "-xzf", tarGzPath, "-C", baseDir)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("failed to extract tar.gz: %v, output: %s", err, string(output))
	}

	var extractedDir string
	var exiftoolScript string

	pattern := filepath.Join(baseDir, "Image-ExifTool-*")
	matches, err := filepath.Glob(pattern)
	if err == nil && len(matches) > 0 {
		extractedDir = matches[0]
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	} else {
		extractedDir = filepath.Join(baseDir, "exiftool")
		exiftoolScript = filepath.Join(extractedDir, "exiftool")
	}

	if _, err := os.Stat(exiftoolScript); err != nil {
		return fmt.Errorf("exiftool script not found in extracted archive (searched in: %s and %s)", pattern, extractedDir)
	}

	if err := os.Chmod(exiftoolScript, 0755); err != nil {
		return fmt.Errorf("failed to make exiftool executable: %v", err)
	}

	return nil
}
