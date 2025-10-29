// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// TestLoginFallback_NoElicitation verifies that when the client does not support
// elicitation, the login tool returns a guidance message marked as IsError.
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

	// Missing handle triggers elicitation need; without support we expect fallback
	args := map[string]interface{}{}

	result, callErr := tool.Call(context.Background(), args, server)
	if callErr != nil {
		t.Fatalf("Unexpected error from login.Call: %v", callErr)
	}
	if result == nil {
		t.Fatalf("Expected non-nil ToolResult")
	}
	if !result.IsError {
		t.Errorf("Expected result.IsError=true for fallback guidance")
	}
	if len(result.Content) == 0 {
		t.Fatalf("Expected at least one content item")
	}
	text := result.Content[0].Text
	if !strings.Contains(strings.ToLower(text), "does not support interactive prompts") {
		t.Errorf("Expected guidance about missing elicitation support, got: %s", text)
	}
}
