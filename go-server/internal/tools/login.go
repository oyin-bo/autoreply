// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"log"
	"strings"
	"time"

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
	return "Authenticate accounts and manage stored credentials. Supports subcommands: list (show accounts), default (set default account), delete (remove credentials), or omit command for login."
}

// InputSchema returns the JSON schema for tool input
func (t *LoginTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"command": {
				Type:        "string",
				Description: "Subcommand: 'list' (show accounts), 'default' (set default), 'delete' (remove credentials), or omit for login",
			},
			"handle": {
				Type:        "string",
				Description: "Bluesky handle (e.g., alice.bsky.social) - optional for OAuth (allows account selection in browser). Required for app password authentication and 'default' subcommand.",
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

// Call executes the login tool - dispatches to subcommands or performs login
func (t *LoginTool) Call(ctx context.Context, args map[string]interface{}, server *mcp.Server) (*mcp.ToolResult, error) {
	// Check for subcommand
	commandRaw, hasCommand := args["command"]
	if hasCommand {
		commandStr, ok := commandRaw.(string)
		if !ok {
			return nil, errors.NewMCPError(errors.InvalidInput, "command must be a string")
		}
		command := strings.ToLower(strings.TrimSpace(commandStr))

		switch command {
		case "list":
			return t.handleList()
		case "default":
			return t.handleDefault(args)
		case "delete":
			return t.handleDelete(args)
		case "":
			// Empty command - proceed with login
			break
		default:
			return nil, errors.NewMCPError(errors.InvalidInput, fmt.Sprintf("Unknown command: %s. Valid commands: list, default, delete", command))
		}
	}

	// No subcommand (or empty) - proceed with login
	return t.performLogin(ctx, args, server)
}

// handleList lists all authenticated accounts
func (t *LoginTool) handleList() (*mcp.ToolResult, error) {
	handles, err := t.credStore.ListHandles()
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to list accounts")
	}

	defaultHandle, _ := t.credStore.GetDefault()

	var message strings.Builder
	message.WriteString("# Authenticated Accounts\n\n")

	if len(handles) == 0 {
		message.WriteString("No authenticated accounts found.\n\n")
		message.WriteString("Use `login` to authenticate with a Bluesky account.\n")
	} else {
		message.WriteString(fmt.Sprintf("Found %d authenticated account(s):\n\n", len(handles)))
		for _, handle := range handles {
			if handle == defaultHandle {
				message.WriteString(fmt.Sprintf("- **@%s** *(default)*\n", handle))
			} else {
				message.WriteString(fmt.Sprintf("- @%s\n", handle))
			}
		}
		message.WriteString("\n")
		if defaultHandle != "" {
			message.WriteString(fmt.Sprintf("Default account: **@%s**\n", defaultHandle))
		}
	}

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message.String(),
			},
		},
	}, nil
}

// handleDefault sets the default account
func (t *LoginTool) handleDefault(args map[string]interface{}) (*mcp.ToolResult, error) {
	handleRaw, ok := args["handle"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle parameter is required for 'default' command")
	}

	handle, ok := handleRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle must be a string")
	}

	handle = strings.TrimSpace(strings.TrimPrefix(handle, "@"))
	if handle == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle cannot be empty")
	}

	// Verify the account exists
	_, err := t.credStore.Load(handle)
	if err != nil {
		return nil, errors.Wrap(err, errors.NotFound, "Account not found. Please login first.")
	}

	// Set as default
	if err := t.credStore.SetDefault(handle); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to set default account")
	}

	message := fmt.Sprintf("# Default Account Updated\n\n"+
		"Default account set to **@%s**\n",
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

// handleDelete removes stored credentials for an account
func (t *LoginTool) handleDelete(args map[string]interface{}) (*mcp.ToolResult, error) {
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
	message := fmt.Sprintf("# Credentials Removed\n\n"+
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

// performLogin executes the login flow - tries OAuth first, falls back to app password
func (t *LoginTool) performLogin(ctx context.Context, args map[string]interface{}, server *mcp.Server) (*mcp.ToolResult, error) {
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

	// Determine execution context (unused now that CLI handles prompts)

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
				// No elicitation support (or CLI mode): return guidance with isError
				return createElicitationUnavailableError(server, "handle"), nil
			}
		}

		// If password is empty
		if password == "" {
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
				// No elicitation support (or CLI mode): return guidance with isError
				return createPasswordElicitationUnavailableError(server, handle), nil
			}
		}

		return t.loginWithPassword(ctx, handle, password)
	}

	// OAuth mode - handle is optional
	// If handle is empty, we'll use default service and allow account selection
	// If handle is provided, we'll discover its PDS and pass it as login_hint

	// OAuth works in CLI mode even without handle - browser will open for account selection
	// No need to check for elicitation support here

	// Try OAuth (handle can be empty for account selection in browser)
	port := 8080
	if portVal, ok := args["port"]; ok {
		if portFloat, ok := portVal.(float64); ok {
			port = int(portFloat)
		}
	}

	// Attempt OAuth login (handle can be empty for account selection)
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

