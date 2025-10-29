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

// FeedTool implements the feed tool
type FeedTool struct {
	client *bluesky.Client
}

// NewFeedTool creates a new feed tool
func NewFeedTool() (*FeedTool, error) {
	client, err := bluesky.NewClient()
	if err != nil {
		return nil, fmt.Errorf("failed to create BlueSky client: %w", err)
	}

	return &FeedTool{
		client: client,
	}, nil
}

// Name returns the tool name
func (t *FeedTool) Name() string {
	return "feed"
}

// Description returns the tool description
func (t *FeedTool) Description() string {
	return "Get the latest feed from BlueSky. Returns a list of posts (also known as tweets or skeets). " +
		"If you want to see the latest posts from a specific user, just provide their handle. " +
		"These feeds are paginated - you get the top chunk and a cursor, which you can use to get more posts."
}

// InputSchema returns the JSON schema for tool input
func (t *FeedTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"feed": {
				Type: "string",
				Description: "(Optional) The feed to retrieve, can be a BlueSky feed URI, or a name for a feed to search for. " +
					"If unspecified, it will return the default popular feed 'What's Hot'.",
			},
			"login": {
				Type: "string",
				Description: "(Optional) BlueSky handle for which the feed is requested. " +
					"If unspecified, or specified as 'anonymous', the feed will be retrieved in incognito mode.",
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
	feedURI := ""
	if v, ok := args["feed"]; ok {
		if s, ok := v.(string); ok {
			feedURI = strings.TrimSpace(s)
		}
	}

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

	cursor := ""
	if v, ok := args["cursor"]; ok {
		if s, ok := v.(string); ok {
			cursor = strings.TrimSpace(s)
		}
	}

	limit := 20
	if v, ok := args["limit"]; ok {
		switch val := v.(type) {
		case float64:
			limit = int(val)
		case int:
			limit = val
		case int64:
			limit = int(val)
		}
	}

	// Validate limit
	if limit < 1 {
		limit = 1
	}
	if limit > 100 {
		limit = 100
	}

	// Fetch feed
	var feedResp *bluesky.FeedResponse
	var err error

	// If login is provided and no specific feed, use timeline
	if login != "" && feedURI == "" {
		feedResp, err = t.client.GetTimeline(ctx, login, cursor, limit)
	} else {
		feedResp, err = t.client.GetFeed(ctx, login, feedURI, cursor, limit)
	}

	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to fetch feed")
	}

	// Format as markdown
	markdown := t.formatFeedAsMarkdown(feedResp, feedURI)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// formatFeedAsMarkdown formats the feed response as markdown
func (t *FeedTool) formatFeedAsMarkdown(feed *bluesky.FeedResponse, feedName string) string {
	var sb strings.Builder

	// Header
	if feedName != "" {
		sb.WriteString(fmt.Sprintf("# Feed: %s\n\n", feedName))
	} else {
		sb.WriteString("# BlueSky Feed\n\n")
	}

	if len(feed.Feed) == 0 {
		sb.WriteString("No posts found in this feed.\n")
		return sb.String()
	}

	sb.WriteString(fmt.Sprintf("Found %d posts.\n\n", len(feed.Feed)))

	// Format each post
	for i, item := range feed.Feed {
		post := item.Post
		
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
		if post.IndexedAt != "" {
			sb.WriteString(fmt.Sprintf("**Indexed:** %s\n\n", post.IndexedAt))
		}

		// Separator between posts
		if i < len(feed.Feed)-1 {
			sb.WriteString("---\n\n")
		}
	}

	// Pagination info
	if feed.Cursor != "" {
		sb.WriteString(fmt.Sprintf("\n**Cursor for next page:** `%s`\n", feed.Cursor))
		sb.WriteString("\nTo get more posts, call this tool again with the cursor parameter.\n")
	}

	return sb.String()
}
