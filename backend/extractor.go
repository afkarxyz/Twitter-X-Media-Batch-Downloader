package backend

import (
	"archive/zip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

const extractorReleaseAPIURL = "https://api.github.com/repos/afkarxyz/xtractor-binaries/releases/latest"
const extractorReleasesAPIURL = "https://api.github.com/repos/afkarxyz/xtractor-binaries/releases"
const extractorReleasePageSize = 20

type ExtractorVersionStatus struct {
	Installed        bool   `json:"installed"`
	InstalledVersion string `json:"installed_version,omitempty"`
	LatestVersion    string `json:"latest_version,omitempty"`
}

type extractorVersionMetadata struct {
	Version   string `json:"version"`
	AssetName string `json:"asset_name,omitempty"`
}

type githubRelease struct {
	TagName string               `json:"tag_name"`
	Assets  []githubReleaseAsset `json:"assets"`
}

type githubReleaseAsset struct {
	Name               string `json:"name"`
	BrowserDownloadURL string `json:"browser_download_url"`
	Digest             string `json:"digest"`
}

func IsExtractorInstalled() bool {
	info, err := os.Stat(getExtractorPath())
	return err == nil && !info.IsDir()
}

func GetExtractorVersionStatus() ExtractorVersionStatus {
	status := ExtractorVersionStatus{
		Installed: IsExtractorInstalled(),
	}

	assetName, assetErr := getExtractorAssetName()
	if status.Installed && assetErr == nil {
		if installedVersion, err := getInstalledExtractorVersion(assetName); err == nil {
			status.InstalledVersion = installedVersion
		}
	}

	if latestRelease, err := fetchLatestExtractorRelease(); err == nil {
		status.LatestVersion = normalizeExtractorVersion(latestRelease.TagName)
	}

	return status
}

func DownloadExtractor(progressCallback func(downloaded, total int64)) error {
	assetName, err := getExtractorAssetName()
	if err != nil {
		return err
	}

	release, err := fetchLatestExtractorRelease()
	if err != nil {
		return err
	}

	asset, err := findExtractorAsset(release.Assets, assetName)
	if err != nil {
		return err
	}

	tempFile, err := os.CreateTemp("", "xtractor-*.zip")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %v", err)
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	if err := downloadExtractorArchive(asset.BrowserDownloadURL, tempFile, progressCallback); err != nil {
		return err
	}

	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to finalize xtractor archive: %v", err)
	}

	if err := verifyDownloadedDigest(tempPath, asset.Digest); err != nil {
		return err
	}

	KillAllExtractorProcesses()

	exePath := getExtractorPath()
	if err := os.MkdirAll(filepath.Dir(exePath), 0755); err != nil {
		return fmt.Errorf("failed to create xtractor directory: %v", err)
	}

	if err := extractExtractorArchive(tempPath, exePath); err != nil {
		return err
	}

	_ = removeLegacyExtractorBinary()
	_ = writeExtractorVersionMetadata(extractorVersionMetadata{
		Version:   normalizeExtractorVersion(release.TagName),
		AssetName: asset.Name,
	})

	return nil
}

func requireExtractorPath() (string, error) {
	exePath := getExtractorPath()
	info, err := os.Stat(exePath)
	if err != nil || info.IsDir() {
		return "", fmt.Errorf("xtractor dependency is not installed. Please download it from the startup prompt or Settings > Core Xtractor")
	}

	if runtime.GOOS != "windows" {
		_ = os.Chmod(exePath, 0755)
	}

	return exePath, nil
}

func getExtractorAssetName() (string, error) {
	switch runtime.GOOS {
	case "windows":
		switch runtime.GOARCH {
		case "amd64", "arm64":
			return "windows-amd64.zip", nil
		}
	case "linux":
		switch runtime.GOARCH {
		case "amd64":
			return "linux-amd64.zip", nil
		case "arm64":
			return "linux-arm64.zip", nil
		}
	case "darwin":
		switch resolveMacOSExtractorArch(runtime.GOARCH, detectMacOSHardwareArch()) {
		case "amd64":
			return "macos-amd64.zip", nil
		case "arm64":
			return "macos-arm64.zip", nil
		}
	}

	return "", fmt.Errorf("unsupported platform for prebuilt xtractor: %s/%s", runtime.GOOS, runtime.GOARCH)
}

func resolveMacOSExtractorArch(processArch, hardwareArch string) string {
	if hardwareArch == "arm64" {
		return "arm64"
	}

	return processArch
}

func detectMacOSHardwareArch() string {
	if runtime.GOOS != "darwin" {
		return ""
	}

	cmd := exec.Command("sysctl", "-in", "hw.optional.arm64")
	hideWindow(cmd)
	output, err := cmd.Output()
	if err != nil {
		return ""
	}

	if strings.TrimSpace(string(output)) == "1" {
		return "arm64"
	}

	return ""
}

