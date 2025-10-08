// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"sort"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
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
			"from": {
				Type:        "string",
				Description: "Handle (alice.bsky.social) or DID (did:plc:...) of the account to search posts from - optional when login is provided",
			},
			"query": {
				Type:        "string",
				Description: "Search terms (case-insensitive)",
			},
			"limit": {
				Type:        "integer",
				Description: "Maximum number of results (default 50, max 200)",
			},
			"login": {
				Type:        "string",
				Description: "Login handle for authenticated search (must be previously authenticated)",
			},
		},
		Required: []string{"query"},
	}
}

// Call executes the search tool
func (t *SearchTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate parameters
	from, query, login, limit, err := t.validateInput(args)
	if err != nil {
		return nil, err
	}

	// Perform search based on parameters
	var carPosts, apiPosts []*bluesky.ParsedPost
	var displayHandle string

	if login != "" {
		// Login-based search
		normalizedLogin := normalizeHandle(login)
		displayHandle = normalizedLogin

		// Perform API search
		apiResults, err := t.searchViaAPI(ctx, normalizedLogin, query, limit)
		if err != nil {
			return nil, err
		}
		apiPosts = apiResults

		// If from is also provided, perform CAR search
		if from != "" {
			carResults, err := t.performCARSearch(ctx, from, query, limit)
			if err != nil {
				return nil, err
			}
			carPosts = carResults
		}
	} else if from != "" {
		// Traditional CAR-only search
		carResults, err := t.performCARSearch(ctx, from, query, limit)
		if err != nil {
			return nil, err
		}
		carPosts = carResults
		displayHandle = from
	}

	// Merge and deduplicate results
	mergedPosts := mergeAndDeduplicateResults(carPosts, apiPosts)

	if len(mergedPosts) == 0 {
		return nil, errors.NewMCPError(errors.NotFound, fmt.Sprintf("No posts found matching query '%s'", query))
	}

	// Format results as markdown
	markdown := t.formatSearchResults(displayHandle, query, mergedPosts)

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
func (t *SearchTool) validateInput(args map[string]interface{}) (from, query, login string, limit int, err error) {
	// Extract from (optional if login is provided)
	if fromRaw, ok := args["from"]; ok {
		from, ok = fromRaw.(string)
		if !ok {
			return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "from must be a string")
		}
		from = strings.TrimSpace(from)
	}

	// Extract login (optional)
	if loginRaw, ok := args["login"]; ok {
		login, ok = loginRaw.(string)
		if !ok {
			return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "login must be a string")
		}
		login = strings.TrimSpace(login)
	}

	// Validate that either from or login is provided
	if from == "" && login == "" {
		return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "Either 'from' or 'login' parameter must be provided")
	}

	// Validate query
	queryRaw, ok := args["query"]
	if !ok {
		return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "query parameter is required")
	}

	query, ok = queryRaw.(string)
	if !ok {
		return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "query must be a string")
	}

	query = strings.TrimSpace(query)
	if query == "" {
		return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot be empty")
	}

	if len(query) > 500 {
		return "", "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot exceed 500 characters")
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

	return from, query, login, limit, nil
}

// normalizeText normalizes text for consistent searching
func normalizeText(text string) string {
	// Apply Unicode NFKC normalization
	normalized := norm.NFKC.String(text)

	return strings.ToLower(normalized)
}

// normalizeHandle removes @ prefix, trims whitespace, and converts to lowercase
func normalizeHandle(handle string) string {
	return strings.ToLower(strings.TrimPrefix(strings.TrimSpace(handle), "@"))
}

// performCARSearch performs CAR-based search on a user's repository
func (t *SearchTool) performCARSearch(ctx context.Context, from, query string, limit int) ([]*bluesky.ParsedPost, error) {
	// Resolve handle to DID
	did, err := t.didResolver.ResolveHandle(ctx, from)
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

	// Sort by CreatedAt descending
	sort.Slice(posts, func(i, j int) bool {
		ti, ei := time.Parse(time.RFC3339, posts[i].CreatedAt)
		tj, ej := time.Parse(time.RFC3339, posts[j].CreatedAt)
		if ei == nil && ej == nil {
			return tj.Before(ti)
		}
		return posts[i].CreatedAt > posts[j].CreatedAt
	})

	// Apply limit
	if limit <= 0 {
		limit = 50
	}
	if limit > 200 {
		limit = 200
	}
	if len(posts) > limit {
		posts = posts[:limit]
	}

	return posts, nil
}

// searchViaAPI performs authenticated API search using BlueSky searchPosts endpoint
func (t *SearchTool) searchViaAPI(ctx context.Context, login, query string, limit int) ([]*bluesky.ParsedPost, error) {
	// Load credentials from storage
	store, err := auth.NewCredentialStore()
	if err != nil {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Failed to initialize credential store: %v", err))
	}

	creds, err := store.Load(login)
	if err != nil {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Login '%s' not found. Please login first using the login tool.", login))
	}

	// Make API request to searchPosts endpoint
	client := &http.Client{Timeout: 30 * time.Second}
	url := fmt.Sprintf("https://bsky.social/xrpc/app.bsky.feed.searchPosts?q=%s&limit=%d", 
		query, limit)

	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Failed to create request: %v", err))
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", creds.AccessToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := client.Do(req)
	if err != nil {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Authenticated search failed: %v", err))
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Authenticated search failed: %s", resp.Status))
	}

	// Parse response
	type SearchPostsResponse struct {
		Posts []struct {
			URI    string `json:"uri"`
			CID    string `json:"cid"`
			Record struct {
				Text      string `json:"text"`
				CreatedAt string `json:"createdAt"`
			} `json:"record"`
		} `json:"posts"`
	}

	var searchResult SearchPostsResponse
	if err := json.NewDecoder(resp.Body).Decode(&searchResult); err != nil {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("Failed to parse search response: %v", err))
	}

	// Convert to ParsedPost format
	var posts []*bluesky.ParsedPost
	for _, apiPost := range searchResult.Posts {
		posts = append(posts, &bluesky.ParsedPost{
			PostRecord: &bluesky.PostRecord{
				URI:       apiPost.URI,
				CID:       apiPost.CID,
				Text:      apiPost.Record.Text,
				CreatedAt: apiPost.Record.CreatedAt,
			},
		})
	}

	return posts, nil
}

// mergeAndDeduplicateResults merges and deduplicates search results from CAR and API sources
func mergeAndDeduplicateResults(carPosts, apiPosts []*bluesky.ParsedPost) []*bluesky.ParsedPost {
	postsByURI := make(map[string]*bluesky.ParsedPost)

	// Add CAR posts first
	for _, post := range carPosts {
		postsByURI[post.URI] = post
	}

	// Add or merge API posts (API posts have priority)
	for _, post := range apiPosts {
		postsByURI[post.URI] = post
	}

	// Convert back to slice and sort by created_at descending
	var merged []*bluesky.ParsedPost
	for _, post := range postsByURI {
		merged = append(merged, post)
	}

	sort.Slice(merged, func(i, j int) bool {
		return merged[i].CreatedAt > merged[j].CreatedAt
	})

	return merged
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
