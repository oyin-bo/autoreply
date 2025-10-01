// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
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
	// Extract client ID (use default if not provided)
	clientID := "autoreply-cli"
	if clientIDRaw, ok := args["client_id"]; ok {
		if clientIDStr, ok := clientIDRaw.(string); ok && clientIDStr != "" {
			clientID = clientIDStr
		}
	}

	// Extract port (use default if not provided)
	port := 8080
	if portRaw, ok := args["port"]; ok {
		switch v := portRaw.(type) {
		case float64:
			port = int(v)
		case int:
			port = v
		}
	}

	// TODO: Get OAuth configuration from BlueSky
	// For now, use placeholder endpoints
	config := &auth.OAuthConfig{
		AuthorizationEndpoint: "https://bsky.social/oauth/authorize",
		TokenEndpoint:         "https://bsky.social/oauth/token",
		ClientID:              clientID,
		RedirectURI:           auth.GetRedirectURIWithPort(port),
		Scope:                 "atproto transition:generic",
	}

	// Create OAuth flow
	flow, err := auth.NewOAuthFlow(config)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to create OAuth flow")
	}

	// Start callback server
	callbackServer := auth.NewCallbackServer(port)
	if err := callbackServer.Start(); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to start callback server")
	}
	defer func() {
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		callbackServer.Stop(shutdownCtx)
	}()

	// Get authorization URL
	authURL := flow.GetAuthorizationURL()

	// Return instructions to user
	message := fmt.Sprintf("# OAuth Login\n\n"+
		"To authenticate, please visit this URL in your browser:\n\n"+
		"**%s**\n\n"+
		"After authorizing, you will be redirected back and the authentication will complete automatically.\n\n"+
		"Waiting for authorization...\n",
		authURL)

	// For CLI use, we need to wait for the callback
	// For MCP use, we return the URL and wait in background
	// This is a simplified implementation - in production, you'd want to handle this better

	// Wait for callback with timeout
	callbackCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()

	result, err := callbackServer.WaitForCallback(callbackCtx)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to receive OAuth callback")
	}

	if result.Error != "" {
		return nil, errors.NewMCPError(errors.InternalError, fmt.Sprintf("OAuth authorization failed: %s", result.Error))
	}

	// Exchange code for tokens
	creds, err := flow.ExchangeCode(ctx, result.Code, result.State)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to exchange authorization code")
	}

	// TODO: Get user handle and DID from token or make additional API call
	// For now, we need to make a call to get the session info
	// This would require using the access token with DPoP

	// Store credentials
	if creds.Handle == "" {
		creds.Handle = "oauth-user" // Placeholder until we can get real handle
	}
	if err := t.credStore.Save(creds); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to store credentials")
	}

	// Set as default handle
	if err := t.credStore.SetDefault(creds.Handle); err != nil {
		fmt.Printf("Warning: Failed to set default handle: %v\n", err)
	}

	// Format success message
	successMsg := fmt.Sprintf("# OAuth Login Successful\n\n"+
		"Successfully authenticated via OAuth!\n\n"+
		"**Handle:** @%s\n"+
		"**DID:** `%s`\n\n"+
		"Credentials have been securely stored and will be used for authenticated operations.\n",
		creds.Handle, creds.DID)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message + "\n\n" + successMsg,
			},
		},
	}, nil
}
