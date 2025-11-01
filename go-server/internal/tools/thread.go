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
	return "Fetch a thread by post reference (at:// URI, https://bsky.app/... URL, or @handle/rkey). Returns all the replies and replies to replies - the whole conversation."
}

// InputSchema returns the JSON schema for tool input
func (t *ThreadTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"postURI": {
				Type:        "string",
				Description: "Post reference: at:// URI, https://bsky.app/... URL, or @handle/rkey format",
			},
			"viewAs": {
				Type:        "string",
				Description: "(Optional) Account to view thread as: handle, @handle, DID, Bsky.app profile URL, or partial DID suffix. Use 'anonymous' for incognito mode (default if not specified).",
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
	atURI, err := t.normalizePostURI(ctx, postURI)
	if err != nil {
		return nil, errors.Wrap(err, errors.InvalidInput, "Failed to parse post URI")
	}

	// Extract viewAs parameter
	viewAs := getStringParam(args, "viewAs", "")

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

	// Fetch thread data
	params := map[string]string{
		"uri": atURI,
	}

	threadData, err := t.apiClient.GetWithOptionalAuth(ctx, viewAs, "app.bsky.feed.getPostThread", params)
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
func (t *ThreadTool) normalizePostURI(ctx context.Context, uri string) (string, error) {
	uri = strings.TrimSpace(uri)

	// If already an AT URI, return as is
	if strings.HasPrefix(uri, "at://") {
		return uri, nil
	}

	// Try compact format @handle/rkey
	if strings.HasPrefix(uri, "@") && strings.Contains(uri, "/") {
		parts := strings.SplitN(uri[1:], "/", 2) // Remove @ and split on first /
		if len(parts) == 2 {
			handle := parts[0]
			rkey := parts[1]

			// Resolve handle to DID
			did, err := t.resolveHandle(ctx, handle)
			if err != nil {
				return "", fmt.Errorf("failed to resolve handle '%s': %w", handle, err)
			}

			// Construct AT URI
			atURI := fmt.Sprintf("at://%s/app.bsky.feed.post/%s", did, rkey)
			return atURI, nil
		}
	}

	// Try to parse BlueSky web URL
	// https://bsky.app/profile/{handle}/post/{rkey}
	if strings.HasPrefix(uri, "https://bsky.app/profile/") {
		parts := strings.TrimPrefix(uri, "https://bsky.app/profile/")
		segments := strings.Split(parts, "/post/")
		if len(segments) == 2 {
			handle := segments[0]
			postID := strings.Split(segments[1], "/")[0] // Remove trailing slashes
			postID = strings.Split(postID, "?")[0]       // Remove query params

			// Check if handle is already a DID
			var did string
			if strings.HasPrefix(handle, "did:") {
				did = handle
			} else {
				// Resolve handle to DID
				resolvedDID, err := t.resolveHandle(ctx, handle)
				if err != nil {
					return "", fmt.Errorf("failed to resolve handle '%s': %w", handle, err)
				}
				did = resolvedDID
			}

			// Construct AT URI
			atURI := fmt.Sprintf("at://%s/app.bsky.feed.post/%s", did, postID)
			return atURI, nil
		}
	}

	return "", fmt.Errorf("invalid post URI: %s. Expected at:// URI, https://bsky.app/profile/handle/post/id URL, or @handle/rkey", uri)
}

// resolveHandle resolves a BlueSky handle to a DID
func (t *ThreadTool) resolveHandle(ctx context.Context, handle string) (string, error) {
	handle = strings.TrimPrefix(handle, "@")

	params := map[string]string{
		"handle": handle,
	}

	result, err := t.apiClient.GetPublic(ctx, "com.atproto.identity.resolveHandle", params)
	if err != nil {
		return "", fmt.Errorf("failed to resolve handle: %w", err)
	}

	did, ok := result["did"].(string)
	if !ok || did == "" {
		return "", fmt.Errorf("did not found in resolve response")
	}

	return did, nil
}

// formatThreadMarkdown formats thread data as markdown per docs/16-mcp-schemas.md spec
func (t *ThreadTool) formatThreadMarkdown(threadData map[string]interface{}) string {
	var sb strings.Builder

	// Extract thread from response
	thread, ok := threadData["thread"].(map[string]interface{})
	if !ok {
		sb.WriteString("# Thread · 0 posts\n\nNo thread data found.\n")
		return sb.String()
	}

	// Count total posts
	totalPosts := t.countPosts(thread)
	sb.WriteString(fmt.Sprintf("# Thread · %d posts\n\n", totalPosts))

	// Track seen posts for ID compaction
	seenPosts := make(map[string]bool)

	// Format thread recursively
	t.formatThreadRecursive(&sb, thread, seenPosts, 0, nil)

	return sb.String()
}

// countPosts counts total posts in thread
func (t *ThreadTool) countPosts(node map[string]interface{}) int {
	count := 0

	// Count current post
	if _, ok := node["post"].(map[string]interface{}); ok {
		count = 1
	}

	// Count replies recursively
	if replies, ok := node["replies"].([]interface{}); ok {
		for _, reply := range replies {
			if replyMap, ok := reply.(map[string]interface{}); ok {
				count += t.countPosts(replyMap)
			}
		}
	}

	return count
}

// formatThreadRecursive recursively formats thread with proper indentation and threading indicators
func (t *ThreadTool) formatThreadRecursive(sb *strings.Builder, node map[string]interface{}, seenPosts map[string]bool, depth int, parentPost map[string]interface{}) {
	post, ok := node["post"].(map[string]interface{})
	if !ok {
		return
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

	// Build the first line with threading indicator (INDENTED)
	fullID := fmt.Sprintf("%s/%s", handle, rkey)
	authorID := CompactPostID(handle, rkey, seenPosts)

	if depth == 0 {
		// Root post - just the author ID, no indent
		sb.WriteString(fmt.Sprintf("%s\n", authorID))
	} else if parentPost != nil {
		// Reply - show threading indicator with indentation
		parentURI, _ := parentPost["uri"].(string)
		parentRkey := ExtractRkey(parentURI)
		parentAuthor, _ := parentPost["author"].(map[string]interface{})
		parentHandle, _ := parentAuthor["handle"].(string)

		parentCompact := UltraCompactID(parentHandle, parentRkey)
		indicator := ThreadingIndicator(depth, parentCompact, authorID)
		sb.WriteString(fmt.Sprintf("%s\n", indicator))
	}

	// Mark this post as seen
	seenPosts[fullID] = true

	// Blockquote the content (ALWAYS FLUSH-LEFT, NO INDENTATION)
	sb.WriteString(BlockquoteContent(text))
	sb.WriteString("\n")

	// Stats and timestamp on same line (FLUSH-LEFT)
	stats := FormatStats(likes, reposts, quotes, replies)
	timestamp := FormatTimestamp(createdAt)

	if stats != "" {
		sb.WriteString(fmt.Sprintf("%s  %s\n", stats, timestamp))
	} else {
		sb.WriteString(fmt.Sprintf("%s\n", timestamp))
	}

	// Blank line before next post
	sb.WriteString("\n")

	// Process replies recursively
	if repliesArray, ok := node["replies"].([]interface{}); ok {
		for _, reply := range repliesArray {
			if replyMap, ok := reply.(map[string]interface{}); ok {
				t.formatThreadRecursive(sb, replyMap, seenPosts, depth+1, post)
			}
		}
	}
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
