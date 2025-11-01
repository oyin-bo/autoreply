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

// FeedTool implements the feed tool
type FeedTool struct {
	apiClient *bluesky.APIClient
	credStore *auth.CredentialStore
}

// NewFeedTool creates a new feed tool
func NewFeedTool() (*FeedTool, error) {
	apiClient, err := bluesky.NewAPIClient()
	if err != nil {
		return nil, fmt.Errorf("failed to create API client: %w", err)
	}

	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &FeedTool{
		apiClient: apiClient,
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *FeedTool) Name() string {
	return "feed"
}

// Description returns the tool description
func (t *FeedTool) Description() string {
	return "Get the latest feed from BlueSky. Returns a list of posts (also known as tweets or skeets). If you want to see the latest posts from a specific user, provide their handle. These feeds are paginated - you get the top chunk and a cursor to get more posts."
}

// InputSchema returns the JSON schema for tool input
func (t *FeedTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"feed": {
				Type:        "string",
				Description: "(Optional) The feed to retrieve, can be a BlueSky feed URI, or a name for a feed to search for. If unspecified, returns the default popular feed 'What is Hot'.",
			},
			"login": {
				Type:        "string",
				Description: "(Optional) BlueSky handle for which the feed is requested. If unspecified or 'anonymous', the feed will be retrieved in incognito mode.",
			},
			"cursor": {
				Type:        "string",
				Description: "(Optional) Cursor for pagination.",
			},
			"limit": {
				Type:        "integer",
				Description: "(Optional) Limit the number of posts returned, defaults to 20, max 100.",
			},
		},
		Required: []string{},
	}
}

// Call executes the feed tool
func (t *FeedTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract parameters
	feed := getStringParam(args, "feed", "")
	login := getStringParam(args, "login", "")
	cursor := getStringParam(args, "cursor", "")
	limit := getIntParam(args, "limit", 20)

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

	// Determine which feed to fetch
	var feedData map[string]interface{}
	var err error

	params := make(map[string]string)
	if cursor != "" {
		params["cursor"] = cursor
	}
	if limit > 0 {
		if limit > 100 {
			limit = 100
		}
		params["limit"] = fmt.Sprintf("%d", limit)
	}

	if feed != "" {
		// Resolve feed URI if needed
		feedURI, err := t.resolveFeedURI(ctx, feed)
		if err != nil {
			return nil, errors.Wrap(err, errors.InvalidInput, "Failed to resolve feed")
		}

		// Use resolved feed URI
		params["feed"] = feedURI
		feedData, err = t.apiClient.GetWithOptionalAuth(ctx, login, "app.bsky.feed.getFeed", params)
	} else if login != "" && login != "anonymous" {
		// Get authenticated user's timeline
		feedData, err = t.apiClient.GetWithAuth(ctx, login, "app.bsky.feed.getTimeline", params)
	} else {
		// Use default "What's Hot" feed for anonymous users
		params["feed"] = "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot"
		feedData, err = t.apiClient.GetPublic(ctx, "app.bsky.feed.getFeed", params)
	}

	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to fetch feed")
	}

	// Format results as markdown
	markdown := t.formatFeedMarkdown(feedData)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// resolveFeedURI resolves a feed name/query to a full at:// URI
func (t *FeedTool) resolveFeedURI(ctx context.Context, feed string) (string, error) {
	// Check if it's already a valid at:// URI
	if strings.HasPrefix(feed, "at://") && strings.Contains(feed, "/app.bsky.feed.generator/") {
		return feed, nil
	}

	// Not a full URI - search for feed by name
	params := map[string]string{
		"query": feed,
	}

	searchResult, err := t.apiClient.GetPublic(ctx, "app.bsky.unspecced.getPopularFeedGenerators", params)
	if err != nil {
		return "", fmt.Errorf("failed to search for feed: %w", err)
	}

	// Extract feeds from search result
	feeds, ok := searchResult["feeds"].([]interface{})
	if !ok || len(feeds) == 0 {
		return "", fmt.Errorf("no feeds found matching '%s'. Please provide a valid feed URI (at://...) or search term", feed)
	}

	// Get the first feed's URI
	firstFeed, ok := feeds[0].(map[string]interface{})
	if !ok {
		return "", fmt.Errorf("invalid feed data in search result")
	}

	uri, ok := firstFeed["uri"].(string)
	if !ok || uri == "" {
		return "", fmt.Errorf("feed URI not found in search result")
	}

	return uri, nil
}

// formatFeedMarkdown formats feed data as markdown
func (t *FeedTool) formatFeedMarkdown(feedData map[string]interface{}) string {
	var sb strings.Builder

	// Header
	sb.WriteString("# BlueSky Feed\n\n")

	// Extract posts from feed
	feedArray, ok := feedData["feed"].([]interface{})
	if !ok || len(feedArray) == 0 {
		sb.WriteString("No posts found in feed.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("Found %d posts.\n\n", len(feedArray)))

	// Format each post
	for i, item := range feedArray {
		feedItem, ok := item.(map[string]interface{})
		if !ok {
			continue
		}

		post, ok := feedItem["post"].(map[string]interface{})
		if !ok {
			continue
		}

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

		if i < len(feedArray)-1 {
			sb.WriteString("---\n\n")
		}
	}

	// Add cursor information for pagination
	if cursor, ok := feedData["cursor"].(string); ok && cursor != "" {
		sb.WriteString(fmt.Sprintf("**Next cursor:** `%s`\n", cursor))
	}

	return sb.String()
}

// atURIToBskyURL converts an AT URI to a Bluesky web URL
func (t *FeedTool) atURIToBskyURL(atURI string) string {
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

// Helper functions for extracting values from maps

func getStringParam(args map[string]interface{}, key, defaultValue string) string {
	if val, ok := args[key]; ok {
		if str, ok := val.(string); ok {
			return strings.TrimSpace(str)
		}
	}
	return defaultValue
}

func getIntParam(args map[string]interface{}, key string, defaultValue int) int {
	if val, ok := args[key]; ok {
		switch v := val.(type) {
		case int:
			return v
		case int64:
			return int(v)
		case float64:
			return int(v)
		}
	}
	return defaultValue
}

func getStringFromMap(m map[string]interface{}, key, defaultValue string) string {
	if val, ok := m[key]; ok {
		if str, ok := val.(string); ok {
			return str
		}
	}
	return defaultValue
}

func getIntFromMap(m map[string]interface{}, key string, defaultValue int) int {
	if val, ok := m[key]; ok {
		switch v := val.(type) {
		case int:
			return v
		case int64:
			return int(v)
		case float64:
			return int(v)
		}
	}
	return defaultValue
}
