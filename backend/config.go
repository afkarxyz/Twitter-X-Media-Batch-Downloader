package backend

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"time"
)

func GetDefaultDownloadPath() string {
	// Get user's home directory
	homeDir, err := os.UserHomeDir()
	if err != nil {
		// Fallback to Public Pictures if can't get home dir
		return "C:\\Users\\Public\\Pictures"
	}

	// Return path to user's Pictures folder
	return filepath.Join(homeDir, "Pictures")
}

// GetProxyURL gets proxy URL from environment variables or custom proxy setting
// Priority: customProxy > HTTP_PROXY/HTTPS_PROXY > http_proxy/https_proxy
func GetProxyURL(customProxy string) (*url.URL, error) {
	// First, check custom proxy setting
	if customProxy != "" {
		proxyURL, err := url.Parse(customProxy)
		if err != nil {
			return nil, fmt.Errorf("invalid custom proxy URL: %v", err)
		}
		return proxyURL, nil
	}

	// Check environment variables (case-insensitive on Windows)
	proxyEnv := os.Getenv("HTTPS_PROXY")
	if proxyEnv == "" {
		proxyEnv = os.Getenv("https_proxy")
	}
	if proxyEnv == "" {
		proxyEnv = os.Getenv("HTTP_PROXY")
	}
	if proxyEnv == "" {
		proxyEnv = os.Getenv("http_proxy")
	}

	if proxyEnv != "" {
		proxyURL, err := url.Parse(proxyEnv)
		if err != nil {
			return nil, fmt.Errorf("invalid proxy URL from environment: %v", err)
		}
		return proxyURL, nil
	}

	return nil, nil // No proxy
}

// CreateHTTPClient creates an HTTP client with proxy support
func CreateHTTPClient(customProxy string, timeout time.Duration) (*http.Client, error) {
	proxyURL, err := GetProxyURL(customProxy)
	if err != nil {
		return nil, err
	}

	transport := &http.Transport{
		Proxy: http.ProxyURL(proxyURL),
	}

	client := &http.Client{
		Transport: transport,
		Timeout:   timeout,
	}

	return client, nil
}
