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

// AccountsTool implements the accounts management tool
type AccountsTool struct {
	credStore *auth.CredentialStore
}

// NewAccountsTool creates a new accounts tool
func NewAccountsTool() (*AccountsTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &AccountsTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *AccountsTool) Name() string {
	return "accounts"
}

// Description returns the tool description
func (t *AccountsTool) Description() string {
	return "List authenticated accounts and manage default account"
}

// InputSchema returns the JSON schema for tool input
func (t *AccountsTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"action": {
				Type:        "string",
				Description: "Action to perform: 'list' or 'set-default'",
			},
			"handle": {
				Type:        "string",
				Description: "Handle for set-default action",
			},
		},
		Required: []string{},
	}
}

// Call executes the accounts tool
func (t *AccountsTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Default action is list
	action := "list"
	if actionRaw, ok := args["action"]; ok {
		actionStr, ok := actionRaw.(string)
		if !ok {
			return nil, errors.NewMCPError(errors.InvalidInput, "action must be a string")
		}
		action = strings.ToLower(strings.TrimSpace(actionStr))
	}

	switch action {
	case "list":
		return t.listAccounts()
	case "set-default":
		return t.setDefaultAccount(args)
	default:
		return nil, errors.NewMCPError(errors.InvalidInput, fmt.Sprintf("Unknown action: %s", action))
	}
}

// listAccounts lists all authenticated accounts
func (t *AccountsTool) listAccounts() (*mcp.ToolResult, error) {
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

// setDefaultAccount sets the default account
func (t *AccountsTool) setDefaultAccount(args map[string]interface{}) (*mcp.ToolResult, error) {
	handleRaw, ok := args["handle"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "handle parameter is required for set-default action")
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
