// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"strings"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/cache"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// ProfileTool implements the profile tool
type ProfileTool struct {
	didResolver  *bluesky.DIDResolver
	carProcessor *bluesky.CARProcessor
}

// NewProfileTool creates a new profile tool
func NewProfileTool() *ProfileTool {
	cacheManager, _ := cache.NewManager()
	return &ProfileTool{
		didResolver:  bluesky.NewDIDResolver(),
		carProcessor: bluesky.NewCARProcessor(cacheManager),
	}
}

// Name returns the tool name
func (t *ProfileTool) Name() string {
	return "profile"
}

// Description returns the tool description
func (t *ProfileTool) Description() string {
	return "Retrieve user profile information from Bluesky"
}

// InputSchema returns the JSON schema for tool input
func (t *ProfileTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"account": {
				Type:        "string",
				Description: "Handle (alice.bsky.social) or DID (did:plc:...)",
			},
		},
		Required: []string{"account"},
	}
}

// Call executes the profile tool
func (t *ProfileTool) Call(ctx context.Context, args map[string]interface{}) (*mcp.ToolResult, error) {
	// Extract and validate account parameter
	accountRaw, ok := args["account"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "account parameter is required")
	}

	account, ok := accountRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "account must be a string")
	}

	if strings.TrimSpace(account) == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "account cannot be empty")
	}

	// Resolve handle to DID
	did, err := t.didResolver.ResolveHandle(ctx, account)
	if err != nil {
		return nil, err
	}

	// Fetch repository if needed
	if err := t.carProcessor.FetchRepository(ctx, did); err != nil {
		return nil, err
	}

	// Get profile information
	profile, err := t.carProcessor.GetProfile(did)
	if err != nil {
		return nil, err
	}

	// Format as markdown
	markdown := t.formatProfileMarkdown(account, did, profile)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// formatProfileMarkdown formats profile information as markdown
func (t *ProfileTool) formatProfileMarkdown(handle, did string, profile *bluesky.ParsedProfile) string {
	var sb strings.Builder

	// Header with Bluesky web link
	cleanHandle := strings.TrimPrefix(handle, "@")
	profileURL := fmt.Sprintf("https://bsky.app/profile/%s", cleanHandle)
	sb.WriteString(fmt.Sprintf("# [@%s](%s)\n\n", cleanHandle, profileURL))
	sb.WriteString(fmt.Sprintf("**DID:** `%s`\n\n", did))

	// Display name
	if profile.DisplayName != nil && *profile.DisplayName != "" {
		sb.WriteString(fmt.Sprintf("**Display Name:** %s\n\n", *profile.DisplayName))
	}

	// Description
	if profile.Description != nil && *profile.Description != "" {
		sb.WriteString("**Description:**\n")
		sb.WriteString(fmt.Sprintf("%s\n\n", *profile.Description))
	}

	// Avatar
	if profile.Avatar != nil && *profile.Avatar != "" {
		sb.WriteString(fmt.Sprintf("**Avatar:** ![Avatar](%s)\n\n", *profile.Avatar))
	}

	// Stats
	sb.WriteString("**Stats:**\n")
	if profile.CreatedAt != "" {
		sb.WriteString(fmt.Sprintf("- Created: %s\n", profile.CreatedAt))
	}
	sb.WriteString(fmt.Sprintf("- Profile fetched: %s\n", profile.ParsedTime.Format("2006-01-02 15:04:05")))

	// Raw data section
	sb.WriteString("\n<details>\n<summary>Raw Profile Data</summary>\n\n```json\n")
	sb.WriteString("{\n")
	if profile.DisplayName != nil {
		sb.WriteString(fmt.Sprintf("  \"displayName\": \"%s\",\n", *profile.DisplayName))
	}
	if profile.Description != nil {
		sb.WriteString(fmt.Sprintf("  \"description\": \"%s\",\n", *profile.Description))
	}
	if profile.CreatedAt != "" {
		sb.WriteString(fmt.Sprintf("  \"createdAt\": \"%s\"\n", profile.CreatedAt))
	}
	sb.WriteString("}\n```\n\n</details>")

	return sb.String()
}
