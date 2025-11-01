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

// Execute runs the login tool with interactive prompting if needed and handles subcommands
func (a *InteractiveLoginAdapter) Execute(ctx context.Context, args interface{}) (string, error) {
	// Convert args to LoginArgs
	loginArgs, ok := args.(*LoginArgs)
	if !ok {
		return "", fmt.Errorf("invalid arguments type for login")
	}

	// Build args map for tool call
	argsMap := map[string]interface{}{}

	// Check for subcommand
	if loginArgs.Command != "" {
		argsMap["command"] = loginArgs.Command

		// For 'default' command, handle is required
		if loginArgs.Command == "default" {
			if loginArgs.Handle == "" {
				return "", fmt.Errorf("handle is required for 'default' command")
			}
			argsMap["handle"] = loginArgs.Handle
		}

		// For 'delete' command, handle is optional (uses default if not provided)
		if loginArgs.Command == "delete" && loginArgs.Handle != "" {
			argsMap["handle"] = loginArgs.Handle
		}

		// For 'list' command, no additional parameters needed

		// Call the tool directly for subcommands
		result, err := a.tool.Call(ctx, argsMap, nil)
		if err != nil {
			return "", err
		}

		// Extract text content from result
		if len(result.Content) > 0 && result.Content[0].Type == "text" {
			return result.Content[0].Text, nil
		}
		return "", nil
	}

	// No subcommand - proceed with login flow
	handle := loginArgs.Handle
	password := loginArgs.Password
	port := loginArgs.Port
	if port == 0 {
		port = 8080
	}

	// Determine if password flag was provided
	// In CLI, if --password or -p is used (even without value), we want app password mode
	passwordFlagProvided := loginArgs.Password != "" || isPasswordFlagInArgs()

	// Check if we're in an interactive terminal
	// For OAuth, handle is optional - user can select account in browser
	// Only prompt for handle if password mode is being used
	needsPromptForHandle := handle == "" && passwordFlagProvided

	if needsPromptForHandle {
		// Prompt for handle if not provided and using password mode
		fmt.Fprintln(os.Stderr, "Bluesky Login (App Password Mode)")
		fmt.Fprintln(os.Stderr, "")

		promptedHandle, err := PromptForInput("Handle: ")
		if err != nil {
			return "", fmt.Errorf("failed to read handle: %w", err)
		}
		if promptedHandle == "" {
			return "", fmt.Errorf("handle is required for app password authentication")
		}
		handle = promptedHandle

		// If password flag was provided but empty, prompt for it now
		if password == "" {
			fmt.Fprintln(os.Stderr, "")
			promptedPassword, err := PromptForPassword("App Password: ")
			if err != nil {
				return "", fmt.Errorf("failed to read password: %w", err)
			}
			if promptedPassword == "" {
				return "", fmt.Errorf("password cannot be empty")
			}
			password = promptedPassword
		}
	} else if passwordFlagProvided && password == "" {
		// Password mode but no handle prompt needed - still need to prompt for password
		fmt.Fprintln(os.Stderr, "")
		promptedPassword, err := PromptForPassword("App Password: ")
		if err != nil {
			return "", fmt.Errorf("failed to read password: %w", err)
		}
		if promptedPassword == "" {
			return "", fmt.Errorf("password cannot be empty")
		}
		password = promptedPassword
	}

	// Add handle to args if provided (can be empty for OAuth account selection)
	if handle != "" {
		argsMap["handle"] = handle
	}
	argsMap["port"] = float64(port)

	// Note: CLI no longer uses tool-driven elicitation via input_text metadata.
	// All interactive prompts are handled here before calling the tool.

	// If password flag was provided, include password parameter (even if empty)
	// This signals to use app password mode
	if passwordFlagProvided {
		argsMap["password"] = password
	}

	// Call the MCP tool (CLI mode doesn't have a server context, pass nil)
	result, err := a.tool.Call(ctx, argsMap, nil)
	if err != nil {
		return "", err
	}

	// Tool no longer emits input_text prompts; all prompting done above.

	// If OAuth failed and we're in interactive mode, offer to try app password
	if result.IsError && !passwordFlagProvided {
		fmt.Fprintln(os.Stderr, "")
		// Print the actual error message from the tool
		if len(result.Content) > 0 {
			fmt.Fprintln(os.Stderr, result.Content[0].Text)
		} else {
			fmt.Fprintln(os.Stderr, "OAuth authentication failed.")
		}
		fmt.Fprintln(os.Stderr, "")

		// If we don't have a handle yet, ask for one
		if handle == "" {
			promptedHandle, err := PromptForInput("Enter handle for app password authentication: ")
			if err != nil {
				return "", fmt.Errorf("failed to read handle: %w", err)
			}
			if promptedHandle == "" {
				// User doesn't want to retry
				return result.Content[0].Text, nil
			}
			handle = promptedHandle
			argsMap["handle"] = handle
		}

		retry, err := PromptForInput("Try app password instead? [y/N]: ")
		if err != nil {
			return "", fmt.Errorf("failed to read choice: %w", err)
		}

		if retry == "y" || retry == "Y" || retry == "yes" || retry == "Yes" {
			fmt.Fprintln(os.Stderr, "")
			promptedPassword, err := PromptForPassword("App Password: ")
			if err != nil {
				return "", fmt.Errorf("failed to read password: %w", err)
			}
			if promptedPassword == "" {
				return "", fmt.Errorf("password cannot be empty")
			}

			// Retry with app password
			argsMap["password"] = promptedPassword
			result, err = a.tool.Call(ctx, argsMap, nil)
			if err != nil {
				return "", err
			}
		}
	}

	// Extract text content from result
	if len(result.Content) > 0 && result.Content[0].Type == "text" {
		return result.Content[0].Text, nil
	}

	return "", nil
}

// isPasswordFlagInArgs checks if --password or -p flag was provided in command line args
func isPasswordFlagInArgs() bool {
	for _, arg := range os.Args {
		if arg == "--password" || arg == "-p" {
			return true
		}
	}
	return false
}

// (no local prompt id generator here; tools create prompt ids when eliciting)
