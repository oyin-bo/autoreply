package tools

import (
	"context"
	"fmt"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// AuthStatusTool provides authentication status information
type AuthStatusTool struct {
	cm *auth.CredentialManager
}

// NewAuthStatusTool creates a new auth status tool
func NewAuthStatusTool() *AuthStatusTool {
	cm, err := auth.NewCredentialManager()
	if err != nil {
		// Log error but return tool anyway - it will fail at call time if needed
		fmt.Printf("Warning: failed to create credential manager: %v\n", err)
	}
	
	return &AuthStatusTool{
		cm: cm,
	}
}

// Name returns the tool name
func (t *AuthStatusTool) Name() string {
	return "auth_status"
}

// Description returns the tool description
func (t *AuthStatusTool) Description() string {
	return "Check authentication status and list authenticated accounts"
}

// InputSchema returns the JSON schema for tool input
func (t *AuthStatusTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"handle": {
				Type:        "string",
				Description: "Optional: Check status for specific account handle",
			},
		},
	}
}

// Call executes the auth status tool
func (t *AuthStatusTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Extract optional handle parameter
	var handle *string
	if handleRaw, ok := args["handle"]; ok {
		if handleStr, ok := handleRaw.(string); ok {
			handle = &handleStr
		}
	}
	
	accounts, err := t.cm.ListAccounts(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to list accounts: %w", err)
	}
	
	defaultAccount, err := t.cm.GetDefaultAccount(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get default account: %w", err)
	}
	
	// If specific handle requested, filter to that account
	if handle != nil {
		filtered := []auth.Account{}
		for _, acc := range accounts {
			if acc.Handle == *handle {
				filtered = append(filtered, acc)
				break
			}
		}
		accounts = filtered
	}
	
	// Format as markdown for text content
	text := "# Authentication Status\n\n"
	if len(accounts) == 0 {
		text += "No authenticated accounts found.\n"
		text += "\nRun `autoreply login` to authenticate.\n"
	} else {
		text += fmt.Sprintf("**Authenticated Accounts:** %d\n\n", len(accounts))
		for _, acc := range accounts {
			marker := " "
			isDefault := defaultAccount != nil && *defaultAccount == acc.Handle
			if isDefault {
				marker = "✓"
			}
			text += fmt.Sprintf("%s **@%s**\n", marker, acc.Handle)
			if acc.DID != "" {
				text += fmt.Sprintf("  - DID: `%s`\n", acc.DID)
			}
			if acc.PDS != "" {
				text += fmt.Sprintf("  - PDS: `%s`\n", acc.PDS)
			}
			text += fmt.Sprintf("  - Created: %s\n", acc.CreatedAt.Format("2006-01-02T15:04:05Z07:00"))
			text += fmt.Sprintf("  - Last used: %s\n", acc.LastUsed.Format("2006-01-02T15:04:05Z07:00"))
			if isDefault {
				text += "  - _(default)_\n"
			}
			text += "\n"
		}
		
		if defaultAccount != nil {
			text += fmt.Sprintf("**Default Account:** @%s\n", *defaultAccount)
		}
	}
	
	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: text,
			},
		},
	}, nil
}
