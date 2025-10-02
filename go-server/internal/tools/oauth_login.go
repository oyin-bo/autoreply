// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// OAuthLoginTool implements the OAuth login tool
type OAuthLoginTool struct {
	credStore *auth.CredentialStore
}

// NewOAuthLoginTool creates a new OAuth login tool
func NewOAuthLoginTool() (*OAuthLoginTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &OAuthLoginTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *OAuthLoginTool) Name() string {
	return "oauth-login"
}

// Description returns the tool description
func (t *OAuthLoginTool) Description() string {
	return "Authenticate with Bluesky using OAuth 2.0 with PKCE and DPoP (most secure)"
}

// InputSchema returns the JSON schema for tool input
func (t *OAuthLoginTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"client_id": {
				Type:        "string",
				Description: "OAuth client ID (optional, uses default if not provided)",
			},
			"port": {
				Type:        "integer",
				Description: "Local callback server port (default: 8080)",
			},
		},
		Required: []string{},
	}
}

// Call executes the OAuth login tool
func (t *OAuthLoginTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Note: This implementation requires a publicly accessible client_id URL
	// For now, return an error with instructions
	message := `# OAuth Login Not Yet Fully Configured

## Implementation Status

The AT Protocol OAuth infrastructure has been implemented per the official specification:

✅ Server metadata discovery (/.well-known endpoints)
✅ Handle and DID resolution (did:plc, did:web)
✅ PAR (Pushed Authorization Request) 
✅ PKCE with S256
✅ DPoP with server nonces
✅ Token exchange with proper verification

## What's Missing

OAuth requires a **publicly accessible client_id URL** where client metadata is hosted.

For example:
- client_id: https://autoreply.example.com/client-metadata.json
- This URL must serve the client metadata JSON
- The URL itself becomes the client_id

## For Now

Use app password authentication:
` + "```bash\nautoreply login\n```" + `

This will prompt for your handle and app password.

## Future Work

To enable OAuth:
1. Host client metadata at a public HTTPS URL
2. Configure the client_id in the OAuth flow
3. Update the tool to use the hosted metadata

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
