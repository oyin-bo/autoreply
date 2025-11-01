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
	return "Get the latest feed from BlueSky. Returns a list of posts (also known as tweets or skeets). If you want to see the latest posts from a specific user, provide their handle."
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
			"viewAs": {
				Type:        "string",
				Description: "(Optional) Account to view feed as: handle (alice.bsky.social), @handle, DID, Bsky.app profile URL, or partial DID suffix. If unspecified or 'anonymous', the feed will be retrieved in incognito mode.",
			},
			"continueAtCursor": {
				Type:        "string",
				Description: "(Optional) Cursor for pagination. When provided, fetches the next batch of posts from where the previous request left off.",
			},
			"limit": {
				Type:        "integer",
				Description: "(Optional) Defaults to 50",
			},
		},
		Required: []string{},
	}
}

// Call executes the feed tool
func (t *FeedTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract parameters
	feed := getStringParam(args, "feed", "")
	viewAs := getStringParam(args, "viewAs", "")
	continueAtCursor := getStringParam(args, "continueAtCursor", "")
	limit := getIntParam(args, "limit", 50)

	// Validate and normalize viewAs
	if viewAs != "" && viewAs != "anonymous" {
		// Check if credentials exist
		_, err := t.credStore.Load(viewAs)
		if err != nil {
			// Try to get default handle
			defaultHandle, defErr := t.credStore.GetDefault()
			if defErr == nil && defaultHandle != "" {
				viewAs = defaultHandle
			} else {
				viewAs = "anonymous" // Fall back to anonymous
			}
		}
	}

	// If viewAs is empty, try to get default handle
	if viewAs == "" {
		defaultHandle, err := t.credStore.GetDefault()
		if err == nil && defaultHandle != "" {
			viewAs = defaultHandle
		}
	}

	// Fetch the feed - if limit > 100, fetch in batches
	var allPosts []interface{}
	var cursor string
	if continueAtCursor != "" {
		cursor = continueAtCursor
	}

	requestedLimit := limit
	if requestedLimit <= 0 {
		requestedLimit = 50
	}

	// Fetch in batches if needed
	for len(allPosts) < requestedLimit {
		batchSize := requestedLimit - len(allPosts)
		if batchSize > 100 {
			batchSize = 100 // API limit per request
		}

		params := make(map[string]string)
		if cursor != "" {
			params["cursor"] = cursor
		}
		params["limit"] = fmt.Sprintf("%d", batchSize)

		var feedData map[string]interface{}
		var err error

		if feed != "" {
			// Resolve feed URI if needed
			var feedURI string
			feedURI, err = t.resolveFeedURI(ctx, feed)
			if err != nil {
				return nil, errors.Wrap(err, errors.InvalidInput, "Failed to resolve feed")
			}

			// Use resolved feed URI
			params["feed"] = feedURI
			feedData, err = t.apiClient.GetWithOptionalAuth(ctx, viewAs, "app.bsky.feed.getFeed", params)
		} else if viewAs != "" && viewAs != "anonymous" {
			// Get authenticated user's timeline
			feedData, err = t.apiClient.GetWithAuth(ctx, viewAs, "app.bsky.feed.getTimeline", params)
		} else {
			// Use default "What's Hot" feed for anonymous users
			params["feed"] = "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot"
			feedData, err = t.apiClient.GetPublic(ctx, "app.bsky.feed.getFeed", params)
		}

		if err != nil {
			return nil, errors.Wrap(err, errors.InternalError, "Failed to fetch feed")
		}

		// Extract posts from this batch
		batchPosts, ok := feedData["feed"].([]interface{})
		if !ok || len(batchPosts) == 0 {
			// No more posts available
			break
		}

		allPosts = append(allPosts, batchPosts...)

		// Check if there's a cursor for next batch
		nextCursor, hasCursor := feedData["cursor"].(string)
		if !hasCursor || nextCursor == "" {
			// No more pages available
			break
		}
		cursor = nextCursor

		// If we got fewer posts than requested in this batch, we've reached the end
		if len(batchPosts) < batchSize {
			break
		}
	}

	// Rebuild feedData with all collected posts
	feedData := map[string]interface{}{
		"feed": allPosts,
	}
	if cursor != "" {
		feedData["cursor"] = cursor
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

// formatFeedMarkdown formats feed data as markdown per docs/16-mcp-schemas.md spec
func (t *FeedTool) formatFeedMarkdown(feedData map[string]interface{}) string {
	var sb strings.Builder

	// Extract posts from feed
	feedArray, ok := feedData["feed"].([]interface{})
	if !ok || len(feedArray) == 0 {
		sb.WriteString("# Feed · 0 posts\n\nNo posts found in feed.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("# Feed · %d posts\n\n", len(feedArray)))

	// Track seen posts for ID compaction
	seenPosts := make(map[string]bool)

	// Format each post
	for _, item := range feedArray {
		feedItem, ok := item.(map[string]interface{})
		if !ok {
			continue
		}

		post, ok := feedItem["post"].(map[string]interface{})
		if !ok {
			continue
		}

		// Extract post data
		uri, _ := post["uri"].(string)
		rkey := ExtractRkey(uri)

		author, _ := post["author"].(map[string]interface{})
		handle, _ := author["handle"].(string)

		record, _ := post["record"].(map[string]interface{})
		text, _ := record["text"].(string)
		createdAt, _ := record["createdAt"].(string)

		likes := GetIntField(post, "likeCount")
		replies := GetIntField(post, "replyCount")
		reposts := GetIntField(post, "repostCount")
		quotes := GetIntField(post, "quoteCount")

		// Author ID line
		fullID := fmt.Sprintf("%s/%s", handle, rkey)
		authorID := CompactPostID(handle, rkey, seenPosts)
		sb.WriteString(fmt.Sprintf("%s\n", authorID))
		seenPosts[fullID] = true

		// Blockquote content
		sb.WriteString(BlockquoteContent(text))
		sb.WriteString("\n")

		// Stats and timestamp
		stats := FormatStats(likes, reposts, quotes, replies)
		timestamp := FormatTimestamp(createdAt)

		if stats != "" {
			sb.WriteString(fmt.Sprintf("%s  %s\n", stats, timestamp))
		} else {
			sb.WriteString(fmt.Sprintf("%s\n", timestamp))
		}

		sb.WriteString("\n")
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
