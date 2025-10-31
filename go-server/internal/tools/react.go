// Package tools provides MCP tool implementations
package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// ReactTool implements the react tool for batch post operations (like, unlike, repost, delete)
type ReactTool struct {
	credStore *auth.CredentialStore
	client    *http.Client
}

// NewReactTool creates a new react tool
func NewReactTool() (*ReactTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &ReactTool{
		credStore: credStore,
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
	}, nil
}

// Name returns the tool name
func (t *ReactTool) Name() string {
	return "react"
}

// Description returns the tool description
func (t *ReactTool) Description() string {
	return "Perform batch reactions on posts: like, unlike, repost, and delete. Supports mixing at:// URIs and https://bsky.app/... URLs."
}

// InputSchema returns the JSON schema for tool input
func (t *ReactTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"reactAs": {
				Type:        "string",
				Description: "Handle or DID to react as (uses default account if not specified)",
			},
			"like": {
				Type:        "string",
				Description: "Post URIs/URLs to like (comma or newline separated)",
			},
			"unlike": {
				Type:        "string",
				Description: "Post URIs/URLs to unlike (remove like) (comma or newline separated)",
			},
			"repost": {
				Type:        "string",
				Description: "Post URIs/URLs to repost (comma or newline separated)",
			},
			"delete": {
				Type:        "string",
				Description: "Post URIs/URLs to delete (must be your own posts) (comma or newline separated)",
			},
		},
	}
}

// Call executes the react tool
func (t *ReactTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract reactAs parameter (optional, uses default if not provided)
	var reactAs string
	if reactAsRaw, ok := args["reactAs"]; ok {
		if reactAsStr, ok := reactAsRaw.(string); ok {
			reactAs = strings.TrimSpace(strings.TrimPrefix(reactAsStr, "@"))
		}
	}

	// Get credentials for the user
	creds, err := t.getCredentials(reactAs)
	if err != nil {
		return nil, err
	}

	// Process all operations
	results := &OperationResults{
		Successes: []string{},
		Failures:  []OperationFailure{},
	}

	// Process like operations
	for _, uri := range parseURIList(args["like"]) {
		if err := t.likePost(ctx, creds, uri); err != nil {
			results.Failures = append(results.Failures, OperationFailure{
				Operation: "like",
				URI:       uri,
				Error:     err.Error(),
			})
		} else {
			results.Successes = append(results.Successes, fmt.Sprintf("Liked: %s", uri))
		}
	}

	// Process unlike operations
	for _, uri := range parseURIList(args["unlike"]) {
		if err := t.unlikePost(ctx, creds, uri); err != nil {
			results.Failures = append(results.Failures, OperationFailure{
				Operation: "unlike",
				URI:       uri,
				Error:     err.Error(),
			})
		} else {
			results.Successes = append(results.Successes, fmt.Sprintf("Unliked: %s", uri))
		}
	}

	// Process repost operations
	for _, uri := range parseURIList(args["repost"]) {
		if err := t.repostPost(ctx, creds, uri); err != nil {
			results.Failures = append(results.Failures, OperationFailure{
				Operation: "repost",
				URI:       uri,
				Error:     err.Error(),
			})
		} else {
			results.Successes = append(results.Successes, fmt.Sprintf("Reposted: %s", uri))
		}
	}

	// Process delete operations
	for _, uri := range parseURIList(args["delete"]) {
		if err := t.deletePost(ctx, creds, uri); err != nil {
			results.Failures = append(results.Failures, OperationFailure{
				Operation: "delete",
				URI:       uri,
				Error:     err.Error(),
			})
		} else {
			results.Successes = append(results.Successes, fmt.Sprintf("Deleted: %s", uri))
		}
	}

	// Build response message
	message := t.formatResults(results)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
		IsError: len(results.Failures) > 0 && len(results.Successes) == 0,
	}, nil
}

// OperationResults tracks successes and failures of batch operations
type OperationResults struct {
	Successes []string
	Failures  []OperationFailure
}

// OperationFailure represents a failed operation
type OperationFailure struct {
	Operation string
	URI       string
	Error     string
}

// getCredentials retrieves credentials for a user (by handle or DID)
func (t *ReactTool) getCredentials(reactAs string) (*auth.Credentials, error) {
	// If no reactAs specified, use default
	if reactAs == "" {
		defaultHandle, err := t.credStore.GetDefault()
		if err != nil {
			return nil, errors.NewMCPError(errors.InvalidInput, "No default account set. Please login first or specify reactAs parameter.")
		}
		reactAs = defaultHandle
	}

	// Load credentials
	creds, err := t.credStore.Load(reactAs)
	if err != nil {
		return nil, errors.Wrap(err, errors.NotFound, fmt.Sprintf("No credentials found for %s. Please login first.", reactAs))
	}

	return creds, nil
}

