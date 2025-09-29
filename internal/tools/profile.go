// profile.go - Profile tool implementation
package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/internal/bluesky"
	"github.com/oyin-bo/autoreply/pkg/errors"
)

// ProfileTool implements the profile tool
type ProfileTool struct {
	processor *bluesky.CarProcessor
	resolver  *bluesky.DidResolver
}

// ProfileArgs represents arguments for the profile tool
type ProfileArgs struct {
	Account string `json:"account"`
}

// NewProfileTool creates a new profile tool
func NewProfileTool() *ProfileTool {
	processor, err := bluesky.NewCarProcessor()
	if err != nil {
		panic(fmt.Sprintf("Failed to create CAR processor: %v", err))
	}

	return &ProfileTool{
		processor: processor,
		resolver:  bluesky.NewDidResolver(),
	}
}

// Execute executes the profile tool
func (p *ProfileTool) Execute(ctx context.Context, args json.RawMessage) (*ToolResult, error) {
	// Set total timeout to 120 seconds as specified
	ctx, cancel := context.WithTimeout(ctx, 120*time.Second)
	defer cancel()

	// Parse arguments
	var profileArgs ProfileArgs
	if err := json.Unmarshal(args, &profileArgs); err != nil {
		return nil, errors.NewMcpError(errors.InvalidInput, fmt.Sprintf("Invalid arguments: %v", err))
	}

	// Validate account parameter
	if err := errors.ValidateAccount(profileArgs.Account); err != nil {
		return nil, err
	}

	return p.executeImpl(ctx, profileArgs)
}

func (p *ProfileTool) executeImpl(ctx context.Context, args ProfileArgs) (*ToolResult, error) {
	// Resolve handle to DID if necessary
	did, err := p.resolver.ResolveHandle(ctx, args.Account)
	if err != nil {
		return nil, err
	}

	// Determine display handle
	displayHandle := args.Account
	if strings.HasPrefix(args.Account, "did:plc:") {
		displayHandle = did // Use DID if that's what was provided
	}

	// Fetch repository CAR file
	carData, err := p.processor.FetchRepo(ctx, did)
	if err != nil {
		return nil, err
	}

	// Extract profile record
	profile, err := p.processor.ExtractProfileRecord(carData, did)
	if err != nil {
		return nil, err
	}

	// Discover PDS endpoint for additional info
	pdsEndpoint, err := p.resolver.DiscoverPDS(ctx, did)
	if err != nil {
		// Continue without PDS info if discovery fails
		pdsEndpoint = "Unknown"
	}

	// Format profile as markdown
	markdown := p.formatProfile(profile, displayHandle, did, pdsEndpoint)

	return &ToolResult{
		Content: []ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// formatProfile formats profile data as markdown
func (p *ProfileTool) formatProfile(profile *bluesky.ProfileRecord, handle, did, pdsEndpoint string) string {
	var markdown strings.Builder

	// Header
	markdown.WriteString(fmt.Sprintf("# @%s (%s)\n\n", handle, did))

	// Display name
	if profile.DisplayName != nil && *profile.DisplayName != "" {
		markdown.WriteString(fmt.Sprintf("**Display Name:** %s\n\n", *profile.DisplayName))
	}

	// Description
	description := profile.GetDescription()
	if description != "" {
		markdown.WriteString("**Description:**\n")
		markdown.WriteString(fmt.Sprintf("%s\n\n", description))
	}

	// Avatar (if present)
	if profile.Avatar != nil && *profile.Avatar != "" {
		markdown.WriteString(fmt.Sprintf("**Avatar:** ![Avatar](%s)\n\n", *profile.Avatar))
	}

	// Banner (if present)
	if profile.Banner != nil && *profile.Banner != "" {
		markdown.WriteString(fmt.Sprintf("**Banner:** ![Banner](%s)\n\n", *profile.Banner))
	}

	// Stats section
	markdown.WriteString("**Stats:**\n")
	
	// Created date
	if createdAt, err := profile.GetCreatedAt(); err == nil {
		markdown.WriteString(fmt.Sprintf("- Created: %s\n", createdAt.Format("January 2, 2006")))
	} else {
		markdown.WriteString(fmt.Sprintf("- Created: %s\n", profile.CreatedAt))
	}

	// PDS endpoint
	markdown.WriteString(fmt.Sprintf("- PDS: %s\n\n", pdsEndpoint))

	// Raw profile data in collapsible section
	markdown.WriteString("<details>\n<summary>Raw Profile Data</summary>\n\n")
	markdown.WriteString("```json\n")
	
	// Create a clean JSON representation
	profileData := map[string]interface{}{
		"$type":     profile.Type,
		"createdAt": profile.CreatedAt,
	}
	
	if profile.DisplayName != nil {
		profileData["displayName"] = *profile.DisplayName
	}
	if profile.Description != nil {
		profileData["description"] = *profile.Description
	}
	if profile.Avatar != nil {
		profileData["avatar"] = *profile.Avatar
	}
	if profile.Banner != nil {
		profileData["banner"] = *profile.Banner
	}

	if jsonBytes, err := json.MarshalIndent(profileData, "", "  "); err == nil {
		markdown.WriteString(string(jsonBytes))
	} else {
		markdown.WriteString("Error formatting profile data")
	}
	
	markdown.WriteString("\n```\n</details>\n")

	return markdown.String()
}