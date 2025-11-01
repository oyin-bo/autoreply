package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

func TestThreadToolBasics(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	t.Run("Name", func(t *testing.T) {
		name := tool.Name()
		if name != "thread" {
			t.Errorf("Expected name 'thread', got '%s'", name)
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

		// Check required parameters
		if _, ok := schema.Properties["postURI"]; !ok {
			t.Error("Expected 'postURI' property in schema")
		}

		// Check optional parameters
		if _, ok := schema.Properties["viewAs"]; !ok {
			t.Error("Expected 'viewAs' property in schema")
		}

		// Verify required fields
		hasPostURI := false
		for _, req := range schema.Required {
			if req == "postURI" {
				hasPostURI = true
			}
		}
		if !hasPostURI {
			t.Error("Expected 'postURI' to be in required fields")
		}
	})
}

func TestThreadToolValidation(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	server := &mcp.Server{}
	ctx := context.Background()

	t.Run("MissingPostURI", func(t *testing.T) {
		args := map[string]interface{}{}
		_, err := tool.Call(ctx, args, server)
		if err == nil {
			t.Error("Expected error for missing postURI")
		}
		if mcpErr, ok := err.(*errors.MCPError); ok {
			if mcpErr.Code != errors.InvalidInput {
				t.Errorf("Expected InvalidInput error code, got %v", mcpErr.Code)
			}
		}
	})

	t.Run("EmptyPostURI", func(t *testing.T) {
		args := map[string]interface{}{
			"postURI": "",
		}
		_, err := tool.Call(ctx, args, server)
		if err == nil {
			t.Error("Expected error for empty postURI")
		}
	})

	t.Run("EmptyPostURIWithSpaces", func(t *testing.T) {
		args := map[string]interface{}{
			"postURI": "   ",
		}
		_, err := tool.Call(ctx, args, server)
		if err == nil {
			t.Error("Expected error for postURI with only spaces")
		}
	})

	t.Run("NonStringPostURI", func(t *testing.T) {
		args := map[string]interface{}{
			"postURI": 12345,
		}
		_, err := tool.Call(ctx, args, server)
		if err == nil {
			t.Error("Expected error for non-string postURI")
		}
	})
}

func TestThreadToolMarkdownOutput(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Test with empty thread data
	threadData := map[string]interface{}{
		"thread": map[string]interface{}{},
	}

	markdown := tool.formatThreadMarkdown(threadData)
	if markdown == "" {
		t.Error("Expected non-empty markdown output")
	}

	// Verify it's markdown (should contain header)
	if len(markdown) < 10 || markdown[0] != '#' {
		t.Error("Expected markdown output to start with header (#)")
	}
}

func TestThreadToolFormatting(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	// Test with sample thread data
	threadData := map[string]interface{}{
		"thread": map[string]interface{}{
			"post": map[string]interface{}{
				"uri": "at://did:plc:test123/app.bsky.feed.post/abc123",
				"author": map[string]interface{}{
					"handle":      "test.bsky.social",
					"displayName": "Test User",
				},
				"record": map[string]interface{}{
					"text":      "This is the main post",
					"createdAt": "2024-01-01T00:00:00Z",
				},
				"likeCount":  10,
				"replyCount": 3,
			},
			"replies": []interface{}{
				map[string]interface{}{
					"post": map[string]interface{}{
						"uri": "at://did:plc:test456/app.bsky.feed.post/def456",
						"author": map[string]interface{}{
							"handle":      "reply.bsky.social",
							"displayName": "Reply User",
						},
						"record": map[string]interface{}{
							"text":      "This is a reply",
							"createdAt": "2024-01-01T01:00:00Z",
						},
						"likeCount": 2,
					},
				},
			},
		},
	}

	markdown := tool.formatThreadMarkdown(threadData)

	// Verify markdown contains expected elements per docs/16-mcp-schemas.md spec
	if !strings.Contains(markdown, "# Thread · 2 posts") {
		t.Error("Expected markdown to contain '# Thread · 2 posts' header")
	}
	if !strings.Contains(markdown, "> This is the main post") {
		t.Error("Expected markdown to contain blockquoted main post text")
	}
	if !strings.Contains(markdown, "> This is a reply") {
		t.Error("Expected markdown to contain blockquoted reply text")
	}
	// Should NOT contain old labels
	if strings.Contains(markdown, "**Link:**") {
		t.Error("Expected markdown to NOT contain **Link:** label")
	}
	if strings.Contains(markdown, "**Created:**") {
		t.Error("Expected markdown to NOT contain **Created:** label")
	}
}

func TestThreadToolATURIConversion(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
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

func TestThreadToolNormalizePostURI(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "AT URI unchanged",
			input:    "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			expected: "at://did:plc:abc123/app.bsky.feed.post/xyz789",
		},
		// Note: Web URL test removed - normalizePostURI now requires valid context for handle resolution
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := tool.normalizePostURI(context.Background(), tt.input)
			if err != nil {
				t.Fatalf("normalizePostURI failed: %v", err)
			}
			if result != tt.expected {
				t.Errorf("Expected '%s', got '%s'", tt.expected, result)
			}
		})
	}
}
