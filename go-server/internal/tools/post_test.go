package tools

import (
	"context"
	"testing"
)

func TestPostTool_Name(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	if tool.Name() != "post" {
		t.Errorf("Expected name 'post', got '%s'", tool.Name())
	}
}

func TestPostTool_InputSchema(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	schema := tool.InputSchema()
	
	if schema.Type != "object" {
		t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
	}

	// Check required field
	if len(schema.Required) != 1 || schema.Required[0] != "text" {
		t.Errorf("Expected required field 'text', got %v", schema.Required)
	}

	// Check text property exists
	if _, ok := schema.Properties["text"]; !ok {
		t.Error("Expected 'text' property in schema")
	}

	// Check postAs property exists
	if _, ok := schema.Properties["postAs"]; !ok {
		t.Error("Expected 'postAs' property in schema")
	}

	// Check replyTo property exists
	if _, ok := schema.Properties["replyTo"]; !ok {
		t.Error("Expected 'replyTo' property in schema")
	}
}

func TestPostTool_Call_MissingText(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	ctx := context.Background()
	args := map[string]interface{}{}

	_, err = tool.Call(ctx, args, nil)
	if err == nil {
		t.Error("Expected error when text is missing")
	}
}

func TestPostTool_Call_EmptyText(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	ctx := context.Background()
	args := map[string]interface{}{
		"text": "   ",
	}

	_, err = tool.Call(ctx, args, nil)
	if err == nil {
		t.Error("Expected error when text is empty/whitespace")
	}
}

func TestPostTool_Call_NoAuth(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	ctx := context.Background()
	args := map[string]interface{}{
		"text": "Test post",
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

func TestPostTool_Description(t *testing.T) {
	tool, err := NewPostTool()
	if err != nil {
		t.Fatalf("Failed to create post tool: %v", err)
	}

	desc := tool.Description()
	if desc == "" {
		t.Error("Expected non-empty description")
	}
	
	// Description should mention key functionality
	if !contains(desc, "post") && !contains(desc, "Post") {
		t.Error("Description should mention 'post'")
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && 
		(s[:len(substr)] == substr || s[len(s)-len(substr):] == substr || 
			containsHelper(s, substr)))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
