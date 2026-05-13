package backend

import (
	"archive/zip"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	neturl "net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"time"
)

const (
	ffmpegReleaseAPIURL         = "https://api.github.com/repos/afkarxyz/ffmpeg-binaries/releases/latest"
	ffmpegLatestReleaseURL      = "https://github.com/afkarxyz/ffmpeg-binaries/releases/latest"
	ffmpegLatestDownloadBaseURL = "https://github.com/afkarxyz/ffmpeg-binaries/releases/latest/download"
)

var ffmpegVersionPattern = regexp.MustCompile(`^[0-9]+(?:\.[0-9]+)+`)

type ffmpegVersionMetadata struct {
	Version   string `json:"version"`
	AssetName string `json:"asset_name,omitempty"`
}

type ffmpegResolvedRelease struct {
	Version   string
	AssetURL  string
	AssetName string
	Digest    string
}

func GetFFmpegPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")

	switch runtime.GOOS {
	case "windows":
		return filepath.Join(baseDir, "ffmpeg.exe")
	default:
		return filepath.Join(baseDir, "ffmpeg")
	}
}

func IsFFmpegInstalled() bool {
	ffmpegPath := GetFFmpegPath()
	if _, err := os.Stat(ffmpegPath); err != nil {
		return false
	}

	cmd := exec.Command(ffmpegPath, "-version")
	hideWindow(cmd)
	return cmd.Run() == nil
}

func GetFFmpegVersionStatus() DependencyVersionStatus {
	status := DependencyVersionStatus{
		Installed: IsFFmpegInstalled(),
	}

	if status.Installed {
		if installedVersion, err := getInstalledFFmpegVersion(); err == nil {
			status.InstalledVersion = installedVersion
		}
	}

	if latestVersion, err := fetchLatestFFmpegVersion(); err == nil {
		status.LatestVersion = latestVersion
	}

	return status
}

func DownloadFFmpeg(progressCallback func(downloaded, total int64)) error {
	assetName, err := getFFmpegAssetName()
	if err != nil {
		return err
	}

	releaseInfo, err := resolveLatestFFmpegRelease(assetName)
	if err != nil {
		return err
	}

	tempFile, err := os.CreateTemp("", "ffmpeg-*.zip")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	if err := downloadFFmpegArchive(releaseInfo.AssetURL, tempFile, progressCallback); err != nil {
		return err
	}

	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to finalize ffmpeg archive: %v", err)
	}

	if releaseInfo.Digest != "" {
		if err := verifyDownloadedDigest(tempPath, releaseInfo.Digest); err != nil {
			return err
		}
	}

	ffmpegPath := GetFFmpegPath()
	baseDir := filepath.Dir(ffmpegPath)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return fmt.Errorf("failed to create ffmpeg directory: %v", err)
	}

	if err := extractFFmpegFromZip(tempPath, ffmpegPath); err != nil {
		return err
	}

	_ = writeFFmpegVersionMetadata(ffmpegVersionMetadata{
		Version:   releaseInfo.Version,
		AssetName: releaseInfo.AssetName,
	})

	return nil
}

func getFFmpegAssetName() (string, error) {
	switch runtime.GOOS {
	case "windows":
		return "ffmpeg-windows.zip", nil
	case "linux":
		switch runtime.GOARCH {
		case "amd64":
			return "ffmpeg-linux-amd64.zip", nil
		case "arm64":
			return "ffmpeg-linux-arm64v8.zip", nil
		}
	case "darwin":
		switch runtime.GOARCH {
		case "amd64":
			return "ffmpeg-macos-amd64.zip", nil
		case "arm64":
			return "ffmpeg-macos-arm64.zip", nil
		}
	}

	return "", fmt.Errorf("unsupported platform for ffmpeg binary: %s/%s", runtime.GOOS, runtime.GOARCH)
}

