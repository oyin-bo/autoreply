package mcp

import (
	"context"
	"strings"
	"testing"
)

// TestNoPromptIDInToolsList ensures the server does not expose any prompt_id in tools/list schemas
func TestNoPromptIDInToolsList(t *testing.T) {
	server, err := NewServer()
	if err != nil {
		t.Fatalf("Failed to create server: %v", err)
	}

	// Register a minimal mock tool without any prompt_id to exercise the path
	server.RegisterTool("mock", &mockNoPromptIDTool{})

	result := server.listTools()
	for _, tool := range result.Tools {
		for key, prop := range tool.InputSchema.Properties {
			if strings.Contains(strings.ToLower(key), "prompt_id") {
				t.Fatalf("Input schema contains forbidden key 'prompt_id' in property: %s", key)
			}
			if strings.Contains(strings.ToLower(prop.Description), "prompt_id") {
				t.Fatalf("Input schema description mentions 'prompt_id' in property: %s -> %s", key, prop.Description)
			}
		}
		for _, req := range tool.InputSchema.Required {
			if strings.EqualFold(req, "prompt_id") {
				t.Fatalf("Input schema marks forbidden field 'prompt_id' as required")
			}
		}
	}
}

// mockNoPromptIDTool is a local tool used only for this test to avoid cross-package imports
type mockNoPromptIDTool struct{}

func (m *mockNoPromptIDTool) Name() string        { return "mock" }
func (m *mockNoPromptIDTool) Description() string { return "mock tool" }
func (m *mockNoPromptIDTool) InputSchema() InputSchema {
	return InputSchema{Type: "object", Properties: map[string]PropertySchema{
		"foo": {Type: "string", Description: "a field"},
	}}
}
func (m *mockNoPromptIDTool) Call(_ context.Context, _ map[string]interface{}, _ *Server) (*ToolResult, error) {
	return &ToolResult{Content: []ContentItem{{Type: "text", Text: "ok"}}}, nil
}
