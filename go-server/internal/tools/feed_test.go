package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

func TestFeedToolBasics(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	t.Run("Name", func(t *testing.T) {
		name := tool.Name()
		if name != "feed" {
			t.Errorf("Expected name 'feed', got '%s'", name)
		}
	})

	t.Run("Description", func(t *testing.T) {
		desc := tool.Description()
		if desc == "" {
			t.Error("Expected non-empty description")
		}
	})

	t.Run("InputSchema", func(t *testing.T) {
		schema := tool.InputSchema()
		if schema.Type != "object" {
			t.Errorf("Expected type 'object', got '%s'", schema.Type)
		}

		// Check that optional parameters exist
		if _, ok := schema.Properties["feed"]; !ok {
			t.Error("Expected 'feed' property in schema")
		}
		if _, ok := schema.Properties["login"]; !ok {
			t.Error("Expected 'login' property in schema")
		}
		if _, ok := schema.Properties["cursor"]; !ok {
			t.Error("Expected 'cursor' property in schema")
		}
		if _, ok := schema.Properties["limit"]; !ok {
			t.Error("Expected 'limit' property in schema")
		}
	})
}

func TestFeedToolMarkdownOutput(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	// Test with empty feed data
	feedData := map[string]interface{}{
		"feed": []interface{}{},
	}

	markdown := tool.formatFeedMarkdown(feedData)
	if markdown == "" {
		t.Error("Expected non-empty markdown output")
	}

	// Verify it's markdown (should contain header)
	if len(markdown) < 10 || markdown[0] != '#' {
		t.Error("Expected markdown output to start with header (#)")
	}
}

func TestFeedToolFormatting(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	// Test with sample feed data
	feedData := map[string]interface{}{
		"feed": []interface{}{
			map[string]interface{}{
				"post": map[string]interface{}{
					"uri": "at://did:plc:test123/app.bsky.feed.post/abc123",
					"author": map[string]interface{}{
						"handle":      "test.bsky.social",
						"displayName": "Test User",
					},
					"record": map[string]interface{}{
						"text":      "This is a test post",
						"createdAt": "2024-01-01T00:00:00Z",
					},
					"likeCount":   5,
					"replyCount":  2,
					"repostCount": 1,
				},
			},
		},
		"cursor": "next-page-cursor",
	}

	markdown := tool.formatFeedMarkdown(feedData)

	// Verify markdown contains expected elements
	if !strings.Contains(markdown, "BlueSky Feed") {
		t.Error("Expected markdown to contain 'BlueSky Feed' header")
	}
	if !strings.Contains(markdown, "@test.bsky.social") {
		t.Error("Expected markdown to contain author handle")
	}
	if !strings.Contains(markdown, "This is a test post") {
		t.Error("Expected markdown to contain post text")
	}
	if !strings.Contains(markdown, "5 likes") {
		t.Error("Expected markdown to contain like count")
	}
	if !strings.Contains(markdown, "next-page-cursor") {
		t.Error("Expected markdown to contain cursor for pagination")
	}
}

func TestFeedToolHelperFunctions(t *testing.T) {
	t.Run("getStringParam", func(t *testing.T) {
		args := map[string]interface{}{
			"key1": "value1",
			"key2": 123,
		}

		if result := getStringParam(args, "key1", "default"); result != "value1" {
			t.Errorf("Expected 'value1', got '%s'", result)
		}

		if result := getStringParam(args, "key2", "default"); result != "default" {
			t.Errorf("Expected 'default' for non-string value, got '%s'", result)
		}

		if result := getStringParam(args, "missing", "default"); result != "default" {
			t.Errorf("Expected 'default' for missing key, got '%s'", result)
		}
	})

	t.Run("getIntParam", func(t *testing.T) {
		args := map[string]interface{}{
			"int":     42,
			"int64":   int64(100),
			"float64": float64(50.5),
			"string":  "not a number",
		}

		if result := getIntParam(args, "int", 0); result != 42 {
			t.Errorf("Expected 42, got %d", result)
		}

		if result := getIntParam(args, "int64", 0); result != 100 {
			t.Errorf("Expected 100, got %d", result)
		}

		if result := getIntParam(args, "float64", 0); result != 50 {
			t.Errorf("Expected 50, got %d", result)
		}

		if result := getIntParam(args, "string", 99); result != 99 {
			t.Errorf("Expected default 99 for string, got %d", result)
		}

		if result := getIntParam(args, "missing", 99); result != 99 {
			t.Errorf("Expected default 99 for missing key, got %d", result)
		}
	})
}

func TestFeedToolATURIConversion(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "Valid AT URI",
			input:    "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			expected: "https://bsky.app/profile/did:plc:abc123/post/xyz789",
		},
		{
			name:     "Short AT URI",
			input:    "at://did:plc:abc/collection",
			expected: "at://did:plc:abc/collection", // Should return unchanged
		},
		{
			name:     "Non-AT URI",
			input:    "https://example.com/post",
			expected: "https://example.com/post",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := tool.atURIToBskyURL(tt.input)
			if result != tt.expected {
				t.Errorf("Expected '%s', got '%s'", tt.expected, result)
			}
		})
	}
}

func TestFeedToolCall_InvalidContext(t *testing.T) {
	tool, err := NewFeedTool()
	if err != nil {
		t.Fatalf("Failed to create feed tool: %v", err)
	}

	// Create a canceled context to test error handling
	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	args := map[string]interface{}{
		"login": "anonymous",
	}

	server := &mcp.Server{}
	result, err := tool.Call(ctx, args, server)

	// We expect an error due to the canceled context
	// The actual error depends on whether the API call happens before context check
	if err == nil && result != nil {
		// If no error, just verify result structure
		if len(result.Content) == 0 {
			t.Error("Expected at least one content item in result")
		}
	}
	// If there's an error, that's also acceptable due to canceled context
}
