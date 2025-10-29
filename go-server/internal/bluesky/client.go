// Package bluesky provides AT Protocol client utilities
package bluesky

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
)

// Client represents an AT Protocol client for making authenticated API calls
type Client struct {
	httpClient *http.Client
	creds      *auth.Credentials
	pds        string // Personal Data Server URL
}

// NewClient creates a new AT Protocol client with credentials
func NewClient(creds *auth.Credentials) *Client {
	pds := "https://bsky.social"
	// TODO: Could resolve actual PDS from DID document
	return &Client{
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		creds: creds,
		pds:   pds,
	}
}

// GetRecord retrieves a record from the repository
func (c *Client) GetRecord(ctx context.Context, repo, collection, rkey string) (map[string]interface{}, error) {
	url := fmt.Sprintf("%s/xrpc/com.atproto.repo.getRecord?repo=%s&collection=%s&rkey=%s",
		c.pds, repo, collection, rkey)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply-go/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var result map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return result, nil
}

// CreateRecord creates a new record in the repository
func (c *Client) CreateRecord(ctx context.Context, repo, collection string, record map[string]interface{}) (map[string]interface{}, error) {
	url := fmt.Sprintf("%s/xrpc/com.atproto.repo.createRecord", c.pds)

	requestBody := map[string]interface{}{
		"repo":       repo,
		"collection": collection,
		"record":     record,
	}

	jsonData, err := json.Marshal(requestBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.creds.AccessToken))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "autoreply-go/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var result map[string]interface{}
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return result, nil
}

// DeleteRecord deletes a record from the repository
func (c *Client) DeleteRecord(ctx context.Context, repo, collection, rkey string) error {
	url := fmt.Sprintf("%s/xrpc/com.atproto.repo.deleteRecord", c.pds)

	requestBody := map[string]interface{}{
		"repo":       repo,
		"collection": collection,
		"rkey":       rkey,
	}

	jsonData, err := json.Marshal(requestBody)
	if err != nil {
		return fmt.Errorf("failed to marshal request: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewBuffer(jsonData))
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.creds.AccessToken))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "autoreply-go/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	return nil
}

// ListRecords lists records of a specific collection
func (c *Client) ListRecords(ctx context.Context, repo, collection string, limit int) ([]map[string]interface{}, error) {
	url := fmt.Sprintf("%s/xrpc/com.atproto.repo.listRecords?repo=%s&collection=%s&limit=%d",
		c.pds, repo, collection, limit)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply-go/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var result struct {
		Records []map[string]interface{} `json:"records"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	return result.Records, nil
}
