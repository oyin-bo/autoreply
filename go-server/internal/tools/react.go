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

// ReactTool implements the react tool for batch reactions (like, unlike, repost, delete)
type ReactTool struct {
	credStore *auth.CredentialStore
}

// NewReactTool creates a new react tool
func NewReactTool() (*ReactTool, error) {
	credStore, err := auth.NewCredentialStore()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize credential store: %w", err)
	}

	return &ReactTool{
		credStore: credStore,
	}, nil
}

// Name returns the tool name
func (t *ReactTool) Name() string {
	return "react"
}

// Description returns the tool description
func (t *ReactTool) Description() string {
	return "Perform reactions on posts: like, unlike, repost, or delete. Supports batching multiple operations in a single call. URIs can be in at:// or https://bsky.app/... format."
}

// InputSchema returns the JSON schema for tool input
func (t *ReactTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"reactAs": {
				Type:        "string",
				Description: "Handle or DID to react as (uses default authenticated account if not specified)",
			},
			"like": {
				Type:        "array",
				Description: "Array of post URIs to like",
			},
			"unlike": {
				Type:        "array",
				Description: "Array of post URIs to unlike (remove like)",
			},
			"repost": {
				Type:        "array",
				Description: "Array of post URIs to repost",
			},
			"delete": {
				Type:        "array",
				Description: "Array of post URIs to delete (only works for your own posts)",
			},
		},
	}
}

// Call executes the react tool
func (t *ReactTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract optional reactAs parameter
	var reactAs string
	if reactAsRaw, ok := args["reactAs"]; ok {
		if reactAsStr, ok := reactAsRaw.(string); ok {
			reactAs = bluesky.NormalizeHandle(reactAsStr)
		}
	}

	// Resolve credentials
	creds, err := t.resolveCredentials(reactAs)
	if err != nil {
		return nil, err
	}

	client := bluesky.NewClient(creds)

	// Process all reactions
	results := &reactionResults{
		Handle: creds.Handle,
	}

	// Process likes
	if likeRaw, ok := args["like"]; ok {
		if likeArray, ok := likeRaw.([]interface{}); ok {
			for _, uriRaw := range likeArray {
				if uri, ok := uriRaw.(string); ok {
					err := t.likePost(ctx, client, creds.DID, uri)
					results.addResult("like", uri, err)
				}
			}
		}
	}

	// Process unlikes
	if unlikeRaw, ok := args["unlike"]; ok {
		if unlikeArray, ok := unlikeRaw.([]interface{}); ok {
			for _, uriRaw := range unlikeArray {
				if uri, ok := uriRaw.(string); ok {
					err := t.unlikePost(ctx, client, creds.DID, uri)
					results.addResult("unlike", uri, err)
				}
			}
		}
	}

	// Process reposts
	if repostRaw, ok := args["repost"]; ok {
		if repostArray, ok := repostRaw.([]interface{}); ok {
			for _, uriRaw := range repostArray {
				if uri, ok := uriRaw.(string); ok {
					err := t.repostPost(ctx, client, creds.DID, uri)
					results.addResult("repost", uri, err)
				}
			}
		}
	}

	// Process deletes
	if deleteRaw, ok := args["delete"]; ok {
		if deleteArray, ok := deleteRaw.([]interface{}); ok {
			for _, uriRaw := range deleteArray {
				if uri, ok := uriRaw.(string); ok {
					err := t.deletePost(ctx, client, creds.DID, uri)
					results.addResult("delete", uri, err)
				}
			}
		}
	}

	// Check if any operations were requested
	if results.isEmpty() {
		return nil, errors.NewMCPError(errors.InvalidInput, 
			"No operations specified. Please provide at least one of: like, unlike, repost, or delete")
	}

	// Format results as markdown
	message := results.formatMarkdown()

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: message,
			},
		},
		IsError: results.hasErrors(),
	}, nil
}

