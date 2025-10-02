// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"net/url"
	"os"
	"strings"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// LoginTool implements the login tool with OAuth as default and app password as fallback
type LoginTool struct {
	sessionManager *auth.SessionManager
	credStore      *auth.CredentialStore
}

// NewLoginTool creates a new login tool
func NewLoginTool() (*LoginTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &LoginTool{
		sessionManager: auth.NewSessionManager(),
		credStore:      credStore,
	}, nil
}

// Name returns the tool name
func (t *LoginTool) Name() string {
	return "login"
}

// Description returns the tool description
func (t *LoginTool) Description() string {
	return "Authenticate with Bluesky. Uses OAuth 2.0 by default (most secure), or app password if specified or OAuth fails."
}

// InputSchema returns the JSON schema for tool input
func (t *LoginTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"handle": {
				Type:        "string",
				Description: "Bluesky handle (e.g., alice.bsky.social)",
			},
			"password": {
				Type:        "string",
				Description: "App password (generated in Bluesky settings). If this parameter is present (even empty), skips OAuth and uses app password authentication. Omit this parameter to use OAuth.",
			},
			"port": {
				Type:        "integer",
				Description: "Local callback server port for OAuth (default: 8080)",
			},
		},
		Required: []string{"handle"},
	}
}

// Call executes the login tool - tries OAuth first, falls back to app password
func (t *LoginTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Extract and validate handle parameter
	handleRaw, ok := args["handle"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle parameter is required")
	}

	handle, ok := handleRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle must be a string")
	}

	handle = strings.TrimSpace(strings.TrimPrefix(handle, "@"))
	if handle == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle cannot be empty")
	}

	// Check if password parameter is present (forces app password mode)
	// If the key exists in args, even with empty value, we use app password mode
	passwordRaw, hasPassword := args["password"]
	if hasPassword {
		// Password parameter present - use app password authentication
		password := ""
		if passwordStr, ok := passwordRaw.(string); ok {
			password = strings.TrimSpace(passwordStr)
		}
		return t.loginWithPassword(ctx, handle, password)
	}

	// Otherwise, try OAuth first
	port := 8080
	if portVal, ok := args["port"]; ok {
		if portFloat, ok := portVal.(float64); ok {
			port = int(portFloat)
		}
	}

	// Attempt OAuth login
	result, err := t.loginWithOAuth(ctx, handle, port)
	if err != nil {
		// OAuth failed - provide helpful error message
		return &mcp.ToolResult{
			Content: []mcp.ContentItem{
				{
					Type: "text",
					Text: fmt.Sprintf("OAuth authentication failed: %v\n\nTo use app password authentication instead, provide the password parameter.", err),
				},
			},
			IsError: true,
		}, nil
	}

	return result, nil
}

// loginWithPassword performs app password authentication
func (t *LoginTool) loginWithPassword(ctx context.Context, handle, password string) (*mcp.ToolResult, error) {
	if strings.TrimSpace(password) == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "password cannot be empty")
	}

	// Create session with AT Protocol
	creds, err := t.sessionManager.CreateSession(ctx, handle, password)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to create session")
	}

	// Store credentials securely
	if err := t.credStore.Save(creds); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to store credentials")
	}

	// Set as default handle
	if err := t.credStore.SetDefault(creds.Handle); err != nil {
		// Non-fatal - just log
		fmt.Printf("Warning: Failed to set default handle: %v\n", err)
	}

	// Format success message
	message := fmt.Sprintf("# Login Successful (App Password)\n\n"+
		"Successfully authenticated as **@%s**\n\n"+
		"**DID:** `%s`\n\n"+
		"Credentials have been securely stored and will be used for authenticated operations.\n",
		creds.Handle, creds.DID)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
	}, nil
}

// loginWithOAuth performs OAuth authentication
func (t *LoginTool) loginWithOAuth(ctx context.Context, handle string, port int) (*mcp.ToolResult, error) {
	// Setup redirect URI based on port
	redirectURI := fmt.Sprintf("http://127.0.0.1:%d/callback", port)

	// Discover server metadata from handle
	metadata, err := auth.DiscoverServerMetadataFromHandle(ctx, handle)
	if err != nil {
		return nil, fmt.Errorf("failed to discover OAuth server for %s: %w", handle, err)
	}

	// Setup OAuth config with localhost development client
	// Per AT Protocol OAuth spec, use http://localhost with redirect_uri in query param
	clientID := fmt.Sprintf("http://localhost?redirect_uri=%s&scope=atproto%%20transition:generic",
		url.QueryEscape(redirectURI))

	config := &auth.OAuthConfig{
		ClientID:       clientID,
		RedirectURI:    redirectURI,
		Scope:          "atproto transition:generic",
		ServerMetadata: metadata,
	}

	// Create OAuth flow
	flow, err := auth.NewOAuthFlow(config)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize OAuth flow: %w", err)
	}

	// Push authorization request (PAR)
	requestURI, err := flow.PushAuthorizationRequest(ctx, handle)
	if err != nil {
		return nil, fmt.Errorf("failed to push authorization request: %w", err)
	}

	// Get authorization URL
	authURL := flow.GetAuthorizationURL(requestURI)

	// Start callback server and wait for authorization
	callbackServer := auth.NewCallbackServer(port)
	if err := callbackServer.Start(); err != nil {
		return nil, fmt.Errorf("failed to start callback server: %w", err)
	}
	defer callbackServer.Stop(ctx)

	message := fmt.Sprintf(`
# OAuth Login Initiated

1. Open this URL in your browser:
   %s

2. Authorize the application in your browser

3. Waiting for authorization callback on http://127.0.0.1:%d/callback

The server will automatically receive the authorization code and complete the login.
`, authURL, port)

	// Print to stderr for CLI users
	fmt.Fprint(os.Stderr, message)

	// Wait for callback result
	callbackResult, err := callbackServer.WaitForCallback(ctx)
	if err != nil {
		return nil, fmt.Errorf("authorization failed: %w", err)
	}

	if callbackResult.Error != "" {
		return nil, fmt.Errorf("authorization failed: %s", callbackResult.Error)
	}

	// Exchange code for tokens
	creds, err := flow.ExchangeCode(ctx, callbackResult.Code, callbackResult.State)
	if err != nil {
		return nil, fmt.Errorf("token exchange failed: %w", err)
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
		return nil, fmt.Errorf("failed to store credentials: %w", err)
	}

	// Set as default
	if err := t.credStore.SetDefault(creds.Handle); err != nil {
		fmt.Printf("Warning: Failed to set default handle: %v\n", err)
	}

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: fmt.Sprintf("# Login Successful (OAuth 2.0)\n\n"+
					"Successfully authenticated as **@%s**\n\n"+
					"**DID:** `%s`\n\n"+
					"Access token expires at: %s\n\n"+
					"Credentials stored securely.",
					creds.Handle, creds.DID, creds.ExpiresAt.Format("2006-01-02 15:04:05")),
			},
		},
	}, nil
}
