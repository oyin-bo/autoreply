// Package cli provides command-line interface support for trial mode
package cli

import (
	"context"
	"fmt"
	"os"

	"github.com/oyin-bo/autoreply/go-server/internal/tools"
)

// InteractiveOAuthLoginAdapter wraps the OAuthLoginTool and adds interactive prompting
type InteractiveOAuthLoginAdapter struct {
	tool *tools.OAuthLoginTool
}

// NewInteractiveOAuthLoginAdapter creates a new interactive OAuth login adapter
func NewInteractiveOAuthLoginAdapter(tool *tools.OAuthLoginTool) *InteractiveOAuthLoginAdapter {
	return &InteractiveOAuthLoginAdapter{tool: tool}
}

// Execute runs the OAuth login tool with interactive prompting if needed
func (a *InteractiveOAuthLoginAdapter) Execute(ctx context.Context, args interface{}) (string, error) {
	// Convert args to OAuthLoginArgs
	oauthArgs, ok := args.(*OAuthLoginArgs)
	if !ok {
		return "", fmt.Errorf("invalid arguments type for oauth-login")
	}

	handle := oauthArgs.Handle
	port := oauthArgs.Port
	if port == 0 {
		port = 8080
	}

	// Check if we're in an interactive terminal
	needsPrompt := handle == ""

	if needsPrompt {
		// Prompt for handle if not provided
		fmt.Fprintln(os.Stderr, "Bluesky OAuth Login")
		fmt.Fprintln(os.Stderr, "")
		
		promptedHandle, err := PromptForInput("Handle: ")
		if err != nil {
			return "", fmt.Errorf("failed to read handle: %w", err)
		}
		if promptedHandle == "" {
			return "", fmt.Errorf("handle cannot be empty")
		}
		handle = promptedHandle
	} else {
		// Validate provided arguments
		if handle == "" {
			return "", fmt.Errorf("handle cannot be empty")
		}
	}

	// Convert to map for tool call
	argsMap := map[string]interface{}{
		"handle": handle,
		"port":   float64(port),
	}

	// Call the MCP tool
	result, err := a.tool.Call(ctx, argsMap)
	if err != nil {
		return "", err
	}

	// Extract text content from result
	if len(result.Content) > 0 && result.Content[0].Type == "text" {
		return result.Content[0].Text, nil
	}

	return "", nil
}
