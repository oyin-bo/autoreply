// common.go - Shared tool utilities
package tools

import (
	"context"
	"encoding/json"

	"github.com/oyin-bo/autoreply/pkg/errors"
)

// Tool represents a tool definition for tools/list
type Tool struct {
	Name        string      `json:"name"`
	Description string      `json:"description"`
	InputSchema interface{} `json:"inputSchema"`
}

// ContentItem represents a piece of content in tool results
type ContentItem struct {
	Type string `json:"type"`
	Text string `json:"text"`
}

// ToolResult represents the result of a tool call
type ToolResult struct {
	Content []ContentItem `json:"content"`
}

// Manager manages all available tools
type Manager struct {
	profile *ProfileTool
	search  *SearchTool
}

// NewManager creates a new tools manager
func NewManager() *Manager {
	return &Manager{
		profile: NewProfileTool(),
		search:  NewSearchTool(),
	}
}

// ListTools returns all available tools
func (m *Manager) ListTools() []Tool {
	return []Tool{
		{
			Name:        "profile",
			Description: "Retrieve user profile information from their Bluesky repository",
			InputSchema: map[string]interface{}{
				"type": "object",
				"properties": map[string]interface{}{
					"account": map[string]interface{}{
						"type":        "string",
						"description": "Handle (alice.bsky.social) or DID (did:plc:...)",
					},
				},
				"required": []string{"account"},
			},
		},
		{
			Name:        "search",
			Description: "Search posts within a user's Bluesky repository",
			InputSchema: map[string]interface{}{
				"type": "object",
				"properties": map[string]interface{}{
					"account": map[string]interface{}{
						"type":        "string",
						"description": "Handle (alice.bsky.social) or DID (did:plc:...)",
					},
					"query": map[string]interface{}{
						"type":        "string",
						"description": "Search terms (case-insensitive)",
					},
				},
				"required": []string{"account", "query"},
			},
		},
	}
}

// CallTool executes a tool by name
func (m *Manager) CallTool(ctx context.Context, name string, args json.RawMessage) (*ToolResult, error) {
	switch name {
	case "profile":
		return m.profile.Execute(ctx, args)
	case "search":
		return m.search.Execute(ctx, args)
	default:
		return nil, errors.NewMcpError(errors.InvalidInput, "Tool not found: "+name)
	}
}