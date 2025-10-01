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
	// Extract client ID (use default if not provided)
	clientID := "autoreply-cli"
	if clientIDRaw, ok := args["client_id"]; ok {
		if clientIDStr, ok := clientIDRaw.(string); ok && clientIDStr != "" {
			clientID = clientIDStr
		}
	}

	// TODO: Get device auth configuration from BlueSky
	// For now, use placeholder endpoints
	config := &auth.DeviceAuthConfig{
		DeviceAuthorizationEndpoint: "https://bsky.social/oauth/device/code",
		TokenEndpoint:               "https://bsky.social/oauth/token",
		ClientID:                    clientID,
		Scope:                       "atproto transition:generic",
	}

	// Create device auth flow
	flow, err := auth.NewDeviceAuthFlow(config)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to create device auth flow")
	}

	// Request device code
	deviceResp, err := flow.RequestDeviceCode(ctx)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to request device code")
	}

	// Calculate expiration time
	expiresAt := time.Now().Add(time.Duration(deviceResp.ExpiresIn) * time.Second)

	// Format instructions message
	message := fmt.Sprintf("# Device Authorization\n\n"+
		"To authenticate, please visit the following URL in your browser:\n\n"+
		"**%s**\n\n"+
		"And enter this code:\n\n"+
		"## %s\n\n"+
		"Or scan this direct link:\n%s\n\n"+
		"Code expires at: %s\n\n"+
		"Waiting for authorization (polling every %d seconds)...\n",
		deviceResp.VerificationURI,
		deviceResp.UserCode,
		deviceResp.VerificationURIComplete,
		expiresAt.Format("15:04:05"),
		deviceResp.Interval)

	// Poll for token with timeout
	pollCtx, cancel := context.WithTimeout(ctx, time.Duration(deviceResp.ExpiresIn)*time.Second)
	defer cancel()

	creds, err := flow.PollForToken(pollCtx, deviceResp.DeviceCode, deviceResp.Interval)
	if err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to obtain authorization")
	}

	// TODO: Get user handle and DID from token or make additional API call
	// For now, we need to make a call to get the session info
	// This would require using the access token with DPoP

	// Store credentials
	if creds.Handle == "" {
		creds.Handle = "device-user" // Placeholder until we can get real handle
	}
	if err := t.credStore.Save(creds); err != nil {
		return nil, errors.Wrap(err, errors.InternalError, "Failed to store credentials")
	}

	// Set as default handle
	if err := t.credStore.SetDefault(creds.Handle); err != nil {
		fmt.Printf("Warning: Failed to set default handle: %v\n", err)
	}

	// Format success message
	successMsg := fmt.Sprintf("# Device Login Successful\n\n"+
		"Successfully authenticated via device authorization!\n\n"+
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
