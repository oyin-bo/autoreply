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

// PostTool implements the post tool for creating posts and replies
type PostTool struct {
	credStore *auth.CredentialStore
	client    *http.Client
}

// NewPostTool creates a new post tool
func NewPostTool() (*PostTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &PostTool{
		credStore: credStore,
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
	}, nil
}

// Name returns the tool name
func (t *PostTool) Name() string {
	return "post"
}

// Description returns the tool description
func (t *PostTool) Description() string {
	return "Create new posts with text content and optional reply functionality. Supports posting as authenticated users."
}

// InputSchema returns the JSON schema for tool input
func (t *PostTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"postAs": {
				Type:        "string",
				Description: "Handle or DID to post as (uses default account if not specified)",
			},
			"text": {
				Type:        "string",
				Description: "Text content of the post (required)",
			},
			"replyTo": {
				Type:        "string",
				Description: "Post URI (at://...) or Bluesky URL (https://bsky.app/...) to reply to (optional)",
			},
		},
		Required: []string{"text"},
	}
}

// Call executes the post tool
func (t *PostTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract text parameter (required)
	textRaw, ok := args["text"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "text parameter is required")
	}

	text, ok := textRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "text must be a string")
	}

	text = strings.TrimSpace(text)
	if text == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "text cannot be empty")
	}

	// Extract postAs parameter (optional, uses default if not provided)
	var postAs string
	if postAsRaw, ok := args["postAs"]; ok {
		if postAsStr, ok := postAsRaw.(string); ok {
			postAs = strings.TrimSpace(strings.TrimPrefix(postAsStr, "@"))
		}
	}

	// Get credentials for the user
	creds, err := t.getCredentials(postAs)
	if err != nil {
		return nil, err
	}

	// Extract replyTo parameter (optional)
	var replyTo string
	if replyToRaw, ok := args["replyTo"]; ok {
		if replyToStr, ok := replyToRaw.(string); ok {
			replyTo = strings.TrimSpace(replyToStr)
		}
	}

	// Create the post
	result, err := t.createPost(ctx, creds, text, replyTo)
	if err != nil {
		return nil, err
	}

	return result, nil
}

// getCredentials retrieves credentials for a user (by handle or DID)
func (t *PostTool) getCredentials(postAs string) (*auth.Credentials, error) {
	// If no postAs specified, use default
	if postAs == "" {
		defaultHandle, err := t.credStore.GetDefault()
		if err != nil {
			return nil, errors.NewMCPError(errors.InvalidInput, "No default account set. Please login first or specify postAs parameter.")
		}
		postAs = defaultHandle
	}

	// Load credentials
	creds, err := t.credStore.Load(postAs)
	if err != nil {
		return nil, errors.Wrap(err, errors.NotFound, fmt.Sprintf("No credentials found for %s. Please login first.", postAs))
	}

	return creds, nil
}

// createPost creates a new post, optionally as a reply
func (t *PostTool) createPost(ctx context.Context, creds *auth.Credentials, text, replyTo string) (*mcp.ToolResult, error) {
	// Build the post record
	record := map[string]interface{}{
		"$type":     "app.bsky.feed.post",
		"text":      text,
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	var replyInfo *ReplyInfo

	// If this is a reply, fetch the parent post
	if replyTo != "" {
		var err error
		replyInfo, err = t.getReplyInfo(ctx, creds, replyTo)
		if err != nil {
			return nil, errors.Wrap(err, errors.InvalidInput, "Failed to process replyTo parameter")
		}

		record["reply"] = map[string]interface{}{
			"root":   replyInfo.Root,
			"parent": replyInfo.Parent,
		}
	}

	// Create the record via AT Protocol API
	endpoint := "https://bsky.social/xrpc/com.atproto.repo.createRecord"

	requestBody := map[string]interface{}{
		"repo":       creds.DID,
		"collection": "app.bsky.feed.post",
		"record":     record,
	}

	jsonData, err := json.Marshal(requestBody)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to marshal request")
	}

	req, err := http.NewRequestWithContext(ctx, "POST", endpoint, bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to create request")
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := t.client.Do(req)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to create post")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Post creation failed with status %d: %v", resp.StatusCode, errorResp))
	}

	var createResp struct {
		URI string `json:"uri"`
		CID string `json:"cid"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&createResp); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to parse response")
	}

	// Build response message
	var message strings.Builder
	if replyInfo != nil {
		message.WriteString(fmt.Sprintf("# Reply Posted\n\n**Reply to:** %s\n\n**Your reply:**\n%s\n\n**Post URI:** `%s`\n",
			replyInfo.ParentText, text, createResp.URI))
	} else {
		message.WriteString(fmt.Sprintf("# Post Created\n\n**Text:**\n%s\n\n**Post URI:** `%s`\n",
			text, createResp.URI))
	}

	// Convert URI to Bluesky web URL
	webURL := t.atURIToBskyURL(createResp.URI, creds.Handle)
	message.WriteString(fmt.Sprintf("\n**View on Bluesky:** %s\n", webURL))

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message.String(),
			},
		},
	}, nil
}

// ReplyInfo contains information about a reply's parent and root posts
type ReplyInfo struct {
	Root       map[string]interface{}
	Parent     map[string]interface{}
	ParentText string
}

// getReplyInfo fetches information about the post being replied to
func (t *PostTool) getReplyInfo(ctx context.Context, creds *auth.Credentials, replyTo string) (*ReplyInfo, error) {
	// Parse the URI/URL
	postRef, err := parsePostReference(replyTo)
	if err != nil {
		return nil, err
	}

	// Fetch the post record
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
		URI   string                 `json:"uri"`
		CID   string                 `json:"cid"`
		Value map[string]interface{} `json:"value"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&recordResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	// Build reply structure
	replyInfo := &ReplyInfo{
		Parent: map[string]interface{}{
			"uri": recordResp.URI,
			"cid": recordResp.CID,
		},
	}

	// Extract parent text for display
	if text, ok := recordResp.Value["text"].(string); ok {
		replyInfo.ParentText = text
	}

	// Check if parent is already a reply - if so, use its root
	if parentReply, ok := recordResp.Value["reply"].(map[string]interface{}); ok {
		if root, ok := parentReply["root"].(map[string]interface{}); ok {
			replyInfo.Root = root
		} else {
			// Parent is a reply but has no root - use parent as root
			replyInfo.Root = replyInfo.Parent
		}
	} else {
		// Parent is not a reply - it becomes the root
		replyInfo.Root = replyInfo.Parent
	}

	return replyInfo, nil
}

// atURIToBskyURL converts an AT URI to a Bluesky web URL
func (t *PostTool) atURIToBskyURL(atURI, handle string) string {
	// Parse AT URI: at://{did}/{collection}/{rkey}
	if !strings.HasPrefix(atURI, "at://") {
		return atURI
	}

	parts := strings.Split(strings.TrimPrefix(atURI, "at://"), "/")
	if len(parts) < 3 {
		return atURI
	}

	rkey := parts[2]
	profile := strings.TrimPrefix(handle, "@")
	if profile == "" {
		profile = parts[0] // Use DID as fallback
	}

	return fmt.Sprintf("https://bsky.app/profile/%s/post/%s", profile, rkey)
}
