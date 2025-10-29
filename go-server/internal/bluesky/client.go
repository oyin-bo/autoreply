// Package bluesky provides BlueSky API client functionality
package bluesky

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
)

// Client provides methods to interact with BlueSky API
type Client struct {
	httpClient     *http.Client
	credStore      *auth.CredentialStore
	sessionManager *auth.SessionManager
}

// NewClient creates a new BlueSky API client
func NewClient() (*Client, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &Client{
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		credStore:      credStore,
		sessionManager: auth.NewSessionManager(),
	}, nil
}

// GetCredentials retrieves credentials for a handle (or default if empty)
func (c *Client) GetCredentials(handle string) (*auth.Credentials, error) {
	if handle == "" {
		defaultHandle, err := c.credStore.GetDefault()
		if err != nil {
			return nil, fmt.Errorf("no handle provided and no default handle set: %w", err)
		}
		handle = defaultHandle
	}

	creds, err := c.credStore.Load(handle)
	if err != nil {
		return nil, fmt.Errorf("failed to load credentials for %s: %w", handle, err)
	}

	return creds, nil
}

// MakeAuthenticatedRequest makes an authenticated API request
func (c *Client) MakeAuthenticatedRequest(ctx context.Context, method, endpoint string, params url.Values, handle string) ([]byte, error) {
	// Get credentials
	creds, err := c.GetCredentials(handle)
	if err != nil {
		return nil, err
	}

	// Build URL with query parameters
	apiURL := fmt.Sprintf("https://bsky.social/xrpc/%s", endpoint)
	if len(params) > 0 {
		apiURL = fmt.Sprintf("%s?%s", apiURL, params.Encode())
	}

	req, err := http.NewRequestWithContext(ctx, method, apiURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, fmt.Errorf("API request failed with status %d: %v", resp.StatusCode, errorResp)
	}

	var result []byte
	decoder := json.NewDecoder(resp.Body)
	var data json.RawMessage
	if err := decoder.Decode(&data); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	result = []byte(data)
	return result, nil
}

// MakePublicRequest makes an unauthenticated API request to the public API
func (c *Client) MakePublicRequest(ctx context.Context, method, endpoint string, params url.Values) ([]byte, error) {
	// Use public API endpoint
	apiURL := fmt.Sprintf("https://public.api.bsky.app/xrpc/%s", endpoint)
	if len(params) > 0 {
		apiURL = fmt.Sprintf("%s?%s", apiURL, params.Encode())
	}

	req, err := http.NewRequestWithContext(ctx, method, apiURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, fmt.Errorf("API request failed with status %d: %v", resp.StatusCode, errorResp)
	}

	var result []byte
	decoder := json.NewDecoder(resp.Body)
	var data json.RawMessage
	if err := decoder.Decode(&data); err != nil {
		return nil, fmt.Errorf("failed to decode response: %w", err)
	}

	result = []byte(data)
	return result, nil
}

// FeedPost represents a post in a feed
type FeedPost struct {
	URI         string         `json:"uri"`
	CID         string         `json:"cid"`
	Author      Author         `json:"author"`
	Record      FeedPostRecord `json:"record"`
	IndexedAt   string         `json:"indexedAt"`
	LikeCount   int            `json:"likeCount,omitempty"`
	ReplyCount  int            `json:"replyCount,omitempty"`
	RepostCount int            `json:"repostCount,omitempty"`
	QuoteCount  int            `json:"quoteCount,omitempty"`
}

// Author represents a post author
type Author struct {
	DID         string `json:"did"`
	Handle      string `json:"handle"`
	DisplayName string `json:"displayName,omitempty"`
	Avatar      string `json:"avatar,omitempty"`
}

// FeedPostRecord represents the record content of a post in a feed
type FeedPostRecord struct {
	Text      string    `json:"text"`
	CreatedAt string    `json:"createdAt"`
	Reply     *ReplyRef `json:"reply,omitempty"`
}

// ReplyRef represents a reply reference
type ReplyRef struct {
	Root   *StrongRef `json:"root,omitempty"`
	Parent *StrongRef `json:"parent,omitempty"`
}

