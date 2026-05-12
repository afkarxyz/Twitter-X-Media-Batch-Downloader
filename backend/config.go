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

	homeDir, err := os.UserHomeDir()
	if err != nil {

		return "C:\\Users\\Public\\Pictures"
	}

	return filepath.Join(homeDir, "Pictures")
}

func GetProxyURL(customProxy string) (*url.URL, error) {

	if customProxy != "" {
		proxyURL, err := url.Parse(customProxy)
		if err != nil {
			return nil, fmt.Errorf("invalid custom proxy URL: %v", err)
		}
		return proxyURL, nil
	}

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

	return nil, nil
}

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
