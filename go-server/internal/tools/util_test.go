// Package tools provides MCP tool implementations
package tools

import (
	"testing"
)

// TestParsePostReferenceUtil tests the shared parsePostReference utility function
func TestParsePostReferenceUtil(t *testing.T) {
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
			name:      "AT URI with extra segments",
			input:     "at://did:plc:abc123/app.bsky.feed.post/xyz789/extra",
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
			name:      "Bluesky URL with handle (not supported)",
			input:     "https://bsky.app/profile/alice.bsky.social/post/xyz789",
			wantError: true,
		},
		{
			name:      "Invalid AT URI - too few parts",
			input:     "at://did:plc:abc123/app.bsky.feed.post",
			wantError: true,
		},
		{
			name:      "Invalid AT URI - no collection",
			input:     "at://did:plc:abc123",
			wantError: true,
		},
		{
			name:      "Invalid Bluesky URL - missing /post/",
			input:     "https://bsky.app/profile/did:plc:abc123/xyz789",
			wantError: true,
		},
		{
			name:      "Invalid format - not AT URI or Bluesky URL",
			input:     "https://example.com/post/123",
			wantError: true,
		},
		{
			name:      "Empty string",
			input:     "",
			wantError: true,
		},
		{
			name:      "Whitespace only",
			input:     "   ",
			wantError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ref, err := parsePostReference(tt.input)

			if tt.wantError {
				if err == nil {
					t.Errorf("Expected error for input %q, got nil", tt.input)
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error for input %q: %v", tt.input, err)
				return
			}

			if ref.DID != tt.wantDID {
				t.Errorf("Expected DID %q, got %q", tt.wantDID, ref.DID)
			}

			if ref.RKey != tt.wantRKey {
				t.Errorf("Expected RKey %q, got %q", tt.wantRKey, ref.RKey)
			}
		})
	}
}