func fetchLatestExtractorRelease() (*githubRelease, error) {
	client, err := CreateHTTPClient("", 30*time.Second)
	if err != nil {
		return nil, fmt.Errorf("failed to configure xtractor network client: %v", err)
	}

	req, err := newExtractorGitHubRequest(extractorReleaseAPIURL)
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch xtractor release info: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to fetch xtractor release info: status %d", resp.StatusCode)
	}

	var release githubRelease
	if err := json.NewDecoder(resp.Body).Decode(&release); err != nil {
		return nil, fmt.Errorf("failed to decode xtractor release info: %v", err)
	}

	return &release, nil
}

func fetchExtractorReleases(limit int) ([]githubRelease, error) {
	client, err := CreateHTTPClient("", 60*time.Second)
	if err != nil {
		return nil, fmt.Errorf("failed to configure xtractor release client: %v", err)
	}

	req, err := newExtractorGitHubRequest(fmt.Sprintf("%s?per_page=%d", extractorReleasesAPIURL, limit))
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch xtractor releases: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("failed to fetch xtractor releases: status %d", resp.StatusCode)
	}

	var releases []githubRelease
	if err := json.NewDecoder(resp.Body).Decode(&releases); err != nil {
		return nil, fmt.Errorf("failed to decode xtractor releases: %v", err)
	}

	return releases, nil
}

func newExtractorGitHubRequest(url string) (*http.Request, error) {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create xtractor GitHub request: %v", err)
	}

	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "Twitter-X-Media-Batch-Downloader")
	return req, nil
}

func findExtractorAsset(assets []githubReleaseAsset, assetName string) (*githubReleaseAsset, error) {
	for i := range assets {
		if assets[i].Name == assetName {
			return &assets[i], nil
		}
	}

	return nil, fmt.Errorf("xtractor asset %q not found in latest release", assetName)
}

func downloadExtractorArchive(downloadURL string, destination *os.File, progressCallback func(downloaded, total int64)) error {
	client, err := CreateHTTPClient("", 10*time.Minute)
	if err != nil {
		return fmt.Errorf("failed to configure xtractor download client: %v", err)
	}

	req, err := http.NewRequest(http.MethodGet, downloadURL, nil)
	if err != nil {
		return fmt.Errorf("failed to create xtractor download request: %v", err)
	}
	req.Header.Set("User-Agent", "Twitter-X-Media-Batch-Downloader")

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("failed to download xtractor: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("failed to download xtractor: status %d", resp.StatusCode)
	}

	total := resp.ContentLength
	var downloaded int64
	buf := make([]byte, 32*1024)

	for {
		n, err := resp.Body.Read(buf)
		if n > 0 {
			if _, writeErr := destination.Write(buf[:n]); writeErr != nil {
				return fmt.Errorf("failed to write xtractor archive: %v", writeErr)
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
			return fmt.Errorf("failed to download xtractor: %v", err)
		}
	}

	return nil
}

func verifyDownloadedDigest(filePath, digest string) error {
	digest = strings.TrimSpace(digest)
	if digest == "" {
		return nil
	}

	expectedHash := strings.TrimPrefix(strings.ToLower(digest), "sha256:")

	file, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("failed to open downloaded xtractor archive: %v", err)
	}
	defer file.Close()

	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return fmt.Errorf("failed to verify xtractor archive: %v", err)
	}

	actualHash := hex.EncodeToString(hash.Sum(nil))
	if actualHash != expectedHash {
		return fmt.Errorf("xtractor archive checksum mismatch")
	}

	return nil
}

func extractExtractorArchive(zipPath, destPath string) error {
	reader, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("failed to open xtractor archive: %v", err)
	}
	defer reader.Close()

	entry, err := findExtractorBinaryEntry(reader.File)
	if err != nil {
		return err
	}

	in, err := entry.Open()
	if err != nil {
		return fmt.Errorf("failed to open xtractor binary from archive: %v", err)
	}
	defer in.Close()

	tempDest := destPath + ".download"
	removeTemp := true
	defer func() {
		if removeTemp {
			_ = os.Remove(tempDest)
		}
	}()

	out, err := os.OpenFile(tempDest, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
	if err != nil {
		return fmt.Errorf("failed to create xtractor binary: %v", err)
	}

	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return fmt.Errorf("failed to extract xtractor binary: %v", err)
	}

	if err := out.Close(); err != nil {
		return fmt.Errorf("failed to finalize xtractor binary: %v", err)
	}

	if err := os.Chmod(tempDest, 0755); err != nil && runtime.GOOS != "windows" {
		return fmt.Errorf("failed to make xtractor executable: %v", err)
	}

	if err := os.Remove(destPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to replace existing xtractor: %v", err)
	}

	if err := os.Rename(tempDest, destPath); err != nil {
		return fmt.Errorf("failed to install xtractor: %v", err)
	}

	removeTemp = false
	return nil
}

func findExtractorBinaryEntry(entries []*zip.File) (*zip.File, error) {
	var fallback *zip.File

	for _, entry := range entries {
		if entry.FileInfo().IsDir() {
			continue
		}

		if fallback == nil {
			fallback = entry
		}

		name := strings.ToLower(filepath.Base(entry.Name))
		if strings.Contains(name, "xtractor") || strings.Contains(name, "extractor") {
			return entry, nil
		}
	}

	if fallback != nil {
		return fallback, nil
	}

	return nil, fmt.Errorf("xtractor binary not found in archive")
}