1. **Use OAuth (recommended):** Call login without handle to select account in browser:
   {}
   
   Or with a specific handle:
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

	// Store session (so post/react tools can reuse access token)
	session := &auth.Session{
		Handle:       creds.Handle,
		AccessToken:  creds.AccessToken,
		RefreshToken: creds.RefreshToken,
		DID:          creds.DID,
		ExpiresAt:    creds.ExpiresAt,
	}
	if err := t.credStore.SaveSession(session); err != nil {
		// Non-fatal - just log
		fmt.Printf("Warning: Failed to store session: %v\n", err)
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
// handle can be empty - if empty, uses default bsky.social service and allows account selection
func (t *LoginTool) loginWithOAuth(ctx context.Context, handle string, port int) (*mcp.ToolResult, error) {
	// Setup redirect URI based on port
	// Per AT Protocol OAuth spec for localhost development:
	// - redirect_uri is "http://127.0.0.1:PORT" (root path only, port varies)
	redirectURI := fmt.Sprintf("http://127.0.0.1:%d", port)

	var metadata *auth.AuthorizationServerMetadata
	var err error

	// Validate handle format: must contain a dot (e.g., "user.bsky.social")
	// If handle is provided but invalid, treat it like empty and use default OAuth
	validHandle := handle
	if handle != "" && !strings.Contains(handle, ".") {
		log.Printf("Handle '%s' is invalid (no domain), using default OAuth flow with account selection", handle)
		validHandle = ""
	}

	// Discover server metadata
	if validHandle == "" {
		// No handle provided (or invalid) - use default bsky.social entryway
		// Note: bsky.social is an entryway, not a PDS, so we discover directly from the issuer
		// This allows user to select any account during OAuth
		metadata, err = auth.DiscoverServerMetadataFromIssuer(ctx, "https://bsky.social")
		if err != nil {
			return nil, fmt.Errorf("failed to discover OAuth server for default entryway: %w", err)
		}
	} else {
		// Handle provided - discover from handle and use it as login_hint
		metadata, err = auth.DiscoverServerMetadataFromHandle(ctx, validHandle)
		if err != nil {
			return nil, fmt.Errorf("failed to discover OAuth server for %s: %w", validHandle, err)
		}
	}

	// Setup OAuth config with localhost development client
	// Per AT Protocol OAuth spec for localhost development:
	// - client_id is just "http://localhost" (no query params, no port)
	// - redirect_uri is "http://127.0.0.1:PORT" (root path only, port varies)
	// - scope for localhost is just "atproto" (transition scopes not allowed)
	redirectURI = fmt.Sprintf("http://127.0.0.1:%d", port)

	config := &auth.OAuthConfig{
		ClientID:       "http://localhost",
		RedirectURI:    redirectURI,
		Scope:          "atproto",
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

	// Launch background goroutine to wait for callback and finish the flow
	go func() {
		// Use a new background context for the long-running task
		bgCtx := context.Background()

		// Wait for callback with timeout
		timeoutCtx, cancel := context.WithTimeout(bgCtx, 5*time.Minute)
		defer cancel()

		callbackResult, err := callbackServer.WaitForCallback(timeoutCtx)
		if err != nil {
			log.Printf("OAuth background error: failed to wait for callback: %v", err)
			shutdownCtx, shutdownCancel := context.WithTimeout(bgCtx, 2*time.Second)
			defer shutdownCancel()
			callbackServer.Stop(shutdownCtx)
			return
		}

		// Shutdown the callback server
		shutdownCtx, shutdownCancel := context.WithTimeout(bgCtx, 2*time.Second)
		defer shutdownCancel()
		callbackServer.Stop(shutdownCtx)

		if callbackResult.Error != "" {
			log.Printf("OAuth background error: authorization failed: %s", callbackResult.Error)
			return
		}

		// Exchange code for tokens
		creds, err := flow.ExchangeCode(bgCtx, callbackResult.Code, callbackResult.State)
		if err != nil {
			log.Printf("OAuth background error: token exchange failed: %v", err)
			return
		}

		// Resolve DID to handle
		identity, err := auth.ResolveDID(bgCtx, creds.DID)
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
			log.Printf("OAuth background error: failed to store credentials: %v", err)
			return
		}

		// Store session for OAuth (so post/react tools can reuse access token)
		session := &auth.Session{
			Handle:       creds.Handle,
			AccessToken:  creds.AccessToken,
			RefreshToken: creds.RefreshToken,
			DID:          creds.DID,
			ExpiresAt:    creds.ExpiresAt,
		}
		if err := t.credStore.SaveSession(session); err != nil {
			log.Printf("OAuth background warning: failed to store session: %v", err)
			// Not fatal - credentials are stored, session can be recreated
		}

		// Set as default
		if err := t.credStore.SetDefault(creds.Handle); err != nil {
			log.Printf("OAuth background warning: failed to set default handle: %v", err)
		}

		log.Printf("OAuth background success: authenticated as @%s", creds.Handle)
	}()

	// Immediately return the markdown message with the authorization URL
	message := fmt.Sprintf(
		"# OAuth Login Initiated\n\n1. Open this URL in your browser:\n   %s\n\n2. Authorize the application.\n\nWaiting for authorization on port %d...",
		authURL, port,
	)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
	}, nil
}

// Note: CLI mode no longer uses legacy input_text prompts or prompt identifiers.
// Interactive prompting is handled entirely by the CLI adapter before invoking this tool.
