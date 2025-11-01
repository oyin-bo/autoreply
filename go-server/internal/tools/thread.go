// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"strings"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// ThreadTool implements the thread tool
type ThreadTool struct {
	apiClient *bluesky.APIClient
	credStore *auth.CredentialStore
}

// NewThreadTool creates a new thread tool
func NewThreadTool() (*ThreadTool, error) {
	apiClient, err := bluesky.NewAPIClient()
	if err != nil {
		return nil, fmt.Errorf("failed to create API client: %w", err)
	}

	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &ThreadTool{
		apiClient: apiClient,
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *ThreadTool) Name() string {
	return "thread"
}

// Description returns the tool description
func (t *ThreadTool) Description() string {
	return "Fetch a thread by post URI. Returns all the replies and replies to replies - the whole conversation. If you're already logged in, this will fetch the thread as viewed by the logged in user. If the handle is 'anonymous', it will fetch the thread in incognito mode."
}

// InputSchema returns the JSON schema for tool input
func (t *ThreadTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"postURI": {
				Type:        "string",
				Description: "The BlueSky URL of the post, or the at:// URI of the post to fetch the thread for.",
			},
			"login": {
				Type:        "string",
				Description: "(Optional) BlueSky handle to use for authenticated fetch. Use 'anonymous' for incognito mode.",
			},
		},
		Required: []string{"postURI"},
	}
}

// Call executes the thread tool
func (t *ThreadTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate parameters
	postURIRaw, ok := args["postURI"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "postURI parameter is required")
	}

	postURI, ok := postURIRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "postURI must be a string")
	}

	postURI = strings.TrimSpace(postURI)
	if postURI == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "postURI cannot be empty")
	}

	// Convert BlueSky web URL to AT URI if needed
	postURI = t.normalizePostURI(postURI)

	// Extract login parameter
	login := getStringParam(args, "login", "")

	// Validate and normalize login
	if login != "" && login != "anonymous" {
		// Check if credentials exist
		_, err := t.credStore.Load(login)
		if err != nil {
			// Try to get default handle
			defaultHandle, defErr := t.credStore.GetDefault()
			if defErr == nil && defaultHandle != "" {
				login = defaultHandle
			} else {
				login = "anonymous" // Fall back to anonymous
			}
		}
	}

	// If login is empty, try to get default handle
	if login == "" {
		defaultHandle, err := t.credStore.GetDefault()
		if err == nil && defaultHandle != "" {
			login = defaultHandle
		}
	}

	// Fetch thread
	params := map[string]string{
		"uri": postURI,
	}

	threadData, err := t.apiClient.GetWithOptionalAuth(ctx, login, "app.bsky.feed.getPostThread", params)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to fetch thread")
	}

	// Format results as markdown
	markdown := t.formatThreadMarkdown(threadData)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// normalizePostURI converts BlueSky web URLs to AT URIs
func (t *ThreadTool) normalizePostURI(uri string) string {
	// If already an AT URI, return as is
	if strings.HasPrefix(uri, "at://") {
		return uri
	}

	// Convert BlueSky web URL to AT URI
	// https://bsky.app/profile/{handle}/post/{rkey} -> at://{did}/app.bsky.feed.post/{rkey}
	// Note: This is a simplified conversion. In production, you'd need to resolve handle to DID.
	// For now, we'll return the original URI and let the API handle it.
	return uri
}

// formatThreadMarkdown formats thread data as markdown
func (t *ThreadTool) formatThreadMarkdown(threadData map[string]interface{}) string {
	var sb strings.Builder

	// Header
	sb.WriteString("# BlueSky Thread\n\n")

	// Extract thread from response
	thread, ok := threadData["thread"].(map[string]interface{})
	if !ok {
		sb.WriteString("No thread data found.\n")
		return sb.String()
	}

	// Flatten thread into a list of posts
	posts := t.flattenThread(thread)

	if len(posts) == 0 {
		sb.WriteString("No posts found in thread.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("Found %d posts in thread.\n\n", len(posts)))

	// Format each post
	for i, post := range posts {
		sb.WriteString(fmt.Sprintf("## Post %d\n", i+1))

		// Post URI (link to post)
		if uri, ok := post["uri"].(string); ok {
			webURL := t.atURIToBskyURL(uri)
			sb.WriteString(fmt.Sprintf("**Link:** %s\n", webURL))
		}

		// Created at
		if record, ok := post["record"].(map[string]interface{}); ok {
			if createdAt, ok := record["createdAt"].(string); ok {
				sb.WriteString(fmt.Sprintf("**Created:** %s\n", createdAt))
			}
		}

		sb.WriteString("\n")

		// Post content
		if record, ok := post["record"].(map[string]interface{}); ok {
			if text, ok := record["text"].(string); ok && text != "" {
				sb.WriteString(fmt.Sprintf("%s\n\n", text))
			}
		}

		if i < len(posts)-1 {
			sb.WriteString("---\n\n")
		}
	}

	return sb.String()
}

// flattenThread recursively flattens a thread tree into a list of posts
func (t *ThreadTool) flattenThread(node map[string]interface{}) []map[string]interface{} {
	var posts []map[string]interface{}

	// Add the current post if it exists
	if post, ok := node["post"].(map[string]interface{}); ok {
		posts = append(posts, post)
	}

	// Recursively add replies
	if replies, ok := node["replies"].([]interface{}); ok {
		for _, reply := range replies {
			if replyMap, ok := reply.(map[string]interface{}); ok {
				posts = append(posts, t.flattenThread(replyMap)...)
			}
		}
	}

	return posts
}

// atURIToBskyURL converts an AT URI to a Bluesky web URL
func (t *ThreadTool) atURIToBskyURL(atURI string) string {
	// Parse AT URI: at://{did}/{collection}/{rkey}
	if !strings.HasPrefix(atURI, "at://") {
		return atURI
	}

	parts := strings.Split(strings.TrimPrefix(atURI, "at://"), "/")
	if len(parts) < 3 {
		return atURI
	}

	did := parts[0]
	rkey := parts[2]

	return fmt.Sprintf("https://bsky.app/profile/%s/post/%s", did, rkey)
}
