package tools

import (
	"context"
	"testing"
)

func TestReactTool_Name(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	if tool.Name() != "react" {
		t.Errorf("Expected name 'react', got '%s'", tool.Name())
	}
}

func TestReactTool_InputSchema(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	schema := tool.InputSchema()
	
	if schema.Type != "object" {
		t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
	}

	// Check properties exist
	expectedProps := []string{"reactAs", "like", "unlike", "repost", "delete"}
	for _, prop := range expectedProps {
		if _, ok := schema.Properties[prop]; !ok {
			t.Errorf("Expected '%s' property in schema", prop)
		}
	}
}

func TestReactTool_Call_NoOperations(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	ctx := context.Background()
	args := map[string]interface{}{}

	_, err = tool.Call(ctx, args, nil)
	if err == nil {
		t.Error("Expected error when no operations specified")
	}
}

func TestReactTool_Call_NoAuth(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	ctx := context.Background()
	args := map[string]interface{}{
		"like": []interface{}{"at://did:plc:test/app.bsky.feed.post/test"},
	}

	result, err := tool.Call(ctx, args, nil)
	
	// Should fail because no credentials are set up
	if err == nil {
		t.Error("Expected error when no credentials available")
	}
	
	if result != nil {
		t.Error("Expected nil result when authentication fails")
	}
}

func TestReactTool_Description(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	desc := tool.Description()
	if desc == "" {
		t.Error("Expected non-empty description")
	}
	
	// Description should mention key operations
	keywords := []string{"like", "repost", "delete"}
	foundKeyword := false
	for _, keyword := range keywords {
		if contains(desc, keyword) {
			foundKeyword = true
			break
		}
	}
	if !foundKeyword {
		t.Errorf("Description should mention at least one of: %v", keywords)
	}
}

func TestReactionResults_FormatMarkdown(t *testing.T) {
	results := &reactionResults{
		Handle: "test.bsky.social",
		Likes: []operationResult{
			{URI: "at://did:plc:test/app.bsky.feed.post/123", Error: nil},
		},
		Unlikes: []operationResult{
			{URI: "at://did:plc:test/app.bsky.feed.post/456", Error: nil},
		},
	}

	markdown := results.formatMarkdown()
	
	if markdown == "" {
		t.Error("Expected non-empty markdown output")
	}
	
	// Check that markdown contains expected sections
	if !contains(markdown, "Reaction Results") {
		t.Error("Markdown should contain 'Reaction Results' header")
	}
	
	if !contains(markdown, "test.bsky.social") {
		t.Error("Markdown should contain handle")
	}
	
	if !contains(markdown, "Likes") {
		t.Error("Markdown should contain 'Likes' section")
	}
	
	if !contains(markdown, "Unlikes") {
		t.Error("Markdown should contain 'Unlikes' section")
	}
}

func TestReactionResults_HasErrors(t *testing.T) {
	// Test with no errors
	results := &reactionResults{
		Likes: []operationResult{
			{URI: "at://test/post/123", Error: nil},
		},
	}
	
	if results.hasErrors() {
		t.Error("Expected hasErrors to be false when no errors")
	}
	
	// Test with errors
	results.Likes = []operationResult{
		{URI: "at://test/post/123", Error: context.Canceled},
	}
	
	if !results.hasErrors() {
		t.Error("Expected hasErrors to be true when errors present")
	}
}

func TestReactionResults_IsEmpty(t *testing.T) {
	// Test empty results
	results := &reactionResults{}
	
	if !results.isEmpty() {
		t.Error("Expected isEmpty to be true for empty results")
	}
	
	// Test non-empty results
	results.Likes = []operationResult{
		{URI: "at://test/post/123"},
	}
	
	if results.isEmpty() {
		t.Error("Expected isEmpty to be false when results exist")
	}
}