// likePost creates a like record for a post
func (t *ReactTool) likePost(ctx context.Context, creds *auth.Credentials, postURI string) error {
	// Parse the post reference
	postRef, err := parsePostReference(postURI)
	if err != nil {
		return err
	}

	// First, get the post to get its CID
	post, err := t.getPost(ctx, creds, postRef)
	if err != nil {
		return err
	}

	// Create like record
	endpoint := "https://bsky.social/xrpc/com.atproto.repo.createRecord"

	record := map[string]interface{}{
		"$type": "app.bsky.feed.like",
		"subject": map[string]interface{}{
			"uri": post.URI,
			"cid": post.CID,
		},
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	requestBody := map[string]interface{}{
		"repo":       creds.DID,
		"collection": "app.bsky.feed.like",
		"record":     record,
	}

	return t.makeAuthenticatedRequest(ctx, creds, "POST", endpoint, requestBody)
}

// unlikePost removes a like from a post
func (t *ReactTool) unlikePost(ctx context.Context, creds *auth.Credentials, postURI string) error {
	// Parse the post reference
	postRef, err := parsePostReference(postURI)
	if err != nil {
		return err
	}

	// First, find the like record
	likeURI, err := t.findLikeRecord(ctx, creds, postRef)
	if err != nil {
		return err
	}

	if likeURI == "" {
		return fmt.Errorf("no like found for post %s", postURI)
	}

	// Parse the like URI to extract rkey
	parts := strings.Split(strings.TrimPrefix(likeURI, "at://"), "/")
	if len(parts) < 3 {
		return fmt.Errorf("invalid like URI format: %s", likeURI)
	}
	rkey := parts[2]

	// Delete the like record
	endpoint := "https://bsky.social/xrpc/com.atproto.repo.deleteRecord"

	requestBody := map[string]interface{}{
		"repo":       creds.DID,
		"collection": "app.bsky.feed.like",
		"rkey":       rkey,
	}

	return t.makeAuthenticatedRequest(ctx, creds, "POST", endpoint, requestBody)
}

// repostPost creates a repost record for a post
func (t *ReactTool) repostPost(ctx context.Context, creds *auth.Credentials, postURI string) error {
	// Parse the post reference
	postRef, err := parsePostReference(postURI)
	if err != nil {
		return err
	}

	// Get the post to get its CID
	post, err := t.getPost(ctx, creds, postRef)
	if err != nil {
		return err
	}

	// Create repost record
	endpoint := "https://bsky.social/xrpc/com.atproto.repo.createRecord"

	record := map[string]interface{}{
		"$type": "app.bsky.feed.repost",
		"subject": map[string]interface{}{
			"uri": post.URI,
			"cid": post.CID,
		},
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	requestBody := map[string]interface{}{
		"repo":       creds.DID,
		"collection": "app.bsky.feed.repost",
		"record":     record,
	}

	return t.makeAuthenticatedRequest(ctx, creds, "POST", endpoint, requestBody)
}

// deletePost deletes a post (must be user's own post)
func (t *ReactTool) deletePost(ctx context.Context, creds *auth.Credentials, postURI string) error {
	// Parse the post reference
	postRef, err := parsePostReference(postURI)
	if err != nil {
		return err
	}

	// Verify the post belongs to the user
	if postRef.DID != creds.DID {
		return fmt.Errorf("cannot delete post that doesn't belong to you")
	}

	// Delete the post record
	endpoint := "https://bsky.social/xrpc/com.atproto.repo.deleteRecord"

	requestBody := map[string]interface{}{
		"repo":       creds.DID,
		"collection": "app.bsky.feed.post",
		"rkey":       postRef.RKey,
	}

	return t.makeAuthenticatedRequest(ctx, creds, "POST", endpoint, requestBody)
}

// PostInfo represents basic post information
type PostInfo struct {
	URI string
	CID string
}

// getPost fetches a post's URI and CID
func (t *ReactTool) getPost(ctx context.Context, creds *auth.Credentials, postRef *PostReference) (*PostInfo, error) {
	endpoint := fmt.Sprintf("https://bsky.social/xrpc/com.atproto.repo.getRecord?repo=%s&collection=app.bsky.feed.post&rkey=%s",
		postRef.DID, postRef.RKey)

	req, err := http.NewRequestWithContext(ctx, "GET", endpoint, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := t.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch post: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, fmt.Errorf("failed to fetch post with status %d: %v", resp.StatusCode, errorResp)
	}

	var recordResp struct {
		URI string `json:"uri"`
		CID string `json:"cid"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&recordResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &PostInfo{
		URI: recordResp.URI,
		CID: recordResp.CID,
	}, nil
}

// findLikeRecord finds the like record URI for a given post
func (t *ReactTool) findLikeRecord(ctx context.Context, creds *auth.Credentials, postRef *PostReference) (string, error) {
	// List records in the likes collection
	endpoint := fmt.Sprintf("https://bsky.social/xrpc/com.atproto.repo.listRecords?repo=%s&collection=app.bsky.feed.like&limit=100",
		creds.DID)

	req, err := http.NewRequestWithContext(ctx, "GET", endpoint, nil)
	if err != nil {
		return "", fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := t.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("failed to list likes: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return "", fmt.Errorf("failed to list likes with status %d: %v", resp.StatusCode, errorResp)
	}

	var listResp struct {
		Records []struct {
			URI   string                 `json:"uri"`
			Value map[string]interface{} `json:"value"`
		} `json:"records"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&listResp); err != nil {
		return "", fmt.Errorf("failed to parse response: %w", err)
	}

	// Find the like for this specific post
	targetURI := fmt.Sprintf("at://%s/app.bsky.feed.post/%s", postRef.DID, postRef.RKey)
	for _, record := range listResp.Records {
		if subject, ok := record.Value["subject"].(map[string]interface{}); ok {
			if uri, ok := subject["uri"].(string); ok && uri == targetURI {
				return record.URI, nil
			}
		}
	}

	return "", nil
}

// makeAuthenticatedRequest makes an authenticated HTTP request
func (t *ReactTool) makeAuthenticatedRequest(ctx context.Context, creds *auth.Credentials, method, endpoint string, body interface{}) error {
	var reqBody *bytes.Buffer
	if body != nil {
		jsonData, err := json.Marshal(body)
		if err != nil {
			return fmt.Errorf("failed to marshal request: %w", err)
		}
		reqBody = bytes.NewBuffer(jsonData)
	} else {
		reqBody = bytes.NewBuffer(nil)
	}

	req, err := http.NewRequestWithContext(ctx, method, endpoint, reqBody)
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := t.client.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return fmt.Errorf("request failed with status %d: %v", resp.StatusCode, errorResp)
	}

	return nil
}

// formatResults formats the operation results as markdown
func (t *ReactTool) formatResults(results *OperationResults) string {
	var sb strings.Builder

	sb.WriteString("# Reaction Operations\n\n")

	// Show successes
	if len(results.Successes) > 0 {
		sb.WriteString(fmt.Sprintf("## ✓ Successful Operations (%d)\n\n", len(results.Successes)))
		for _, success := range results.Successes {
			sb.WriteString(fmt.Sprintf("- %s\n", success))
		}
		sb.WriteString("\n")
	}

	// Show failures
	if len(results.Failures) > 0 {
		sb.WriteString(fmt.Sprintf("## ✗ Failed Operations (%d)\n\n", len(results.Failures)))
		for _, failure := range results.Failures {
			sb.WriteString(fmt.Sprintf("- **%s** `%s`: %s\n", failure.Operation, failure.URI, failure.Error))
		}
		sb.WriteString("\n")
	}

	// Summary
	total := len(results.Successes) + len(results.Failures)
	sb.WriteString(fmt.Sprintf("**Summary:** %d successful, %d failed out of %d total operations\n",
		len(results.Successes), len(results.Failures), total))

	return sb.String()
}

// parseURIList normalizes a tool argument into a list of post URIs.
// It accepts either:
// - string: comma/newline/semicolon-separated values
// - []interface{} or []string: list of strings
func parseURIList(v interface{}) []string {
	var out []string
	if v == nil {
		return out
	}
	switch vv := v.(type) {
	case string:
		// Split on comma, newline, or semicolon
		parts := strings.FieldsFunc(vv, func(r rune) bool {
			switch r {
			case ',', '\n', ';':
				return true
			default:
				return false
			}
		})
		for _, p := range parts {
			s := strings.TrimSpace(p)
			if s != "" {
				out = append(out, s)
			}
		}
	case []interface{}:
		for _, item := range vv {
			if s, ok := item.(string); ok {
				s = strings.TrimSpace(s)
				if s != "" {
					out = append(out, s)
				}
			}
		}
	case []string:
		for _, s := range vv {
			s2 := strings.TrimSpace(s)
			if s2 != "" {
				out = append(out, s2)
			}
		}
	}
	return out
}
