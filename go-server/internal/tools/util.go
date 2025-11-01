// Package tools provides MCP tool implementations
package tools

import (
	"fmt"
	"strings"
)

// PostReference represents a parsed post reference
type PostReference struct {
	DID  string
	RKey string
}

// parsePostReference parses an AT URI or Bluesky URL into its components
func parsePostReference(ref string) (*PostReference, error) {
	ref = strings.TrimSpace(ref)

	// Handle at:// URI format: at://did:plc:xyz/app.bsky.feed.post/rkey
	if strings.HasPrefix(ref, "at://") {
		parts := strings.Split(strings.TrimPrefix(ref, "at://"), "/")
		if len(parts) < 3 {
			return nil, fmt.Errorf("invalid AT URI format: %s", ref)
		}
		return &PostReference{
			DID:  parts[0],
			RKey: parts[2],
		}, nil
	}

	// Handle compact format @handle/rkey
	if strings.HasPrefix(ref, "@") && strings.Contains(ref, "/") {
		parts := strings.SplitN(ref[1:], "/", 2) // Remove @ and split on first /
		if len(parts) == 2 {
			// Note: For the post tool, we can't resolve handles here without context
			// This format should be handled by the caller with proper handle resolution
			return nil, fmt.Errorf("compact format @handle/rkey requires handle resolution - please use at:// URI or provide handle resolution context")
		}
	}

	// Handle https://bsky.app/profile/{handle}/post/{rkey} format
	if strings.HasPrefix(ref, "https://bsky.app/profile/") {
		ref = strings.TrimPrefix(ref, "https://bsky.app/profile/")
		parts := strings.Split(ref, "/post/")
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid Bluesky URL format: %s", ref)
		}

		handleOrDID := parts[0]
		rkey := parts[1]

		// If it's not a DID, we need to resolve the handle
		// For now, we'll accept both DIDs and handles
		// The API should handle handle resolution
		if !strings.HasPrefix(handleOrDID, "did:") {
			// This is a handle, we need to resolve it to a DID
			// For simplicity, we'll try to use it directly and let the API handle it
			// In a more robust implementation, we'd resolve the handle first
			return nil, fmt.Errorf("handle resolution not yet implemented, please use at:// URI format or DID in URL")
		}

		return &PostReference{
			DID:  handleOrDID,
			RKey: rkey,
		}, nil
	}

	return nil, fmt.Errorf("unsupported post reference format (use at:// URI, https://bsky.app/... URL, or @handle/rkey with proper context): %s", ref)
}
