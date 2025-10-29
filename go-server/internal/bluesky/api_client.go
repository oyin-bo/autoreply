// Package bluesky provides Bluesky API client functionality
package bluesky

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
)

// APIClient handles authenticated and unauthenticated Bluesky API requests
type APIClient struct {
	client    *http.Client
	credStore *auth.CredentialStore
}

// NewAPIClient creates a new API client
func NewAPIClient() (*APIClient, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &APIClient{
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
		credStore: credStore,
	}, nil
}

// GetWithAuth makes an authenticated GET request to the Bluesky API
func (c *APIClient) GetWithAuth(ctx context.Context, handle, endpoint string, params map[string]string) (map[string]interface{}, error) {
	// Get credentials for the handle
	creds, err := c.credStore.Load(handle)
	if err != nil {
		return nil, fmt.Errorf("failed to load credentials for %s: %w", handle, err)
	}

	// Build URL with query parameters
	baseURL := "https://bsky.social/xrpc/" + endpoint
	if len(params) > 0 {
		u, err := url.Parse(baseURL)
		if err != nil {
			return nil, fmt.Errorf("invalid endpoint URL: %w", err)
		}
		q := u.Query()
		for k, v := range params {
			q.Set(k, v)
		}
		u.RawQuery = q.Encode()
		baseURL = u.String()
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, "GET", baseURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	// Add authorization header
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", "autoreply/1.0")

	// Execute request
	resp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	// Read response body
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	// Check status code
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("API request failed with status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var result map[string]interface{}
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return result, nil
}

// GetPublic makes an unauthenticated GET request to the public Bluesky API
func (c *APIClient) GetPublic(ctx context.Context, endpoint string, params map[string]string) (map[string]interface{}, error) {
	// Build URL with query parameters
	baseURL := "https://public.api.bsky.app/xrpc/" + endpoint
	if len(params) > 0 {
		u, err := url.Parse(baseURL)
		if err != nil {
			return nil, fmt.Errorf("invalid endpoint URL: %w", err)
		}
		q := u.Query()
		for k, v := range params {
			q.Set(k, v)
		}
		u.RawQuery = q.Encode()
		baseURL = u.String()
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, "GET", baseURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	// Add headers
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", "autoreply/1.0")

	// Execute request
	resp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	// Read response body
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	// Check status code
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("API request failed with status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var result map[string]interface{}
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return result, nil
}

// GetWithOptionalAuth makes a request that tries authenticated first, falls back to public
func (c *APIClient) GetWithOptionalAuth(ctx context.Context, handle, endpoint string, params map[string]string) (map[string]interface{}, error) {
	// If handle is provided and not "anonymous", try authenticated request
	if handle != "" && handle != "anonymous" {
		result, err := c.GetWithAuth(ctx, handle, endpoint, params)
		if err == nil {
			return result, nil
		}
		// Fall through to public request on error
	}

	// Use public API
	return c.GetPublic(ctx, endpoint, params)
}