func resolveLatestFFmpegRelease(assetName string) (ffmpegResolvedRelease, error) {
	release, apiErr := fetchLatestFFmpegRelease()
	if apiErr == nil {
		asset, assetErr := findFFmpegAsset(release.Assets, assetName)
		if assetErr == nil {
			return ffmpegResolvedRelease{
				Version:   normalizeFFmpegVersion(release.TagName),
				AssetURL:  asset.BrowserDownloadURL,
				AssetName: asset.Name,
				Digest:    asset.Digest,
			}, nil
		}
		apiErr = assetErr
	}

	fallbackVersion, fallbackErr := fetchLatestFFmpegVersionFromRedirect()
	if fallbackErr != nil {
		return ffmpegResolvedRelease{}, fmt.Errorf("failed to resolve latest ffmpeg release: %v; fallback failed: %v", apiErr, fallbackErr)
	}

	return ffmpegResolvedRelease{
		Version:   fallbackVersion,
		AssetURL:  buildLatestFFmpegDownloadURL(assetName),
		AssetName: assetName,
	}, nil
}

func fetchLatestFFmpegVersion() (string, error) {
	release, err := fetchLatestFFmpegRelease()
	if err == nil {
		version := normalizeFFmpegVersion(release.TagName)
		if version != "" {
			return version, nil
		}
	}

	fallbackVersion, fallbackErr := fetchLatestFFmpegVersionFromRedirect()
	if fallbackErr == nil {
		return fallbackVersion, nil
	}

	if err != nil {
		return "", fmt.Errorf("failed to fetch latest ffmpeg version: %v; fallback failed: %v", err, fallbackErr)
	}

	return "", fmt.Errorf("failed to determine latest ffmpeg version")
}

func fetchLatestFFmpegRelease() (*githubRelease, error) {
	client, err := CreateHTTPClient("", 30*time.Second)
	if err != nil {
		return nil, fmt.Errorf("failed to configure ffmpeg network client: %v", err)
	}

	req, err := newFFmpegGitHubRequest(ffmpegReleaseAPIURL)
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch ffmpeg release info: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to fetch ffmpeg release info: status %d", resp.StatusCode)
	}

	var release githubRelease
	if err := json.NewDecoder(resp.Body).Decode(&release); err != nil {
		return nil, fmt.Errorf("failed to decode ffmpeg release info: %v", err)
	}

	return &release, nil
}

func newFFmpegGitHubRequest(url string) (*http.Request, error) {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create ffmpeg GitHub request: %v", err)
	}

	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "Twitter-X-Media-Batch-Downloader")
	return req, nil
}

func findFFmpegAsset(assets []githubReleaseAsset, assetName string) (*githubReleaseAsset, error) {
	for i := range assets {
		if assets[i].Name == assetName {
			return &assets[i], nil
		}
	}

	return nil, fmt.Errorf("ffmpeg asset %q not found in latest release", assetName)
}

func fetchLatestFFmpegVersionFromRedirect() (string, error) {
	client, err := CreateHTTPClient("", 30*time.Second)
	if err != nil {
		return "", fmt.Errorf("failed to configure ffmpeg redirect client: %v", err)
	}

	client.CheckRedirect = func(req *http.Request, via []*http.Request) error {
		return http.ErrUseLastResponse
	}

	req, err := http.NewRequest(http.MethodGet, ffmpegLatestReleaseURL, nil)
	if err != nil {
		return "", fmt.Errorf("failed to create ffmpeg redirect request: %v", err)
	}
	req.Header.Set("User-Agent", "Twitter-X-Media-Batch-Downloader")

	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("failed to resolve latest ffmpeg redirect: %v", err)
	}
	defer resp.Body.Close()

	location := resp.Header.Get("Location")
	if location == "" && resp.Request != nil && resp.Request.URL != nil {
		location = resp.Request.URL.String()
	}

	version := parseFFmpegVersionFromReleaseURL(location)
	if version == "" {
		return "", fmt.Errorf("failed to parse ffmpeg version from redirect URL")
	}

	return version, nil
}

func parseFFmpegVersionFromReleaseURL(releaseURL string) string {
	parsed, err := neturl.Parse(strings.TrimSpace(releaseURL))
	if err != nil {
		return ""
	}

	segments := strings.Split(strings.Trim(parsed.Path, "/"), "/")
	for index, segment := range segments {
		if (segment == "tag" || segment == "download") && index+1 < len(segments) {
			return normalizeFFmpegVersion(segments[index+1])
		}
	}

	return ""
}

