// Package cli provides command-line interface support for trial mode
package cli

import (
	"context"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// MCPToolAdapter wraps an MCP tool for CLI use
type MCPToolAdapter struct {
	tool mcp.Tool
}

// NewMCPToolAdapter creates a new adapter for an MCP tool
func NewMCPToolAdapter(tool mcp.Tool) *MCPToolAdapter {
	return &MCPToolAdapter{tool: tool}
}

// Execute runs the tool and returns markdown output
func (a *MCPToolAdapter) Execute(ctx context.Context, args interface{}) (string, error) {
	// Convert args to map
	argsMap, err := ConvertToMap(args)
	if err != nil {
		return "", err
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
