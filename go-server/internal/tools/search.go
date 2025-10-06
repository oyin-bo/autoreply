// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/cache"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
	"golang.org/x/text/unicode/norm"
)

// SearchTool implements the search tool
type SearchTool struct {
	didResolver  *bluesky.DIDResolver
	carProcessor *bluesky.CARProcessor
}

// NewSearchTool creates a new search tool
func NewSearchTool() *SearchTool {
	cacheManager, _ := cache.NewManager()
	return &SearchTool{
		didResolver:  bluesky.NewDIDResolver(),
		carProcessor: bluesky.NewCARProcessor(cacheManager),
	}
}

// Name returns the tool name
func (t *SearchTool) Name() string {
	return "search"
}

// Description returns the tool description
func (t *SearchTool) Description() string {
	return "Search posts within a user's repository"
}

// InputSchema returns the JSON schema for tool input
func (t *SearchTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"account": {
				Type:        "string",
				Description: "Handle (alice.bsky.social) or DID (did:plc:...)",
			},
			"query": {
				Type:        "string",
				Description: "Search terms (case-insensitive)",
			},
			"limit": {
				Type:        "integer",
				Description: "Maximum number of results (default 50, max 200)",
			},
		},
		Required: []string{"account", "query"},
	}
}

// Call executes the search tool
func (t *SearchTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate parameters
	account, query, limit, err := t.validateInput(args)
	if err != nil {
		return nil, err
	}

	// Resolve handle to DID
	did, err := t.didResolver.ResolveHandle(ctx, account)
	if err != nil {
		return nil, err
	}

	// Fetch repository if needed
	if err := t.carProcessor.FetchRepository(ctx, did); err != nil {
		return nil, err
	}

	// Search posts
	posts, err := t.carProcessor.SearchPosts(did, query)
	if err != nil {
		return nil, err
	}

	// Sort by CreatedAt descending (ISO8601 strings; parse for robustness)
	sort.Slice(posts, func(i, j int) bool {
		ti, ei := time.Parse(time.RFC3339, posts[i].CreatedAt)
		tj, ej := time.Parse(time.RFC3339, posts[j].CreatedAt)
		if ei == nil && ej == nil {
			return tj.Before(ti)
		}
		// Fallback to string compare
		return posts[i].CreatedAt > posts[j].CreatedAt
	})

	// Apply limit (default 50, max 200)
	if limit <= 0 {
		limit = 50
	}
	if limit > 200 {
		limit = 200
	}
	if len(posts) > limit {
		posts = posts[:limit]
	}

	// URIs are now constructed directly from MST during search
	// No HTTP requests needed!

	// Format results as markdown
	markdown := t.formatSearchResults(account, query, posts)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// validateInput validates the input parameters
func (t *SearchTool) validateInput(args map[string]interface{}) (account, query string, limit int, err error) {
	// Validate account
	accountRaw, ok := args["account"]
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "account parameter is required")
	}

	account, ok = accountRaw.(string)
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "account must be a string")
	}

	if strings.TrimSpace(account) == "" {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "account cannot be empty")
	}

	// Validate query
	queryRaw, ok := args["query"]
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query parameter is required")
	}

	query, ok = queryRaw.(string)
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query must be a string")
	}

	query = strings.TrimSpace(query)
	if query == "" {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot be empty")
	}

	if len(query) > 500 {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot exceed 500 characters")
	}

	// Normalize query for consistent search
	query = normalizeText(query)

	// Optional limit
	limit = 50
	if v, ok := args["limit"]; ok {
		switch vv := v.(type) {
		case float64:
			limit = int(vv)
		case int:
			limit = vv
		case int32:
			limit = int(vv)
		case int64:
			limit = int(vv)
		case string:
			// ignore strings silently
		}
	}
	if limit < 1 {
		limit = 1
	}
	if limit > 200 {
		limit = 200
	}

	return account, query, limit, nil
}