func buildLatestFFmpegDownloadURL(assetName string) string {
	return ffmpegLatestDownloadBaseURL + "/" + assetName
}

func downloadFFmpegArchive(downloadURL string, destination *os.File, progressCallback func(downloaded, total int64)) error {
	client, err := CreateHTTPClient("", 10*time.Minute)
	if err != nil {
		return fmt.Errorf("failed to configure ffmpeg download client: %v", err)
	}

	req, err := http.NewRequest(http.MethodGet, downloadURL, nil)
	if err != nil {
		return fmt.Errorf("failed to create ffmpeg download request: %v", err)
	}
	req.Header.Set("User-Agent", "Twitter-X-Media-Batch-Downloader")

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("failed to download ffmpeg: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download ffmpeg: status %d", resp.StatusCode)
	}

	total := resp.ContentLength
	var downloaded int64
	buf := make([]byte, 32*1024)

	for {
		n, err := resp.Body.Read(buf)
		if n > 0 {
			if _, writeErr := destination.Write(buf[:n]); writeErr != nil {
				return fmt.Errorf("failed to write ffmpeg archive: %v", writeErr)
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
			return fmt.Errorf("failed to download ffmpeg: %v", err)
		}
	}

	return nil
}

func extractFFmpegFromZip(zipPath, destPath string) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open zip: %v", err)
	}
	defer r.Close()

	for _, f := range r.File {
		name := filepath.Base(f.Name)
		if name != "ffmpeg" && name != "ffmpeg.exe" {
			continue
		}

		rc, err := f.Open()
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

		if runtime.GOOS != "windows" {
			if err := os.Chmod(destPath, 0755); err != nil {
				return fmt.Errorf("failed to make ffmpeg executable: %v", err)
			}
		}

		return nil
	}

	return fmt.Errorf("ffmpeg binary not found in archive")
}

func getInstalledFFmpegVersion() (string, error) {
	ffmpegPath := GetFFmpegPath()
	if _, err := os.Stat(ffmpegPath); err != nil {
		return "", fmt.Errorf("ffmpeg not installed")
	}

	cmd := exec.Command(ffmpegPath, "-version")
	hideWindow(cmd)
	output, err := cmd.Output()
	if err == nil {
		version := normalizeFFmpegVersionOutput(string(output))
		if version != "" {
			return version, nil
		}
	}

	metadata, metadataErr := readFFmpegVersionMetadata()
	if metadataErr == nil {
		version := normalizeFFmpegVersion(metadata.Version)
		if version != "" {
			return version, nil
		}
	}

	if err != nil {
		return "", fmt.Errorf("failed to determine ffmpeg version: %v", err)
	}

	return "", fmt.Errorf("failed to determine ffmpeg version")
}

func normalizeFFmpegVersionOutput(output string) string {
	line := strings.TrimSpace(output)
	if line == "" {
		return ""
	}

	line = strings.Split(line, "\n")[0]
	line = strings.TrimSpace(line)
	lowerLine := strings.ToLower(line)
	const prefix = "ffmpeg version "
	if strings.HasPrefix(lowerLine, prefix) {
		line = strings.TrimSpace(line[len(prefix):])
	}

	fields := strings.Fields(line)
	if len(fields) > 0 {
		line = fields[0]
	}

	return normalizeFFmpegVersion(line)
}

func getFFmpegVersionMetadataPath() string {
	return filepath.Join(filepath.Dir(GetFFmpegPath()), "ffmpeg-version.json")
}

func readFFmpegVersionMetadata() (ffmpegVersionMetadata, error) {
	data, err := os.ReadFile(getFFmpegVersionMetadataPath())
	if err != nil {
		return ffmpegVersionMetadata{}, err
	}

	var metadata ffmpegVersionMetadata
	if err := json.Unmarshal(data, &metadata); err != nil {
		return ffmpegVersionMetadata{}, err
	}

	return metadata, nil
}

func writeFFmpegVersionMetadata(metadata ffmpegVersionMetadata) error {
	metadata.Version = normalizeFFmpegVersion(metadata.Version)
	if metadata.Version == "" {
		return nil
	}

	path := getFFmpegVersionMetadataPath()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return fmt.Errorf("failed to create ffmpeg metadata directory: %v", err)
	}

	data, err := json.Marshal(metadata)
	if err != nil {
		return fmt.Errorf("failed to encode ffmpeg metadata: %v", err)
	}

	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("failed to write ffmpeg metadata: %v", err)
	}

	return nil
}

