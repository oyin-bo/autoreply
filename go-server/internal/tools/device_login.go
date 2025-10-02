// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// DeviceLoginTool implements the device authorization login tool
type DeviceLoginTool struct {
	credStore *auth.CredentialStore
}

// NewDeviceLoginTool creates a new device login tool
func NewDeviceLoginTool() (*DeviceLoginTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &DeviceLoginTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *DeviceLoginTool) Name() string {
	return "device-login"
}

// Description returns the tool description
func (t *DeviceLoginTool) Description() string {
	return "Authenticate with Bluesky using Device Authorization Grant (best for headless/remote environments)"
}

// InputSchema returns the JSON schema for tool input
func (t *DeviceLoginTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"client_id": {
				Type:        "string",
				Description: "OAuth client ID (optional, uses default if not provided)",
			},
		},
		Required: []string{},
	}
}

// Call executes the device login tool
func (t *DeviceLoginTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Note: This implementation requires proper AT Protocol OAuth infrastructure
	message := `# Device Authorization Login Not Yet Fully Configured

## Implementation Status

The AT Protocol OAuth infrastructure has been implemented per the official specification, including:

✅ Server metadata discovery
✅ Identity resolution
✅ PAR, PKCE, and DPoP support

## What's Missing

Device Authorization Grant requires:
1. A publicly accessible client_id URL hosting client metadata
2. Proper OAuth server support for device authorization flow
3. The device authorization endpoint from server metadata

## For Now

Use app password authentication:
` + "```bash\nautoreply login\n```" + `

This will prompt for your handle and app password securely.

## Alternative

For browser-based OAuth (when client_id is configured):
` + "```bash\nautoreply oauth-login\n```" + `

See: https://docs.bsky.app/docs/advanced-guides/oauth-client
`

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
	}, nil
}
