// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"crypto/rand"
	"encoding/hex"
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
	}
}

// Call executes the login tool - tries OAuth first, falls back to app password
func (t *LoginTool) Call(ctx context.Context, args map[string]interface{}, server *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate handle parameter
	handleRaw, ok := args["handle"]
	// Handle may be omitted for elicitation - treat missing differently from empty string
	var handle string
	hasHandle := false
	if ok {
		if hs, ok := handleRaw.(string); ok {
			handle = strings.TrimSpace(strings.TrimPrefix(hs, "@"))
			hasHandle = true
		}
	}

	// Check if we're in CLI mode (server is nil) - use legacy input_text pattern
	// In CLI mode, we still use prompt_id for backward compatibility
	isCliMode := server == nil

	// Check if password parameter is present (forces app password mode)
	// If the key exists in args, even with empty value, we use app password mode
	passwordRaw, hasPassword := args["password"]
	if hasPassword {
		// Password parameter present - use app password authentication
		password := ""
		if passwordStr, ok := passwordRaw.(string); ok {
			password = strings.TrimSpace(passwordStr)
		}

		// If handle is missing
		if !hasHandle || handle == "" {
			if isCliMode {
				// CLI mode - use legacy input_text pattern
				promptID := generatePromptID()
				metadata := fmt.Sprintf(`{"prompt_id":"%s","field":"handle","message":"Enter Bluesky handle (e.g., alice.bsky.social)"}`, promptID)
				return &mcp.ToolResult{
					Content: []mcp.ContentItem{{
						Type:     "input_text",
						Text:     "Enter Bluesky handle (e.g., alice.bsky.social)",
						Metadata: []byte(metadata),
					}},
				}, nil
			}
			if server.SupportsElicitation() {
				// Use standard MCP elicitation/create
				schema := map[string]interface{}{
					"type": "object",
					"properties": map[string]interface{}{
						"handle": map[string]interface{}{
							"type":        "string",
							"description": "Your BlueSky handle (e.g., user.bsky.social)",
						},
					},
					"required": []string{"handle"},
				}
				resp, err := server.RequestElicitation(ctx, "Please provide your BlueSky handle", schema)
				if err != nil {
					// Elicitation transport not available - fall back to error
					return createElicitationUnavailableError(server, "handle"), nil
				}
				if resp.Action != "accept" {
					return &mcp.ToolResult{
						Content: []mcp.ContentItem{{Type: "text", Text: "Login cancelled"}},
					}, nil
				}
				if h, ok := resp.Content["handle"].(string); ok {
					handle = h
					hasHandle = true
				}
			} else {
				// Client doesn't support elicitation - return error with guidance
				return createElicitationUnavailableError(server, "handle"), nil
			}
		}

		// If password is empty
		if password == "" {
			if isCliMode {
				// CLI mode - use legacy input_text pattern
				promptID := generatePromptID()
				metadata := fmt.Sprintf(`{"prompt_id":"%s","field":"password","message":"App password for @%s"}`, promptID, handle)
				return &mcp.ToolResult{
					Content: []mcp.ContentItem{{
						Type:     "input_text",
						Text:     fmt.Sprintf("App password for @%s", handle),
						Metadata: []byte(metadata),
					}},
				}, nil
			}
			if server.SupportsElicitation() {
				schema := map[string]interface{}{
					"type": "object",
					"properties": map[string]interface{}{
						"password": map[string]interface{}{
							"type":        "string",
							"description": "BlueSky app password (create at https://bsky.app/settings/app-passwords)",
						},
					},
					"required": []string{"password"},
				}
				message := fmt.Sprintf(`Please provide a BlueSky app password for @%s (NOT your main password).

Create an app password at: https://bsky.app/settings/app-passwords

Alternatively, cancel and use OAuth authentication instead.`, handle)

				resp, err := server.RequestElicitation(ctx, message, schema)
				if err != nil {
					return createPasswordElicitationUnavailableError(server, handle), nil
				}
				if resp.Action == "cancel" {
					return &mcp.ToolResult{
						Content: []mcp.ContentItem{{
							Type: "text",
							Text: fmt.Sprintf("Login cancelled. To use OAuth, call login with handle=%s and omit the password parameter.", handle),
						}},
					}, nil
				}
				if resp.Action != "accept" {
					return &mcp.ToolResult{
						Content: []mcp.ContentItem{{Type: "text", Text: "Login declined"}},
					}, nil
				}
				if p, ok := resp.Content["password"].(string); ok {
					password = p
				}
			} else {
				return createPasswordElicitationUnavailableError(server, handle), nil
			}
		}

		return t.loginWithPassword(ctx, handle, password)
	}

	// If handle is missing
	if !hasHandle || handle == "" {
		if isCliMode {
			// CLI mode - use legacy input_text pattern
			promptID := generatePromptID()
			metadata := fmt.Sprintf(`{"prompt_id":"%s","field":"handle","message":"Enter Bluesky handle (e.g., alice.bsky.social)"}`, promptID)
			return &mcp.ToolResult{
				Content: []mcp.ContentItem{{
					Type:     "input_text",
					Text:     "Enter Bluesky handle (e.g., alice.bsky.social)",
					Metadata: []byte(metadata),
				}},
			}, nil
		}
		if server.SupportsElicitation() {
			schema := map[string]interface{}{
				"type": "object",
				"properties": map[string]interface{}{
					"handle": map[string]interface{}{
						"type":        "string",
						"description": "Your BlueSky handle (e.g., user.bsky.social)",
					},
				},
				"required": []string{"handle"},
			}
			resp, err := server.RequestElicitation(ctx, "Please provide your BlueSky handle", schema)
			if err != nil {
				return createElicitationUnavailableError(server, "handle"), nil
			}
			if resp.Action != "accept" {
				return &mcp.ToolResult{
					Content: []mcp.ContentItem{{Type: "text", Text: "Login cancelled"}},
				}, nil
			}
			if h, ok := resp.Content["handle"].(string); ok {
				handle = h
				hasHandle = true
			}
		} else {
			return createElicitationUnavailableError(server, "handle"), nil
		}
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

// createElicitationUnavailableError creates the standard error for when elicitation is needed but unavailable
func createElicitationUnavailableError(server *mcp.Server, field string) *mcp.ToolResult {
	clientName := server.GetClientName()
	message := fmt.Sprintf(`# Login requires %s

**%s does not support interactive prompts** (MCP elicitation).

To complete login, please:

1. **Use OAuth (recommended):** Call login with your handle:
   {"handle": "your.handle.bsky.social"}

2. **Or provide credentials up-front:** Call login with both handle and password:
   {"handle": "your.handle.bsky.social", "password": "your-app-password"}

**Security Note:** Do NOT use your main BlueSky password. Create an app password at:
https://bsky.app/settings/app-passwords
`, field, clientName)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{{
			Type: "text",
			Text: message,
		}},
		IsError: true,
	}
}

// createPasswordElicitationUnavailableError creates the error for when password elicitation is unavailable
func createPasswordElicitationUnavailableError(server *mcp.Server, handle string) *mcp.ToolResult {
	clientName := server.GetClientName()
	message := fmt.Sprintf(`# Password required for @%s

**%s does not support interactive prompts** (MCP elicitation).

Please choose one of these options:

1. **Use OAuth (strongly recommended):** Call login without password parameter:
   {"handle": "%s"}

2. **Provide app password up-front:** Call login with password:
   {"handle": "%s", "password": "your-app-password"}

**IMPORTANT Security Warning:**
- Do NOT use your main BlueSky account password
- Create an app password at: https://bsky.app/settings/app-passwords
- OAuth is the most secure option and is strongly preferred
`, handle, clientName, handle, handle)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{{
			Type: "text",
			Text: message,
		}},
		IsError: true,
	}
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

// generatePromptID returns a short opaque identifier similar to the Rust implementation
// (16 random bytes, hex-encoded -> 32 chars). This is used when MCP elicitation needs
// an id to correlate prompts and responses.
func generatePromptID() string {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		// fallback to a simple timestamp-based id if crypto rand fails (very unlikely)
		return fmt.Sprintf("fallback-%d", os.Getpid())
	}
	return hex.EncodeToString(b)
}