func getInstalledExtractorVersion(assetName string) (string, error) {
	metadata, err := readExtractorVersionMetadata()
	if err == nil {
		version := normalizeExtractorVersion(metadata.Version)
		if version != "" && (metadata.AssetName == "" || strings.EqualFold(metadata.AssetName, assetName)) {
			return version, nil
		}
	}

	version, err := resolveInstalledExtractorVersionByHash(assetName)
	if err != nil {
		return "", err
	}

	version = normalizeExtractorVersion(version)
	if version != "" {
		_ = writeExtractorVersionMetadata(extractorVersionMetadata{
			Version:   version,
			AssetName: assetName,
		})
	}

	return version, nil
}

func resolveInstalledExtractorVersionByHash(assetName string) (string, error) {
	exePath, err := requireExtractorPath()
	if err != nil {
		return "", err
	}

	installedHash, err := hashFileSHA256(exePath)
	if err != nil {
		return "", fmt.Errorf("failed to hash installed xtractor: %v", err)
	}

	releases, err := fetchExtractorReleases(extractorReleasePageSize)
	if err != nil {
		return "", err
	}

	for _, release := range releases {
		asset, err := findExtractorAsset(release.Assets, assetName)
		if err != nil {
			continue
		}

		matches, err := archiveMatchesExtractorHash(asset.BrowserDownloadURL, installedHash)
		if err != nil {
			continue
		}

		if matches {
			return release.TagName, nil
		}
	}

	return "", nil
}

func archiveMatchesExtractorHash(downloadURL, installedHash string) (bool, error) {
	tempFile, err := os.CreateTemp("", "xtractor-version-*.zip")
	if err != nil {
		return false, fmt.Errorf("failed to create temp archive for version check: %v", err)
	}

	tempPath := tempFile.Name()
	defer os.Remove(tempPath)
	defer tempFile.Close()

	if err := downloadExtractorArchive(downloadURL, tempFile, nil); err != nil {
		return false, err
	}

	if err := tempFile.Close(); err != nil {
		return false, fmt.Errorf("failed to finalize version archive: %v", err)
	}

	archiveHash, err := hashExtractorBinaryInArchive(tempPath)
	if err != nil {
		return false, err
	}

	return archiveHash == installedHash, nil
}

func hashExtractorBinaryInArchive(zipPath string) (string, error) {
	reader, err := zip.OpenReader(zipPath)
	if err != nil {
		return "", fmt.Errorf("failed to open xtractor archive for version check: %v", err)
	}
	defer reader.Close()

	entry, err := findExtractorBinaryEntry(reader.File)
	if err != nil {
		return "", err
	}

	in, err := entry.Open()
	if err != nil {
		return "", fmt.Errorf("failed to open xtractor binary from archive for version check: %v", err)
	}
	defer in.Close()

	return hashReaderSHA256(in)
}

func hashFileSHA256(filePath string) (string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", fmt.Errorf("failed to open xtractor binary: %v", err)
	}
	defer file.Close()

	return hashReaderSHA256(file)
}

func hashReaderSHA256(reader io.Reader) (string, error) {
	hash := sha256.New()
	if _, err := io.Copy(hash, reader); err != nil {
		return "", err
	}

	return hex.EncodeToString(hash.Sum(nil)), nil
}

func getExtractorVersionMetadataPath() string {
	return filepath.Join(filepath.Dir(getExtractorPath()), "xtractor-version.json")
}

func readExtractorVersionMetadata() (extractorVersionMetadata, error) {
	data, err := os.ReadFile(getExtractorVersionMetadataPath())
	if err != nil {
		return extractorVersionMetadata{}, err
	}

	var metadata extractorVersionMetadata
	if err := json.Unmarshal(data, &metadata); err != nil {
		return extractorVersionMetadata{}, err
	}

	return metadata, nil
}

func writeExtractorVersionMetadata(metadata extractorVersionMetadata) error {
	metadata.Version = normalizeExtractorVersion(metadata.Version)
	if metadata.Version == "" {
		return nil
	}

	path := getExtractorVersionMetadataPath()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return fmt.Errorf("failed to create xtractor metadata directory: %v", err)
	}

	data, err := json.Marshal(metadata)
	if err != nil {
		return fmt.Errorf("failed to encode xtractor metadata: %v", err)
	}

	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("failed to write xtractor metadata: %v", err)
	}

	return nil
}

func normalizeExtractorVersion(version string) string {
	return strings.TrimSpace(version)
}

func getLegacyExtractorPath() string {
	homeDir, _ := os.UserHomeDir()
	baseDir := filepath.Join(homeDir, ".twitterxmediabatchdownloader")
	return filepath.Join(baseDir, getLegacyExecutableName())
}

func removeLegacyExtractorBinary() error {
	oldPath := getLegacyExtractorPath()
	info, err := os.Stat(oldPath)
	if err != nil || info.IsDir() {
		return nil
	}

	if err := os.Remove(oldPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove legacy extractor binary: %v", err)
	}

	return nil
}
