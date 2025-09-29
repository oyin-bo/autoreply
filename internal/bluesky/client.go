// client.go - HTTP client utilities
package bluesky

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/oyin-bo/autoreply/pkg/errors"
)

// Client handles HTTP operations for BlueSky API
type Client struct {
	httpClient *http.Client
}

// NewClient creates a new BlueSky client
func NewClient() *Client {
	return &Client{
		httpClient: &http.Client{
			Timeout: 60 * time.Second,
			Transport: &http.Transport{
				MaxIdleConns:        10,
				IdleConnTimeout:     30 * time.Second,
				DisableCompression:  false,
				MaxIdleConnsPerHost: 5,
			},
		},
	}
}

// Get performs a GET request with context
func (c *Client) Get(ctx context.Context, url string) (*http.Response, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("User-Agent", "bluesky-mcp-go/0.1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}

	return resp, nil
}

// GetWithTimeout performs a GET request with a specific timeout
func (c *Client) GetWithTimeout(url string, timeout time.Duration) (*http.Response, error) {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	return c.Get(ctx, url)
}

// StreamDownload downloads a file with streaming and progress tracking
func (c *Client) StreamDownload(ctx context.Context, url string, writer io.Writer) (int64, error) {
	resp, err := c.Get(ctx, url)
	if err != nil {
		return 0, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return 0, errors.NewMcpError(errors.RepoFetchFailed, fmt.Sprintf("HTTP %d: %s", resp.StatusCode, resp.Status))
	}

	// Stream copy with progress tracking
	written, err := io.Copy(writer, resp.Body)
	if err != nil {
		return written, fmt.Errorf("stream copy failed: %w", err)
	}

	return written, nil
}