// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"strings"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// ThreadTool implements the thread tool
type ThreadTool struct {
	client *bluesky.Client
}

// NewThreadTool creates a new thread tool
func NewThreadTool() (*ThreadTool, error) {
	client, err := bluesky.NewClient()
	if err != nil {
		return nil, fmt.Errorf("failed to create BlueSky client: %w", err)
	}

	return &ThreadTool{
		client: client,
	}, nil
}

// Name returns the tool name
func (t *ThreadTool) Name() string {
	return "thread"
}

// Description returns the tool description
func (t *ThreadTool) Description() string {
	return "Fetch a thread by post URI - it returns all the replies and replies to replies, the whole conversation. " +
		"If you're already logged in, this will fetch the thread as viewed by the logged in user. " +
		"If the handle is 'anonymous', it will fetch the thread in incognito mode. " +
		"Messages in the thread are sometimes called skeets, tweets, or posts - they're all the same thing."
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
				Description: "(Optional) BlueSky handle to use for authenticated fetch.",
			},
		},
		Required: []string{"postURI"},
	}
}

// Call executes the thread tool
func (t *ThreadTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate postURI
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
	login := ""
	if v, ok := args["login"]; ok {
		if s, ok := v.(string); ok {
			login = strings.TrimSpace(s)
			// Treat "anonymous" as empty login
			if login == "anonymous" {
				login = ""
			}
		}
	}

	// Fetch thread
	threadResp, err := t.client.GetPostThread(ctx, login, postURI)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to fetch thread")
	}

	// Format as markdown
	markdown := t.formatThreadAsMarkdown(threadResp)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// normalizePostURI converts various post URI formats to AT URI
func (t *ThreadTool) normalizePostURI(uri string) string {
	// Already an AT URI
	if strings.HasPrefix(uri, "at://") {
		return uri
	}

	// Try to parse BlueSky web URLs
	// Format: https://bsky.app/profile/{handle}/post/{rkey}
	if strings.Contains(uri, "bsky.app/profile/") {
		parts := strings.Split(uri, "/")
		if len(parts) >= 6 {
			// Find profile and post segments
			for i := 0; i < len(parts)-2; i++ {
				if parts[i] == "profile" && i+2 < len(parts) && parts[i+2] == "post" && i+3 < len(parts) {
					handle := parts[i+1]
					rkey := parts[i+3]
					// We need to resolve handle to DID, but for now return as-is
					// In production, you'd want to resolve the handle to DID first
					return fmt.Sprintf("at://%s/app.bsky.feed.post/%s", handle, rkey)
				}
			}
		}
	}

	// Return as-is if we can't parse it
	return uri
}

// formatThreadAsMarkdown formats the thread response as markdown
func (t *ThreadTool) formatThreadAsMarkdown(thread *bluesky.ThreadResponse) string {
	var sb strings.Builder

	sb.WriteString("# Thread\n\n")

	// Flatten the thread into a list of posts
	posts := t.flattenThread(&thread.Thread)

	if len(posts) == 0 {
		sb.WriteString("No posts found in this thread.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("Found %d posts in this conversation.\n\n", len(posts)))

	// Format each post
	for i, post := range posts {
		if post == nil {
			continue
		}

		sb.WriteString(fmt.Sprintf("## Post %d\n\n", i+1))

		// Author info
		authorDisplay := fmt.Sprintf("@%s", post.Author.Handle)
		if post.Author.DisplayName != "" {
			authorDisplay = fmt.Sprintf("%s (@%s)", post.Author.DisplayName, post.Author.Handle)
		}
		sb.WriteString(fmt.Sprintf("**Author:** %s\n\n", authorDisplay))

		// Post URI for reference
		sb.WriteString(fmt.Sprintf("**Post URI:** %s\n\n", post.URI))

		// Post content
		if post.Record.Text != "" {
			// Quote the post text
			lines := strings.Split(post.Record.Text, "\n")
			for _, line := range lines {
				sb.WriteString(fmt.Sprintf("> %s\n", line))
			}
			sb.WriteString("\n")
		}

		// Engagement stats
		if post.LikeCount > 0 || post.ReplyCount > 0 || post.RepostCount > 0 || post.QuoteCount > 0 {
			var stats []string
			if post.LikeCount > 0 {
				stats = append(stats, fmt.Sprintf("%d likes", post.LikeCount))
			}
			if post.ReplyCount > 0 {
				stats = append(stats, fmt.Sprintf("%d replies", post.ReplyCount))
			}
			if post.RepostCount > 0 {
				stats = append(stats, fmt.Sprintf("%d reposts", post.RepostCount))
			}
			if post.QuoteCount > 0 {
				stats = append(stats, fmt.Sprintf("%d quotes", post.QuoteCount))
			}
			sb.WriteString(fmt.Sprintf("**Engagement:** %s\n\n", strings.Join(stats, ", ")))
		}

		// Reply info
		if post.Record.Reply != nil && post.Record.Reply.Parent != nil {
			sb.WriteString(fmt.Sprintf("**Reply to:** %s\n\n", post.Record.Reply.Parent.URI))
		}

		// Timestamp
		if post.Record.CreatedAt != "" {
			sb.WriteString(fmt.Sprintf("**Created:** %s\n\n", post.Record.CreatedAt))
		}

		// Separator between posts
		if i < len(posts)-1 {
			sb.WriteString("---\n\n")
		}
	}

	return sb.String()
}

// flattenThread flattens a nested thread structure into a flat list of posts
func (t *ThreadTool) flattenThread(node *bluesky.ThreadNode) []*bluesky.FeedPost {
	var posts []*bluesky.FeedPost

	if node == nil {
		return posts
	}

	// Add parent posts first (to show context)
	if node.Parent != nil {
		posts = append(posts, t.flattenThread(node.Parent)...)
	}

	// Add current post
	if node.Post != nil {
		posts = append(posts, node.Post)
	}

	// Add replies
	if len(node.Replies) > 0 {
		for _, reply := range node.Replies {
			posts = append(posts, t.flattenThread(&reply)...)
		}
	}

	return posts
}
