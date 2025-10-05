// Package mcp provides the MCP server implementation
package mcp

import (
	"context"
	"testing"
)

// TestToolsListResponse tests that tools/list returns correct tool names
func TestToolsListResponse(t *testing.T) {
	server, err := NewServer()
	if err != nil {
		t.Fatalf("Failed to create server: %v", err)
	}

	// This test will be populated when tools are registered
	// For now, verify the structure works
	result := server.listTools()

	if result == nil {
		t.Fatal("Tools list should not be nil")
	}

	if result.Tools == nil {
		t.Fatal("Tools array should not be nil")
	}

	// Empty server should have no tools
	if len(result.Tools) != 0 {
		t.Errorf("Expected 0 tools in empty server, got %d", len(result.Tools))
	}
}

// TestToolRegistration tests that tools can be registered
func TestToolRegistration(t *testing.T) {
	server, err := NewServer()
	if err != nil {
		t.Fatalf("Failed to create server: %v", err)
	}

	// Create a mock tool
	mockTool := &mockTool{
		name:        "test-tool",
		description: "Test tool description",
	}

	// Register it
	server.RegisterTool("test-tool", mockTool)

	// List tools
	result := server.listTools()

	if len(result.Tools) != 1 {
		t.Fatalf("Expected 1 tool, got %d", len(result.Tools))
	}

	if result.Tools[0].Name != "test-tool" {
		t.Errorf("Expected tool name 'test-tool', got '%s'", result.Tools[0].Name)
	}

	if result.Tools[0].Description != "Test tool description" {
		t.Errorf("Expected description 'Test tool description', got '%s'", result.Tools[0].Description)
	}
}

// TestToolRegistrationMultiple tests multiple tool registration
func TestToolRegistrationMultiple(t *testing.T) {
	server, err := NewServer()
	if err != nil {
		t.Fatalf("Failed to create server: %v", err)
	}

	// Register multiple tools
	tools := []string{"login", "profile", "search"}
	for _, name := range tools {
		server.RegisterTool(name, &mockTool{name: name, description: name + " tool"})
	}

	// List tools
	result := server.listTools()

	if len(result.Tools) != len(tools) {
		t.Fatalf("Expected %d tools, got %d", len(tools), len(result.Tools))
	}

	// Verify all tools are present (order may vary)
	foundTools := make(map[string]bool)
	for _, tool := range result.Tools {
		foundTools[tool.Name] = true
	}

	for _, expectedName := range tools {
		if !foundTools[expectedName] {
			t.Errorf("Expected to find tool '%s' in list", expectedName)
		}
	}
}

// TestToolSchemaStructure tests that tools return valid schemas
func TestToolSchemaStructure(t *testing.T) {
	server, err := NewServer()
	if err != nil {
		t.Fatalf("Failed to create server: %v", err)
	}

	mockTool := &mockTool{
		name:        "test-tool",
		description: "Test tool",
		schema: InputSchema{
			Type: "object",
			Properties: map[string]PropertySchema{
				"param1": {Type: "string", Description: "Parameter 1"},
				"param2": {Type: "integer", Description: "Parameter 2"},
			},
			Required: []string{"param1"},
		},
	}

	server.RegisterTool("test-tool", mockTool)

	result := server.listTools()

	if len(result.Tools) != 1 {
		t.Fatalf("Expected 1 tool, got %d", len(result.Tools))
	}

	schema := result.Tools[0].InputSchema

	if schema.Type != "object" {
		t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
	}

	if len(schema.Properties) != 2 {
		t.Errorf("Expected 2 properties, got %d", len(schema.Properties))
	}

	if _, ok := schema.Properties["param1"]; !ok {
		t.Error("Expected property 'param1'")
	}

	if _, ok := schema.Properties["param2"]; !ok {
		t.Error("Expected property 'param2'")
	}

	if len(schema.Required) != 1 || schema.Required[0] != "param1" {
		t.Errorf("Expected required field 'param1', got %v", schema.Required)
	}
}

// mockTool is a mock implementation of the Tool interface for testing
type mockTool struct {
	name        string
	description string
	schema      InputSchema
}

func (m *mockTool) Name() string {
	return m.name
}

func (m *mockTool) Description() string {
	return m.description
}

func (m *mockTool) InputSchema() InputSchema {
	if m.schema.Type == "" {
		return InputSchema{
			Type:       "object",
			Properties: map[string]PropertySchema{},
		}
	}
	return m.schema
}

func (m *mockTool) Call(ctx context.Context, args map[string]interface{}, server *Server) (*ToolResult, error) {
	return &ToolResult{
		Content: []ContentItem{
			{Type: "text", Text: "Mock response"},
		},
	}, nil
}