// normalizeText normalizes text for consistent searching
func normalizeText(text string) string {
	// Apply Unicode NFKC normalization
	normalized := norm.NFKC.String(text)

	return strings.ToLower(normalized)
}

// highlightMatches highlights search matches in text with bold markdown
func (t *SearchTool) highlightMatches(text, query string) string {
	if query == "" {
		return text
	}

	normalizedText := normalizeText(text)
	normalizedQuery := normalizeText(query)

	// Simple substring highlighting - in a production implementation,
	// you would want more sophisticated matching
	if strings.Contains(normalizedText, normalizedQuery) {
		// Find all matches and wrap them with **bold**
		return strings.ReplaceAll(text, query, fmt.Sprintf("**%s**", query))
	}

	return text
}

// atURIToBskyURL converts an AT URI to a Bluesky web URL
// at://did:plc:abc/app.bsky.feed.post/xyz -> https://bsky.app/profile/handle/post/xyz
func (t *SearchTool) atURIToBskyURL(atURI, handle string) string {
	// Parse AT URI: at://{did}/{collection}/{rkey}
	if !strings.HasPrefix(atURI, "at://") {
		return atURI
	}

	parts := strings.Split(strings.TrimPrefix(atURI, "at://"), "/")
	if len(parts) < 3 {
		return atURI
	}

	// parts[0] = DID
	// parts[1] = collection (e.g., app.bsky.feed.post)
	// parts[2] = rkey

	// Use handle if available, otherwise use DID
	profile := handle
	if profile == "" {
		profile = parts[0] // Use DID as fallback
	} else {
		profile = strings.TrimPrefix(profile, "@") // Remove @ if present
	}

	rkey := parts[2]

	return fmt.Sprintf("https://bsky.app/profile/%s/post/%s", profile, rkey)
}

// formatSearchResults formats search results as markdown
func (t *SearchTool) formatSearchResults(handle, query string, posts []*bluesky.ParsedPost) string {
	var sb strings.Builder

	// Header
	sb.WriteString(fmt.Sprintf("# Search Results for \"%s\" in @%s\n\n",
		query, strings.TrimPrefix(handle, "@")))

	if len(posts) == 0 {
		sb.WriteString("No matching posts found.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("Found %d matching posts.\n\n", len(posts)))

	// Format each post in the new blockquote format
	for i, post := range posts {
		// Post identifier (handle/rkey)
		rkey := ""
		if post.URI != "" {
			parts := strings.Split(post.URI, "/")
			if len(parts) > 0 {
				rkey = parts[len(parts)-1]
			}
		}
		cleanHandle := strings.TrimPrefix(handle, "@")
		sb.WriteString(fmt.Sprintf("@%s/%s\n", cleanHandle, rkey))

		// Blockquoted user content
		if post.Text != "" {
			highlightedText := t.highlightMatches(post.Text, query)
			lines := strings.Split(highlightedText, "\n")
			for _, line := range lines {
				sb.WriteString(fmt.Sprintf("> %s\n", line))
			}
		}

		// Images in blockquote
		if len(post.Embeds) > 0 {
			for _, embed := range post.Embeds {
				if len(embed.Images) > 0 {
					for j, img := range embed.Images {
						altText := img.Alt
						if altText == "" {
							altText = fmt.Sprintf("Image %d", j+1)
						}
						sb.WriteString(fmt.Sprintf("> ![%s](image)\n", altText))
					}
				}
			}
		}

		// Stats and metadata (outside blockquote) - timestamp
		if post.CreatedAt != "" {
			sb.WriteString(fmt.Sprintf("%s\n", post.CreatedAt))
		}

		if i < len(posts)-1 {
			sb.WriteString("\n")
		}
	}

	sb.WriteString(fmt.Sprintf("\n---\n\n*Results: Showing %d of %d results.*\n", len(posts), len(posts)))

	return sb.String()
}
