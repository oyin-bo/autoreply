// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
)

func TestThreadTool_Name(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	if tool.Name() != "thread" {
		t.Errorf("Expected name 'thread', got '%s'", tool.Name())
	}
}

func TestThreadTool_Description(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	desc := tool.Description()
	if desc == "" {
		t.Error("Description should not be empty")
	}
}

func TestThreadTool_InputSchema(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	schema := tool.InputSchema()
	
	// Verify schema type
	if schema.Type != "object" {
		t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
	}

	// Verify schema has postURI property
	if _, ok := schema.Properties["postURI"]; !ok {
		t.Error("Expected property 'postURI' in schema")
	}

	// Verify schema has login property
	if _, ok := schema.Properties["login"]; !ok {
		t.Error("Expected property 'login' in schema")
	}

	// Verify postURI is required
	foundRequired := false
	for _, req := range schema.Required {
		if req == "postURI" {
			foundRequired = true
			break
		}
	}
	if !foundRequired {
		t.Error("Expected 'postURI' to be a required field")
	}
}

func TestThreadTool_Call_MissingPostURI(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	ctx := context.Background()

	// Test with missing postURI
	args := map[string]interface{}{}

	_, err = tool.Call(ctx, args, nil)
	if err == nil {
		t.Error("Expected error for missing postURI, got nil")
	}
}

func TestThreadTool_Call_EmptyPostURI(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	ctx := context.Background()

	// Test with empty postURI
	args := map[string]interface{}{
		"postURI": "",
	}

	_, err = tool.Call(ctx, args, nil)
	if err == nil {
		t.Error("Expected error for empty postURI, got nil")
	}
}

func TestThreadTool_NormalizePostURI_ATProtocol(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Test with AT protocol URI (should remain unchanged)
	atURI := "at://did:plc:test/app.bsky.feed.post/test123"
	normalized := tool.normalizePostURI(atURI)

	if normalized != atURI {
		t.Errorf("Expected AT URI to remain unchanged: got '%s', expected '%s'", normalized, atURI)
	}
}

func TestThreadTool_NormalizePostURI_BlueSkyURL(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Test with BlueSky web URL
	webURL := "https://bsky.app/profile/test.bsky.social/post/test123"
	normalized := tool.normalizePostURI(webURL)

	// Should convert to AT URI format
	expectedContains := "test.bsky.social"
	if !containsString(normalized, expectedContains) {
		t.Errorf("Expected normalized URI to contain '%s', got '%s'", expectedContains, normalized)
	}
}

func TestThreadTool_FormatMarkdown(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Create a mock thread response
	mockThread := &bluesky.ThreadResponse{
		Thread: bluesky.ThreadNode{
			Post: &bluesky.FeedPost{
				URI:    "at://did:plc:test/app.bsky.feed.post/test123",
				Author: bluesky.Author{Handle: "test.bsky.social", DisplayName: "Test User"},
				Record: bluesky.FeedPostRecord{
					Text:      "This is a test post in a thread",
					CreatedAt: "2025-01-01T12:00:00Z",
				},
				LikeCount: 10,
			},
			Replies: []bluesky.ThreadNode{
				{
					Post: &bluesky.FeedPost{
						URI:    "at://did:plc:test/app.bsky.feed.post/reply123",
						Author: bluesky.Author{Handle: "reply.bsky.social", DisplayName: "Reply User"},
						Record: bluesky.FeedPostRecord{
							Text:      "This is a reply",
							CreatedAt: "2025-01-01T12:05:00Z",
						},
					},
				},
			},
		},
	}

	markdown := tool.formatThreadAsMarkdown(mockThread)

	// Verify markdown contains expected elements
	expectedStrings := []string{
		"# Thread",
		"Found 2 posts",
		"@test.bsky.social",
		"This is a test post in a thread",
		"@reply.bsky.social",
		"This is a reply",
		"10 likes",
	}

	for _, expected := range expectedStrings {
		if !containsString(markdown, expected) {
			t.Errorf("Expected markdown to contain '%s'", expected)
		}
	}
}

func TestThreadTool_FormatMarkdown_EmptyThread(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Create an empty thread response
	mockThread := &bluesky.ThreadResponse{
		Thread: bluesky.ThreadNode{},
	}

	markdown := tool.formatThreadAsMarkdown(mockThread)

	// Verify markdown contains "No posts found"
	if !containsString(markdown, "No posts found") {
		t.Error("Expected markdown to contain 'No posts found' for empty thread")
	}
}

func TestThreadTool_FlattenThread(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Create a nested thread
	mockThread := &bluesky.ThreadNode{
		Post: &bluesky.FeedPost{
			URI:    "at://did:plc:test/app.bsky.feed.post/root",
			Author: bluesky.Author{Handle: "root.bsky.social"},
			Record: bluesky.FeedPostRecord{Text: "Root post"},
		},
		Replies: []bluesky.ThreadNode{
			{
				Post: &bluesky.FeedPost{
					URI:    "at://did:plc:test/app.bsky.feed.post/reply1",
					Author: bluesky.Author{Handle: "reply1.bsky.social"},
					Record: bluesky.FeedPostRecord{Text: "Reply 1"},
				},
			},
			{
				Post: &bluesky.FeedPost{
					URI:    "at://did:plc:test/app.bsky.feed.post/reply2",
					Author: bluesky.Author{Handle: "reply2.bsky.social"},
					Record: bluesky.FeedPostRecord{Text: "Reply 2"},
				},
			},
		},
	}

	posts := tool.flattenThread(mockThread)

	// Should have 3 posts (root + 2 replies)
	if len(posts) != 3 {
		t.Errorf("Expected 3 posts, got %d", len(posts))
	}

	// Verify posts are in expected order
	if posts[0].Record.Text != "Root post" {
		t.Errorf("Expected first post to be root, got '%s'", posts[0].Record.Text)
	}
}