// StrongRef represents a strong reference to a record
type StrongRef struct {
	URI string `json:"uri"`
	CID string `json:"cid"`
}

// FeedResponse represents the response from getFeed API
type FeedResponse struct {
	Feed   []FeedItem `json:"feed"`
	Cursor string     `json:"cursor,omitempty"`
}

// FeedItem wraps a post in a feed
type FeedItem struct {
	Post FeedPost `json:"post"`
}

// ThreadResponse represents the response from getPostThread API
type ThreadResponse struct {
	Thread ThreadNode `json:"thread"`
}

// ThreadNode represents a node in a thread
type ThreadNode struct {
	Type    string       `json:"$type,omitempty"`
	Post    *FeedPost    `json:"post,omitempty"`
	Parent  *ThreadNode  `json:"parent,omitempty"`
	Replies []ThreadNode `json:"replies,omitempty"`
}

// GetFeed retrieves a feed (timeline or custom feed)
func (c *Client) GetFeed(ctx context.Context, handle, feedURI, cursor string, limit int) (*FeedResponse, error) {
	params := url.Values{}
	
	if feedURI != "" {
		params.Set("feed", feedURI)
	} else {
		// Default to What's Hot feed if no feed specified and no handle
		params.Set("feed", "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot")
	}
	
	if cursor != "" {
		params.Set("cursor", cursor)
	}
	
	if limit > 0 {
		params.Set("limit", fmt.Sprintf("%d", limit))
	}

	var data []byte
	var err error
	
	// Try authenticated request first if handle provided
	if handle != "" {
		data, err = c.MakeAuthenticatedRequest(ctx, "GET", "app.bsky.feed.getFeed", params, handle)
		if err != nil {
			// Fall back to public request if authentication fails
			data, err = c.MakePublicRequest(ctx, "GET", "app.bsky.feed.getFeed", params)
		}
	} else {
		data, err = c.MakePublicRequest(ctx, "GET", "app.bsky.feed.getFeed", params)
	}
	
	if err != nil {
		return nil, err
	}

	var response FeedResponse
	if err := json.Unmarshal(data, &response); err != nil {
		return nil, fmt.Errorf("failed to parse feed response: %w", err)
	}

	return &response, nil
}

// GetTimeline retrieves the authenticated user's timeline
func (c *Client) GetTimeline(ctx context.Context, handle, cursor string, limit int) (*FeedResponse, error) {
	params := url.Values{}
	
	if cursor != "" {
		params.Set("cursor", cursor)
	}
	
	if limit > 0 {
		params.Set("limit", fmt.Sprintf("%d", limit))
	}

	data, err := c.MakeAuthenticatedRequest(ctx, "GET", "app.bsky.feed.getTimeline", params, handle)
	if err != nil {
		return nil, err
	}

	var response FeedResponse
	if err := json.Unmarshal(data, &response); err != nil {
		return nil, fmt.Errorf("failed to parse timeline response: %w", err)
	}

	return &response, nil
}

// GetPostThread retrieves a thread by post URI
func (c *Client) GetPostThread(ctx context.Context, handle, postURI string) (*ThreadResponse, error) {
	params := url.Values{}
	params.Set("uri", postURI)

	var data []byte
	var err error
	
	// Try authenticated request first if handle provided
	if handle != "" {
		data, err = c.MakeAuthenticatedRequest(ctx, "GET", "app.bsky.feed.getPostThread", params, handle)
		if err != nil {
			// Fall back to public request if authentication fails
			data, err = c.MakePublicRequest(ctx, "GET", "app.bsky.feed.getPostThread", params)
		}
	} else {
		data, err = c.MakePublicRequest(ctx, "GET", "app.bsky.feed.getPostThread", params)
	}
	
	if err != nil {
		return nil, err
	}

	var response ThreadResponse
	if err := json.Unmarshal(data, &response); err != nil {
		return nil, fmt.Errorf("failed to parse thread response: %w", err)
	}

	return &response, nil
}
