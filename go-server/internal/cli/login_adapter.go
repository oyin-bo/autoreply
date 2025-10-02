// Package cli provides command-line interface support for trial mode
package cli

import (
	"context"
	"fmt"
	"os"

	"github.com/oyin-bo/autoreply/go-server/internal/tools"
)

// InteractiveLoginAdapter wraps the LoginTool and adds interactive prompting
type InteractiveLoginAdapter struct {
	tool *tools.LoginTool
}

// NewInteractiveLoginAdapter creates a new interactive login adapter
func NewInteractiveLoginAdapter(tool *tools.LoginTool) *InteractiveLoginAdapter {
	return &InteractiveLoginAdapter{tool: tool}
}

// Execute runs the login tool with interactive prompting if needed
func (a *InteractiveLoginAdapter) Execute(ctx context.Context, args interface{}) (string, error) {
	// Convert args to LoginArgs
	loginArgs, ok := args.(*LoginArgs)
	if !ok {
		return "", fmt.Errorf("invalid arguments type for login")
	}

	handle := loginArgs.Handle
	password := loginArgs.Password

	// Check if we're in an interactive terminal
	// Only prompt if arguments are not provided (not even as empty strings via flags)
	needsPrompt := handle == "" && password == ""

	if needsPrompt {
		// Prompt for handle if not provided
		fmt.Fprintln(os.Stderr, "Bluesky Login")
		fmt.Fprintln(os.Stderr, "")
		
		promptedHandle, err := PromptForInput("Handle: ")
		if err != nil {
			return "", fmt.Errorf("failed to read handle: %w", err)
		}
		if promptedHandle == "" {
			return "", fmt.Errorf("handle cannot be empty")
		}
		handle = promptedHandle

		// Prompt for password
		promptedPassword, err := PromptForPassword("Password: ")
		if err != nil {
			return "", fmt.Errorf("failed to read password: %w", err)
		}
		if promptedPassword == "" {
			return "", fmt.Errorf("password cannot be empty")
		}
		password = promptedPassword
	} else {
		// Validate provided arguments
		if handle == "" {
			return "", fmt.Errorf("handle cannot be empty")
		}
		if password == "" {
			return "", fmt.Errorf("password cannot be empty")
		}
	}

	// Convert to map for tool call
	argsMap := map[string]interface{}{
		"handle":   handle,
		"password": password,
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
