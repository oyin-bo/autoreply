// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// PostTool implements the post tool for creating posts
type PostTool struct {
	credStore *auth.CredentialStore
}

// NewPostTool creates a new post tool
func NewPostTool() (*PostTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &PostTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *PostTool) Name() string {
	return "post"
}

// Description returns the tool description
func (t *PostTool) Description() string {
	return "Create a new post on Bluesky. Can optionally reply to another post by providing replyTo URI."
}

// InputSchema returns the JSON schema for tool input
func (t *PostTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"postAs": {
				Type:        "string",
				Description: "Handle or DID to post as (uses default authenticated account if not specified)",
			},
			"text": {
				Type:        "string",
				Description: "Text content of the post (required)",
			},
			"replyTo": {
				Type:        "string",
				Description: "Post URI (at://...) or URL (https://bsky.app/...) to reply to (optional)",
			},
		},
		Required: []string{"text"},
	}
}

// Call executes the post tool
func (t *PostTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate text parameter
	textRaw, ok := args["text"]
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "text parameter is required")
	}

	text, ok := textRaw.(string)
	if !ok {
		return nil, errors.NewMCPError(errors.InvalidInput, "text must be a string")
	}

	text = strings.TrimSpace(text)
	if text == "" {
		return nil, errors.NewMCPError(errors.InvalidInput, "text cannot be empty")
	}

	// Extract optional postAs parameter
	var postAs string
	if postAsRaw, ok := args["postAs"]; ok {
		if postAsStr, ok := postAsRaw.(string); ok {
			postAs = bluesky.NormalizeHandle(postAsStr)
		}
	}

	// Resolve credentials
	creds, err := t.resolveCredentials(postAs)
	if err != nil {
		return nil, err
	}

	// Extract optional replyTo parameter
	var replyTo string
	if replyToRaw, ok := args["replyTo"]; ok {
		if replyToStr, ok := replyToRaw.(string); ok {
			replyTo = strings.TrimSpace(replyToStr)
		}
	}

	// Create the post
	postURI, err := t.createPost(ctx, creds, text, replyTo)
	if err != nil {
		return nil, err
	}

	// Format success message
	var message strings.Builder
	message.WriteString("# Post Created\n\n")
	
	if replyTo != "" {
		message.WriteString(fmt.Sprintf("**Reply to:** %s\n\n", replyTo))
		message.WriteString(fmt.Sprintf("**Your reply:** %s\n\n", text))
	} else {
		message.WriteString(fmt.Sprintf("**Posted:** %s\n\n", text))
	}
	
	message.WriteString(fmt.Sprintf("**Post URI:** %s\n\n", postURI))
	message.WriteString(fmt.Sprintf("**Posted as:** @%s\n", creds.Handle))

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message.String(),
			},
		},
	}, nil
}

// resolveCredentials resolves credentials for the specified handle or uses default
func (t *PostTool) resolveCredentials(postAs string) (*auth.Credentials, error) {
	var handle string
	var err error

	if postAs != "" {
		handle = postAs
	} else {
		// Use default handle
		handle, err = t.credStore.GetDefault()
		if err != nil {
			return nil, errors.NewMCPError(errors.InvalidInput, 
				"No authenticated account found. Please specify postAs or login first using the login tool.")
		}
	}

	// Load credentials
	creds, err := t.credStore.Load(handle)
	if err != nil {
		return nil, errors.Wrap(err, errors.NotFound, 
			fmt.Sprintf("No credentials found for @%s. Please login first using the login tool.", handle))
	}

	return creds, nil
}

// createPost creates a new post with optional reply
func (t *PostTool) createPost(ctx context.Context, creds *auth.Credentials, text, replyTo string) (string, error) {
	client := bluesky.NewClient(creds)

	// Build post record
	record := map[string]interface{}{
		"$type":     "app.bsky.feed.post",
		"text":      text,
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	// Handle reply if specified
	if replyTo != "" {
		replyRef, err := t.resolveReplyReference(ctx, client, replyTo)
		if err != nil {
			return "", errors.Wrap(err, errors.InvalidInput, "Failed to resolve reply target")
		}
		record["reply"] = replyRef
	}

	// Create the post record
	result, err := client.CreateRecord(ctx, creds.DID, "app.bsky.feed.post", record)
	if err != nil {
		return "", errors.Wrap(err, errors.InternalError, "Failed to create post")
	}

	// Extract URI from result
	uri, ok := result["uri"].(string)
	if !ok {
		return "", errors.NewMCPError(errors.InternalError, "Invalid response from server: missing URI")
	}

	return uri, nil
}

// resolveReplyReference resolves a reply-to URI into a reply reference structure
func (t *PostTool) resolveReplyReference(ctx context.Context, client *bluesky.Client, replyTo string) (map[string]interface{}, error) {
	// Parse the URI
	postRef, err := bluesky.ParsePostURI(replyTo)
	if err != nil {
		return nil, fmt.Errorf("invalid reply URI: %w", err)
	}

	// Resolve handle to DID if needed
	did := postRef.DID
	if !bluesky.IsLikelyDID(did) {
		// Need to resolve handle to DID
		resolver := bluesky.NewDIDResolver()
		resolvedDID, err := resolver.ResolveHandle(ctx, did)
		if err != nil {
			return nil, fmt.Errorf("failed to resolve handle %s: %w", did, err)
		}
		did = resolvedDID
	}

	// Get the post record to retrieve its CID
	record, err := client.GetRecord(ctx, did, postRef.Collection, postRef.RKey)
	if err != nil {
		return nil, fmt.Errorf("failed to retrieve reply target post: %w", err)
	}

	uri, ok := record["uri"].(string)
	if !ok {
		return nil, fmt.Errorf("invalid post record: missing URI")
	}

	cid, ok := record["cid"].(string)
	if !ok {
		return nil, fmt.Errorf("invalid post record: missing CID")
	}

	// Check if the target post itself is a reply
	var root map[string]interface{}
	if value, ok := record["value"].(map[string]interface{}); ok {
		if replyData, ok := value["reply"].(map[string]interface{}); ok {
			// This is a reply to another post, use its root
			if rootData, ok := replyData["root"].(map[string]interface{}); ok {
				root = rootData
			}
		}
	}

	// If no root found, this post is the root
	if root == nil {
		root = map[string]interface{}{
			"uri": uri,
			"cid": cid,
		}
	}

	// Build reply reference
	return map[string]interface{}{
		"root": root,
		"parent": map[string]interface{}{
			"uri": uri,
			"cid": cid,
		},
	}, nil
}
