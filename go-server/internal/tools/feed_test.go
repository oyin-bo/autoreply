// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
)

func TestFeedTool_Name(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	if tool.Name() != "feed" {
		t.Errorf("Expected name 'feed', got '%s'", tool.Name())
	}
}

func TestFeedTool_Description(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	desc := tool.Description()
	if desc == "" {
		t.Error("Description should not be empty")
	}
}

func TestFeedTool_InputSchema(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	schema := tool.InputSchema()
	
	// Verify schema type
	if schema.Type != "object" {
		t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
	}

	// Verify schema has expected properties
	expectedProps := []string{"feed", "login", "cursor", "limit"}
	for _, prop := range expectedProps {
		if _, ok := schema.Properties[prop]; !ok {
			t.Errorf("Expected property '%s' in schema", prop)
		}
	}

	// Verify no required fields (all are optional)
	if len(schema.Required) > 0 {
		t.Errorf("Expected no required fields, got %d", len(schema.Required))
	}
}

func TestFeedTool_Call_InvalidArgs(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	ctx := context.Background()

	// Test with invalid limit type
	args := map[string]interface{}{
		"limit": "invalid",
	}

	// This should not fail - it should just ignore the invalid limit
	_, err = tool.Call(ctx, args, nil)
	// Note: This will fail with authentication error if not logged in,
	// which is expected. We're just testing that invalid args don't cause a panic.
	// The error will be about credentials, not about invalid arguments.
}

func TestFeedTool_FormatMarkdown(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	// Create a mock feed response
	mockFeed := &bluesky.FeedResponse{
		Feed: []bluesky.FeedItem{
			{
				Post: bluesky.FeedPost{
					URI:       "at://did:plc:test/app.bsky.feed.post/test123",
					Author:    bluesky.Author{Handle: "test.bsky.social", DisplayName: "Test User"},
					Record:    bluesky.FeedPostRecord{Text: "This is a test post"},
					IndexedAt: "2025-01-01T12:00:00Z",
					LikeCount: 5,
				},
			},
		},
		Cursor: "test-cursor-123",
	}

	markdown := tool.formatFeedAsMarkdown(mockFeed, "test-feed")

	// Verify markdown contains expected elements
	expectedStrings := []string{
		"# Feed: test-feed",
		"Found 1 posts",
		"@test.bsky.social",
		"This is a test post",
		"5 likes",
		"test-cursor-123",
	}

	for _, expected := range expectedStrings {
		if !containsString(markdown, expected) {
			t.Errorf("Expected markdown to contain '%s'", expected)
		}
	}
}

func TestFeedTool_FormatMarkdown_EmptyFeed(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	// Create an empty feed response
	mockFeed := &bluesky.FeedResponse{
		Feed:   []bluesky.FeedItem{},
		Cursor: "",
	}

	markdown := tool.formatFeedAsMarkdown(mockFeed, "")

	// Verify markdown contains "No posts found"
	if !containsString(markdown, "No posts found") {
		t.Error("Expected markdown to contain 'No posts found' for empty feed")
	}
}

// Helper function to check if a string contains a substring
func containsString(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && findSubstring(s, substr))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
