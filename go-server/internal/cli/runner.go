// Package cli provides command-line interface support for trial mode
package cli

import (
	"context"
	"fmt"
	"os"

	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
	"github.com/spf13/cobra"
)

// Runner handles CLI execution
type Runner struct {
	registry *Registry
	rootCmd  *cobra.Command
}

// NewRunner creates a new CLI runner
func NewRunner(registry *Registry) *Runner {
	runner := &Runner{
		registry: registry,
	}

	// Create root command
	runner.rootCmd = &cobra.Command{
		Use:   "autoreply",
		Short: "Autoreply - Bluesky profile and search tool",
		Long: `Autoreply is a tool for retrieving Bluesky profiles and searching posts.

It operates in two modes:
1. MCP Server Mode (default): Run without arguments to start an MCP server
2. CLI Mode: Run with a command to execute a single tool and exit`,
		SilenceUsage: true,
		// Do NOT silence errors - they should be printed to stderr
	}

	// Add version flag
	runner.rootCmd.Version = "0.1.0"
	runner.rootCmd.SetVersionTemplate("autoreply version {{.Version}}\n")

	return runner
}

// RegisterToolCommand adds a tool command to the CLI
func (r *Runner) RegisterToolCommand(def *ToolDefinition) {
	execute := func(ctx context.Context, args interface{}) error {
		output, err := def.Execute(ctx, args)
		if err != nil {
			return r.handleError(err)
		}

		// Output result to stdout
		fmt.Println(output)
		return nil
	}

	cmd := CreateCobraCommand(def, execute)
	r.rootCmd.AddCommand(cmd)
}

// Run executes the CLI
func (r *Runner) Run(ctx context.Context, args []string) error {
	r.rootCmd.SetArgs(args)
	r.rootCmd.SetContext(ctx)
	return r.rootCmd.Execute()
}

// handleError converts errors to appropriate exit codes
func (r *Runner) handleError(err error) error {
	if mcpErr, ok := err.(*errors.MCPError); ok {
		switch mcpErr.Code {
		case errors.InvalidInput:
			fmt.Fprintf(os.Stderr, "Error: %v\n", mcpErr.Message)
			os.Exit(1)
		case errors.NotFound:
			fmt.Fprintf(os.Stderr, "Error: %v\n", mcpErr.Message)
			os.Exit(3)
		case errors.DIDResolveFailed, errors.RepoFetchFailed:
			fmt.Fprintf(os.Stderr, "Error: %v\n", mcpErr.Message)
			os.Exit(2)
		case errors.Timeout:
			fmt.Fprintf(os.Stderr, "Error: %v\n", mcpErr.Message)
			os.Exit(4)
		default:
			fmt.Fprintf(os.Stderr, "Error: %v\n", mcpErr.Message)
			os.Exit(5)
		}
	}

	// Generic error
	fmt.Fprintf(os.Stderr, "Error: %v\n", err)
	os.Exit(5)
	return err
}
