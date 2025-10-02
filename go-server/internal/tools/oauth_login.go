// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"os"

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
			"handle": {
				Type:        "string",
				Description: "Bluesky handle (e.g. alice.bsky.social)",
			},
			"port": {
				Type:        "integer",
				Description: "Local callback server port (default: 8080)",
			},
		},
		Required: []string{"handle"},
	}
}

// Call executes the OAuth login tool
func (t *OAuthLoginTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Get port (default 8080)
	port := 8080
	if portVal, ok := args["port"]; ok {
		if portFloat, ok := portVal.(float64); ok {
			port = int(portFloat)
		}
	}

	// Get handle for login hint
	handle := ""
	if handleVal, ok := args["handle"]; ok {
		if handleStr, ok := handleVal.(string); ok {
			handle = handleStr
		}
	}

	// If no handle provided, ask user
	if handle == "" {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: "Error: handle is required for OAuth login. Please provide a handle (e.g., alice.bsky.social)",
				},
			},
			IsError: true,
		}, nil
	}

	// Setup redirect URI based on port
	redirectURI := fmt.Sprintf("http://127.0.0.1:%d/callback", port)

	// Discover server metadata from handle
	metadata, err := auth.DiscoverServerMetadataFromHandle(ctx, handle)
	if err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: fmt.Sprintf("Error: Failed to discover OAuth server for %s: %v", handle, err),
				},
			},
			IsError: true,
		}, nil
	}

	// Setup OAuth config with loopback client
	config := &auth.OAuthConfig{
		ClientID:       redirectURI, // Use loopback URI as client_id for native apps
		RedirectURI:    redirectURI,
		Scope:          "atproto transition:generic",
		ServerMetadata: metadata,
	}

	// Create OAuth flow
	flow, err := auth.NewOAuthFlow(config)
	if err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: fmt.Sprintf("Error: Failed to initialize OAuth flow: %v", err),
				},
			},
			IsError: true,
		}, nil
	}

	// Push authorization request (PAR)
	requestURI, err := flow.PushAuthorizationRequest(ctx, handle)
	if err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: fmt.Sprintf("Error: Failed to push authorization request: %v", err),
				},
			},
			IsError: true,
		}, nil
	}

	// Get authorization URL
	authURL := flow.GetAuthorizationURL(requestURI)

	// Start callback server and wait for authorization
	callbackServer := auth.NewCallbackServer(port)
	if err := callbackServer.Start(); err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: fmt.Sprintf("Error: Failed to start callback server: %v", err),
				},
			},
			IsError: true,
		}, nil
	}
	defer callbackServer.Stop(ctx)
	
	result := fmt.Sprintf(`
OAuth Login Initiated

1. Open this URL in your browser:
   %s

2. Authorize the application in your browser

3. Waiting for authorization callback on http://127.0.0.1:%d/callback

The server will automatically receive the authorization code and complete the login.
`, authURL, port)

	// Print to stderr for CLI users
	fmt.Fprint(os.Stderr, result)

	// Wait for callback result
	callbackResult, err := callbackServer.WaitForCallback(ctx)
	if err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: result + fmt.Sprintf("\n\nError: Authorization failed: %v", err),
				},
			},
			IsError: true,
		}, nil
	}

	if callbackResult.Error != "" {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: result + fmt.Sprintf("\n\nError: Authorization failed: %s", callbackResult.Error),
				},
			},
			IsError: true,
		}, nil
	}

	// Exchange code for tokens
	creds, err := flow.ExchangeCode(ctx, callbackResult.Code, callbackResult.State)
	if err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: result + fmt.Sprintf("\n\nError: Token exchange failed: %v", err),
				},
			},
			IsError: true,
		}, nil
	}

	// Resolve DID to handle
	identity, err := auth.ResolveDID(ctx, creds.DID)
	if err != nil {
		// Store with DID only
		creds.Handle = creds.DID
	} else {
		creds.Handle = auth.ExtractHandleFromDID(identity)
		if creds.Handle == "" {
			creds.Handle = creds.DID
		}
	}

	// Store credentials
	if err := t.credStore.Save(creds); err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: result + fmt.Sprintf("\n\nError: Failed to store credentials: %v", err),
				},
			},
			IsError: true,
		}, nil
	}

	// Set as default
	if err := t.credStore.SetDefault(creds.Handle); err != nil {
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: result + fmt.Sprintf("\n\nWarning: Failed to set default handle: %v", err),
				},
			},
		}, nil
	}

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: fmt.Sprintf("Successfully authenticated as @%s using OAuth 2.0!\n\nCredentials stored securely. Access token expires at: %s",
					creds.Handle, creds.ExpiresAt.Format("2006-01-02 15:04:05")),
			},
		},
	}, nil
}
