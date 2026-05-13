package backend

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"time"
)

func GetDefaultDownloadPath() string {

	homeDir, err := os.UserHomeDir()
	if err != nil {

		return "C:\\Users\\Public\\Pictures"
	}

	return filepath.Join(homeDir, "Pictures")
}

func parseProxyURLs(rawValue string, sourceName string) ([]*url.URL, error) {
	if strings.TrimSpace(rawValue) == "" {
		return nil, nil
	}

	parts := strings.Split(rawValue, ",")
	proxyURLs := make([]*url.URL, 0, len(parts))

	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed == "" {
			continue
		}

		proxyURL, err := url.Parse(trimmed)
		if err != nil {
			return nil, fmt.Errorf("invalid proxy URL in %s: %v", sourceName, err)
		}
		if proxyURL.Scheme == "" || proxyURL.Host == "" {
			return nil, fmt.Errorf("invalid proxy URL in %s: %q", sourceName, trimmed)
		}

		proxyURLs = append(proxyURLs, proxyURL)
	}

	if len(proxyURLs) == 0 {
		return nil, nil
	}

	return proxyURLs, nil
}

func GetProxyURLs(customProxy string) ([]*url.URL, error) {

	if customProxy != "" {
		return parseProxyURLs(customProxy, "custom proxy setting")
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
		return parseProxyURLs(proxyEnv, "proxy environment variable")
	}

	return nil, nil
}

func GetProxyURL(customProxy string) (*url.URL, error) {
	proxyURLs, err := GetProxyURLs(customProxy)
	if err != nil || len(proxyURLs) == 0 {
		return nil, err
	}
	return proxyURLs[0], nil
}

func buildProxySelector(proxyURLs []*url.URL) func(*http.Request) (*url.URL, error) {
	if len(proxyURLs) == 0 {
		return nil
	}
	if len(proxyURLs) == 1 {
		return http.ProxyURL(proxyURLs[0])
	}

	var requestCount uint64
	return func(*http.Request) (*url.URL, error) {
		index := atomic.AddUint64(&requestCount, 1) - 1
		return proxyURLs[index%uint64(len(proxyURLs))], nil
	}
}

func CreateHTTPClient(customProxy string, timeout time.Duration) (*http.Client, error) {
	proxyURLs, err := GetProxyURLs(customProxy)
	if err != nil {
		return nil, err
	}

	transport := &http.Transport{
		Proxy: buildProxySelector(proxyURLs),
	}

	client := &http.Client{
		Transport: transport,
		Timeout:   timeout,
	}

	return client, nil
}
