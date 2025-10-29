// Package tools provides MCP tool implementations
package tools

import (
	"strings"
	"testing"
)

// TestPostToolBasics tests basic tool properties
func TestPostToolBasics(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	t.Run("Name", func(t *testing.T) {
		if tool.Name() != "post" {
			t.Errorf("Expected name 'post', got '%s'", tool.Name())
		}
	})

	t.Run("Description", func(t *testing.T) {
		desc := tool.Description()
		if desc == "" {
			t.Error("Description should not be empty")
		}
		if !strings.Contains(strings.ToLower(desc), "post") {
			t.Errorf("Description should mention 'post', got: %s", desc)
		}
	})

	t.Run("InputSchema", func(t *testing.T) {
		schema := tool.InputSchema()

		if schema.Type != "object" {
			t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
		}

		// Check for text parameter (required)
		textProp, ok := schema.Properties["text"]
		if !ok {
			t.Fatal("Schema missing 'text' property")
		}

		if textProp.Type != "string" {
			t.Errorf("Text property should be string, got %s", textProp.Type)
		}

		// Text should be required
		found := false
		for _, req := range schema.Required {
			if req == "text" {
				found = true
				break
			}
		}
		if !found {
			t.Error("Text should be in required fields")
		}

		// Check for postAs parameter (optional)
		if postAsProp, ok := schema.Properties["postAs"]; ok {
			if postAsProp.Type != "string" {
				t.Errorf("PostAs property should be string, got %s", postAsProp.Type)
			}
		} else {
			t.Error("Schema missing 'postAs' property")
		}

		// Check for replyTo parameter (optional)
		if replyToProp, ok := schema.Properties["replyTo"]; ok {
			if replyToProp.Type != "string" {
				t.Errorf("ReplyTo property should be string, got %s", replyToProp.Type)
			}
		} else {
			t.Error("Schema missing 'replyTo' property")
		}
	})
}

// TestParsePostReference tests URL/URI parsing
func TestParsePostReference(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		wantDID   string
		wantRKey  string
		wantError bool
	}{
		{
			name:      "AT URI format",
			input:     "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			wantDID:   "did:plc:abc123",
			wantRKey:  "xyz789",
			wantError: false,
		},
		{
			name:      "Bluesky URL with DID",
			input:     "https://bsky.app/profile/did:plc:abc123/post/xyz789",
			wantDID:   "did:plc:abc123",
			wantRKey:  "xyz789",
			wantError: false,
		},
		{
			name:      "Bluesky URL with handle (should error)",
			input:     "https://bsky.app/profile/alice.bsky.social/post/xyz789",
			wantError: true,
		},
		{
			name:      "Invalid AT URI",
			input:     "at://did:plc:abc123",
			wantError: true,
		},
		{
			name:      "Invalid format",
			input:     "https://example.com/post/123",
			wantError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ref, err := parsePostReference(tt.input)

			if tt.wantError {
				if err == nil {
					t.Error("Expected error, got nil")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if ref.DID != tt.wantDID {
				t.Errorf("Expected DID %s, got %s", tt.wantDID, ref.DID)
			}

			if ref.RKey != tt.wantRKey {
				t.Errorf("Expected RKey %s, got %s", tt.wantRKey, ref.RKey)
			}
		})
	}
}

// TestPostToolATURIConversion tests AT URI to Bluesky URL conversion
func TestPostToolATURIConversion(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	tests := []struct {
		name    string
		atURI   string
		handle  string
		wantURL string
	}{
		{
			name:    "With handle",
			atURI:   "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			handle:  "alice.bsky.social",
			wantURL: "https://bsky.app/profile/alice.bsky.social/post/xyz789",
		},
		{
			name:    "Without handle (use DID)",
			atURI:   "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			handle:  "",
			wantURL: "https://bsky.app/profile/did:plc:abc123/post/xyz789",
		},
		{
			name:    "Handle with @ prefix",
			atURI:   "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			handle:  "@alice.bsky.social",
			wantURL: "https://bsky.app/profile/alice.bsky.social/post/xyz789",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			url := tool.atURIToBskyURL(tt.atURI, tt.handle)
			if url != tt.wantURL {
				t.Errorf("Expected URL %s, got %s", tt.wantURL, url)
			}
		})
	}
}
