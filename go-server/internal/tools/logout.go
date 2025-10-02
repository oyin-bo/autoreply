// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"strings"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// LogoutTool implements the logout tool
type LogoutTool struct {
	credStore *auth.CredentialStore
}

// NewLogoutTool creates a new logout tool
func NewLogoutTool() (*LogoutTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &LogoutTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *LogoutTool) Name() string {
	return "logout"
}

// Description returns the tool description
func (t *LogoutTool) Description() string {
	return "Remove stored credentials for a Bluesky account"
}

// InputSchema returns the JSON schema for tool input
func (t *LogoutTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"handle": {
				Type:        "string",
				Description: "Bluesky handle to logout (optional, uses default if not provided)",
			},
		},
		Required: []string{},
	}
}

// Call executes the logout tool
func (t *LogoutTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	var handle string

	// Extract handle parameter (optional)
	if handleRaw, ok := args["handle"]; ok {
		handleStr, ok := handleRaw.(string)
		if !ok {
			return nil, errors.NewMCPError(errors.InvalidInput, "handle must be a string")
		}
		handle = strings.TrimSpace(strings.TrimPrefix(handleStr, "@"))
	}

	// If no handle provided, use default
	if handle == "" {
		defaultHandle, err := t.credStore.GetDefault()
		if err != nil {
			return nil, errors.Wrap(err, errors.InvalidInput, "No handle provided and no default handle set")
		}
		handle = defaultHandle
	}

	// Delete credentials
	if err := t.credStore.Delete(handle); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to delete credentials")
	}

	// Format success message
	message := fmt.Sprintf("# Logout Successful\n\n"+
		"Credentials for **@%s** have been removed.\n",
		handle)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
	}, nil
}