func normalizeFFmpegVersion(version string) string {
	version = strings.TrimSpace(version)
	if version == "" {
		return ""
	}

	lowerVersion := strings.ToLower(version)
	if strings.HasPrefix(lowerVersion, "ffmpeg version ") {
		version = strings.TrimSpace(version[len("ffmpeg version "):])
	}

	fields := strings.Fields(version)
	if len(fields) > 0 {
		version = fields[0]
	}

	if strings.HasPrefix(strings.ToLower(version), "n") && len(version) > 1 {
		next := version[1]
		if next >= '0' && next <= '9' {
			version = version[1:]
		}
	}

	if strings.HasPrefix(strings.ToLower(version), "v") {
		version = strings.TrimPrefix(strings.TrimPrefix(version, "v"), "V")
	}

	if version == "" {
		return ""
	}

	if numericVersion := ffmpegVersionPattern.FindString(version); numericVersion != "" {
		version = numericVersion
	}

	first := version[0]
	if first >= '0' && first <= '9' {
		return "v" + version
	}

	return version
}

func ConvertMP4ToGIF(inputPath, outputPath, quality, resolution string) error {
	ffmpegPath := GetFFmpegPath()

	if !IsFFmpegInstalled() {
		return fmt.Errorf("ffmpeg not installed")
	}

	var args []string

	if quality == "fast" {
		var scaleFilter string
		switch resolution {
		case "high":
			scaleFilter = "scale=800:-1"
		case "medium":
			scaleFilter = "scale=600:-1"
		case "low":
			scaleFilter = "scale=400:-1"
		default:
			scaleFilter = ""
		}

		if scaleFilter != "" {
			args = []string{
				"-i", inputPath,
				"-vf", scaleFilter,
				"-loop", "0",
				"-y",
				outputPath,
			}
		} else {
			args = []string{
				"-i", inputPath,
				"-loop", "0",
				"-y",
				outputPath,
			}
		}
	} else {
		var filter string

		switch resolution {
		case "high":
			filter = "scale=800:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=800:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		case "medium":
			filter = "scale=600:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=600:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		case "low":
			filter = "scale=400:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=400:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a"
		default:
			filter = "palettegen=stats_mode=full[palette];[0:v][palette]paletteuse=dither=sierra2_4a"
		}

		fps := "15"
		if resolution == "medium" {
			fps = "10"
		} else if resolution == "low" {
			fps = "8"
		}

		args = []string{
			"-i", inputPath,
			"-lavfi", filter,
			"-r", fps,
			"-y",
			outputPath,
		}
	}

	cmd := exec.Command(ffmpegPath, args...)
	hideWindow(cmd)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("ffmpeg error: %v, output: %s", err, string(output))
	}

	return nil
}

func ConvertGIFsInFolder(folderPath, quality, resolution string, deleteOriginal bool) (converted int, failed int, err error) {
	if !IsFFmpegInstalled() {
		return 0, 0, fmt.Errorf("ffmpeg not installed")
	}

	cleanPath := filepath.Clean(folderPath)
	gifsFolder := filepath.Join(cleanPath, "gifs")
	if _, err := os.Stat(gifsFolder); os.IsNotExist(err) {
		return 0, 0, fmt.Errorf("gifs folder not found: %s", gifsFolder)
	}

	files, err := os.ReadDir(gifsFolder)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to read gifs folder: %v", err)
	}

	for _, file := range files {
		if file.IsDir() {
			continue
		}

		name := file.Name()
		if !strings.HasSuffix(strings.ToLower(name), ".mp4") {
			continue
		}

		inputPath := filepath.Join(gifsFolder, name)
		outputPath := filepath.Join(gifsFolder, strings.TrimSuffix(name, filepath.Ext(name))+".gif")

		if err := ConvertMP4ToGIF(inputPath, outputPath, quality, resolution); err != nil {
			failed++
			continue
		}

		if deleteOriginal {
			os.Remove(inputPath)
		}

		converted++
	}

	return converted, failed, nil
}
