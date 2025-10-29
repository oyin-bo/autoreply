// Package tools provides MCP tool implementations
package tools

import (
	"context"
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
		if _, ok := schema.Properties["login"]; !ok {
			t.Error("Expected 'login' property in schema")
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

	// Verify markdown contains expected elements
	if !contains(markdown, "BlueSky Thread") {
		t.Error("Expected markdown to contain 'BlueSky Thread' header")
	}
	if !contains(markdown, "@test.bsky.social") {
		t.Error("Expected markdown to contain main post author handle")
	}
	if !contains(markdown, "This is the main post") {
		t.Error("Expected markdown to contain main post text")
	}
	if !contains(markdown, "@reply.bsky.social") {
		t.Error("Expected markdown to contain reply author handle")
	}
	if !contains(markdown, "This is a reply") {
		t.Error("Expected markdown to contain reply text")
	}
	if !contains(markdown, "10 likes") {
		t.Error("Expected markdown to contain like count")
	}
}

func TestThreadToolFlattenThread(t *testing.T) {
	tool, err := NewThreadTool()
	if err != nil {
		t.Fatalf("Failed to create thread tool: %v", err)
	}

	t.Run("SinglePost", func(t *testing.T) {
		node := map[string]interface{}{
			"post": map[string]interface{}{
				"uri": "at://did:plc:test/app.bsky.feed.post/123",
			},
		}

		posts := tool.flattenThread(node)
		if len(posts) != 1 {
			t.Errorf("Expected 1 post, got %d", len(posts))
		}
	})

	t.Run("PostWithReplies", func(t *testing.T) {
		node := map[string]interface{}{
			"post": map[string]interface{}{
				"uri": "at://did:plc:test/app.bsky.feed.post/123",
			},
			"replies": []interface{}{
				map[string]interface{}{
					"post": map[string]interface{}{
						"uri": "at://did:plc:test/app.bsky.feed.post/456",
					},
				},
				map[string]interface{}{
					"post": map[string]interface{}{
						"uri": "at://did:plc:test/app.bsky.feed.post/789",
					},
				},
			},
		}

		posts := tool.flattenThread(node)
		if len(posts) != 3 {
			t.Errorf("Expected 3 posts, got %d", len(posts))
		}
	})

	t.Run("NestedReplies", func(t *testing.T) {
		node := map[string]interface{}{
			"post": map[string]interface{}{
				"uri": "at://did:plc:test/app.bsky.feed.post/123",
			},
			"replies": []interface{}{
				map[string]interface{}{
					"post": map[string]interface{}{
						"uri": "at://did:plc:test/app.bsky.feed.post/456",
					},
					"replies": []interface{}{
						map[string]interface{}{
							"post": map[string]interface{}{
								"uri": "at://did:plc:test/app.bsky.feed.post/789",
							},
						},
					},
				},
			},
		}

		posts := tool.flattenThread(node)
		if len(posts) != 3 {
			t.Errorf("Expected 3 posts (including nested), got %d", len(posts))
		}
	})

	t.Run("EmptyNode", func(t *testing.T) {
		node := map[string]interface{}{}
		posts := tool.flattenThread(node)
		if len(posts) != 0 {
			t.Errorf("Expected 0 posts for empty node, got %d", len(posts))
		}
	})
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
		{
			name:     "Web URL returned as-is",
			input:    "https://bsky.app/profile/user.bsky.social/post/123",
			expected: "https://bsky.app/profile/user.bsky.social/post/123",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := tool.normalizePostURI(tt.input)
			if result != tt.expected {
				t.Errorf("Expected '%s', got '%s'", tt.expected, result)
			}
		})
	}
}
