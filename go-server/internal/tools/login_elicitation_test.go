// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// TestLoginFallback_NoElicitation verifies that when the client does not support
// elicitation, the login tool uses OAuth by default (not app password elicitation).
func TestLoginFallback_NoElicitation(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	// Real server instance without capabilities -> SupportsElicitation()==false
	server, err := mcp.NewServer()
	if err != nil {
		t.Fatalf("Failed to create MCP server: %v", err)
	}

	// Missing handle should trigger OAuth flow (not elicitation for app password)
	args := map[string]interface{}{}

	result, callErr := tool.Call(context.Background(), args, server)
	if callErr != nil {
		t.Fatalf("Unexpected error from login.Call: %v", callErr)
	}
	if result == nil {
		t.Fatalf("Expected non-nil ToolResult")
	}
	// OAuth flow should succeed and provide login instructions (not an error)
	if result.IsError {
		t.Errorf("Expected result.IsError=false for OAuth flow, got true with: %v", result.Content)
	}
	if len(result.Content) == 0 {
		t.Fatalf("Expected at least one content item")
	}
	text := result.Content[0].Text
	// Should contain OAuth login instructions
	if !strings.Contains(text, "OAuth") && !strings.Contains(text, "browser") {
		t.Errorf("Expected OAuth login instructions, got: %s", text)
	}
}