// resolveCredentials resolves credentials for the specified handle or uses default
func (t *ReactTool) resolveCredentials(reactAs string) (*auth.Credentials, error) {
	var handle string
	var err error

	if reactAs != "" {
		handle = reactAs
	} else {
		// Use default handle
		handle, err = t.credStore.GetDefault()
		if err != nil {
			return nil, errors.NewMCPError(errors.InvalidInput, 
				"No authenticated account found. Please specify reactAs or login first using the login tool.")
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

// likePost likes a post
func (t *ReactTool) likePost(ctx context.Context, client *bluesky.Client, myDID, postURI string) error {
	// Parse the URI
	postRef, err := bluesky.ParsePostURI(postURI)
	if err != nil {
		return fmt.Errorf("invalid URI: %w", err)
	}

	// Resolve handle to DID if needed
	did := postRef.DID
	if !bluesky.IsLikelyDID(did) {
		resolver := bluesky.NewDIDResolver()
		did, err = resolver.ResolveHandle(ctx, did)
		if err != nil {
			return fmt.Errorf("failed to resolve handle: %w", err)
		}
	}

	// Get the post to retrieve its CID
	record, err := client.GetRecord(ctx, did, postRef.Collection, postRef.RKey)
	if err != nil {
		return fmt.Errorf("failed to retrieve post: %w", err)
	}

	uri, _ := record["uri"].(string)
	cid, ok := record["cid"].(string)
	if !ok {
		return fmt.Errorf("invalid post record: missing CID")
	}

	// Create like record
	likeRecord := map[string]interface{}{
		"$type": "app.bsky.feed.like",
		"subject": map[string]interface{}{
			"uri": uri,
			"cid": cid,
		},
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	_, err = client.CreateRecord(ctx, myDID, "app.bsky.feed.like", likeRecord)
	return err
}

// unlikePost removes a like from a post
func (t *ReactTool) unlikePost(ctx context.Context, client *bluesky.Client, myDID, postURI string) error {
	// Parse the URI
	postRef, err := bluesky.ParsePostURI(postURI)
	if err != nil {
		return fmt.Errorf("invalid URI: %w", err)
	}

	// Resolve handle to DID if needed
	did := postRef.DID
	if !bluesky.IsLikelyDID(did) {
		resolver := bluesky.NewDIDResolver()
		did, err = resolver.ResolveHandle(ctx, did)
		if err != nil {
			return fmt.Errorf("failed to resolve handle: %w", err)
		}
	}

	// Get the post URI
	uri := bluesky.MakePostURI(did, postRef.RKey)

	// Find the like record to delete
	// List all likes by the user
	records, err := client.ListRecords(ctx, myDID, "app.bsky.feed.like", 100)
	if err != nil {
		return fmt.Errorf("failed to list likes: %w", err)
	}

	// Find the like for this specific post
	for _, rec := range records {
		if value, ok := rec["value"].(map[string]interface{}); ok {
			if subject, ok := value["subject"].(map[string]interface{}); ok {
				if subjectURI, ok := subject["uri"].(string); ok && subjectURI == uri {
					// Found the like, delete it
					if rkey, ok := rec["uri"].(string); ok {
						// Extract rkey from uri: at://did/collection/rkey
						parts := strings.Split(rkey, "/")
						if len(parts) >= 4 {
							return client.DeleteRecord(ctx, myDID, "app.bsky.feed.like", parts[3])
						}
					}
				}
			}
		}
	}

	return fmt.Errorf("like not found for post %s", uri)
}

// repostPost reposts a post
func (t *ReactTool) repostPost(ctx context.Context, client *bluesky.Client, myDID, postURI string) error {
	// Parse the URI
	postRef, err := bluesky.ParsePostURI(postURI)
	if err != nil {
		return fmt.Errorf("invalid URI: %w", err)
	}

	// Resolve handle to DID if needed
	did := postRef.DID
	if !bluesky.IsLikelyDID(did) {
		resolver := bluesky.NewDIDResolver()
		did, err = resolver.ResolveHandle(ctx, did)
		if err != nil {
			return fmt.Errorf("failed to resolve handle: %w", err)
		}
	}

	// Get the post to retrieve its CID
	record, err := client.GetRecord(ctx, did, postRef.Collection, postRef.RKey)
	if err != nil {
		return fmt.Errorf("failed to retrieve post: %w", err)
	}

	uri, _ := record["uri"].(string)
	cid, ok := record["cid"].(string)
	if !ok {
		return fmt.Errorf("invalid post record: missing CID")
	}

	// Create repost record
	repostRecord := map[string]interface{}{
		"$type": "app.bsky.feed.repost",
		"subject": map[string]interface{}{
			"uri": uri,
			"cid": cid,
		},
		"createdAt": time.Now().UTC().Format(time.RFC3339),
	}

	_, err = client.CreateRecord(ctx, myDID, "app.bsky.feed.repost", repostRecord)
	return err
}

// deletePost deletes a post (only works for user's own posts)
func (t *ReactTool) deletePost(ctx context.Context, client *bluesky.Client, myDID, postURI string) error {
	// Parse the URI
	postRef, err := bluesky.ParsePostURI(postURI)
	if err != nil {
		return fmt.Errorf("invalid URI: %w", err)
	}

	// Resolve handle to DID if needed
	did := postRef.DID
	if !bluesky.IsLikelyDID(did) {
		resolver := bluesky.NewDIDResolver()
		did, err = resolver.ResolveHandle(ctx, did)
		if err != nil {
			return fmt.Errorf("failed to resolve handle: %w", err)
		}
	}

	// Verify this is the user's own post
	if did != myDID {
		return fmt.Errorf("can only delete your own posts")
	}

	return client.DeleteRecord(ctx, myDID, postRef.Collection, postRef.RKey)
}

// reactionResults tracks results of batch operations
type reactionResults struct {
	Handle   string
	Likes    []operationResult
	Unlikes  []operationResult
	Reposts  []operationResult
	Deletes  []operationResult
}

type operationResult struct {
	URI   string
	Error error
}

func (r *reactionResults) addResult(operation, uri string, err error) {
	result := operationResult{URI: uri, Error: err}
	switch operation {
	case "like":
		r.Likes = append(r.Likes, result)
	case "unlike":
		r.Unlikes = append(r.Unlikes, result)
	case "repost":
		r.Reposts = append(r.Reposts, result)
	case "delete":
		r.Deletes = append(r.Deletes, result)
	}
}

func (r *reactionResults) isEmpty() bool {
	return len(r.Likes) == 0 && len(r.Unlikes) == 0 && len(r.Reposts) == 0 && len(r.Deletes) == 0
}

func (r *reactionResults) hasErrors() bool {
	for _, result := range r.Likes {
		if result.Error != nil {
			return true
		}
	}
	for _, result := range r.Unlikes {
		if result.Error != nil {
			return true
		}
	}
	for _, result := range r.Reposts {
		if result.Error != nil {
			return true
		}
	}
	for _, result := range r.Deletes {
		if result.Error != nil {
			return true
		}
	}
	return false
}

func (r *reactionResults) formatMarkdown() string {
	var sb strings.Builder
	
	sb.WriteString("# Reaction Results\n\n")
	sb.WriteString(fmt.Sprintf("**Acting as:** @%s\n\n", r.Handle))

	totalOps := len(r.Likes) + len(r.Unlikes) + len(r.Reposts) + len(r.Deletes)
	successCount := 0
	
	// Count successes
	for _, result := range r.Likes {
		if result.Error == nil {
			successCount++
		}
	}
	for _, result := range r.Unlikes {
		if result.Error == nil {
			successCount++
		}
	}
	for _, result := range r.Reposts {
		if result.Error == nil {
			successCount++
		}
	}
	for _, result := range r.Deletes {
		if result.Error == nil {
			successCount++
		}
	}

	sb.WriteString(fmt.Sprintf("**Summary:** %d of %d operations successful\n\n", successCount, totalOps))

	// Format likes
	if len(r.Likes) > 0 {
		sb.WriteString("## Likes\n\n")
		for _, result := range r.Likes {
			if result.Error == nil {
				sb.WriteString(fmt.Sprintf("✅ Liked: %s\n", result.URI))
			} else {
				sb.WriteString(fmt.Sprintf("❌ Failed to like %s: %s\n", result.URI, result.Error.Error()))
			}
		}
		sb.WriteString("\n")
	}

	// Format unlikes
	if len(r.Unlikes) > 0 {
		sb.WriteString("## Unlikes\n\n")
		for _, result := range r.Unlikes {
			if result.Error == nil {
				sb.WriteString(fmt.Sprintf("✅ Unliked: %s\n", result.URI))
			} else {
				sb.WriteString(fmt.Sprintf("❌ Failed to unlike %s: %s\n", result.URI, result.Error.Error()))
			}
		}
		sb.WriteString("\n")
	}

	// Format reposts
	if len(r.Reposts) > 0 {
		sb.WriteString("## Reposts\n\n")
		for _, result := range r.Reposts {
			if result.Error == nil {
				sb.WriteString(fmt.Sprintf("✅ Reposted: %s\n", result.URI))
			} else {
				sb.WriteString(fmt.Sprintf("❌ Failed to repost %s: %s\n", result.URI, result.Error.Error()))
			}
		}
		sb.WriteString("\n")
	}

	// Format deletes
	if len(r.Deletes) > 0 {
		sb.WriteString("## Deletes\n\n")
		for _, result := range r.Deletes {
			if result.Error == nil {
				sb.WriteString(fmt.Sprintf("✅ Deleted: %s\n", result.URI))
			} else {
				sb.WriteString(fmt.Sprintf("❌ Failed to delete %s: %s\n", result.URI, result.Error.Error()))
			}
		}
		sb.WriteString("\n")
	}

	return sb.String()
}
