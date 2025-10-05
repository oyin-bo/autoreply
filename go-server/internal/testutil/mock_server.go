// Package testutil provides testing utilities and mocks
package testutil

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// MockServer is a mock implementation of MCP Server for testing
type MockServer struct {
	mu                  sync.Mutex
	clientInfo          *mcp.ClientInfo
	clientCapability    *mcp.ClientCapabilities
	elicitationRequests []ElicitationRequest
	ElicitationFn       func(ctx context.Context, message string, schema map[string]interface{}) (*mcp.ElicitationResponse, error)
}

// ElicitationRequest records an elicitation request
type ElicitationRequest struct {
	Message string
	Schema  map[string]interface{}
}

// NewMockServer creates a new mock MCP server
func NewMockServer() *MockServer {
	return &MockServer{
		elicitationRequests: []ElicitationRequest{},
	}
}

// NewMockServerWithElicitation creates a mock server that supports elicitation
func NewMockServerWithElicitation(clientName string) *MockServer {
	return &MockServer{
		clientInfo: &mcp.ClientInfo{
			Name:    clientName,
			Version: "1.0.0",
		},
		clientCapability: &mcp.ClientCapabilities{
			Elicitation: &mcp.ElicitationCapability{},
		},
		elicitationRequests: []ElicitationRequest{},
	}
}

// NewMockServerWithoutElicitation creates a mock server without elicitation support
func NewMockServerWithoutElicitation(clientName string) *MockServer {
	return &MockServer{
		clientInfo: &mcp.ClientInfo{
			Name:    clientName,
			Version: "1.0.0",
		},
		clientCapability: &mcp.ClientCapabilities{
			Elicitation: nil, // No elicitation support
		},
		elicitationRequests: []ElicitationRequest{},
	}
}

// RequestElicitation mocks elicitation requests
func (m *MockServer) RequestElicitation(ctx context.Context, message string, schema map[string]interface{}) (*mcp.ElicitationResponse, error) {
	m.mu.Lock()
	m.elicitationRequests = append(m.elicitationRequests, ElicitationRequest{
		Message: message,
		Schema:  schema,
	})
	m.mu.Unlock()

	if m.ElicitationFn != nil {
		return m.ElicitationFn(ctx, message, schema)
	}

	// Default: client doesn't support elicitation
	return nil, fmt.Errorf("client does not support elicitation")
}

// SupportsElicitation returns whether the client supports elicitation
func (m *MockServer) SupportsElicitation() bool {
	return m.clientCapability != nil && m.clientCapability.Elicitation != nil
}

// GetClientName returns the client name
func (m *MockServer) GetClientName() string {
	if m.clientInfo != nil && m.clientInfo.Name != "" {
		return m.clientInfo.Name
	}
	return "Unknown Client"
}

// GetElicitationRequests returns all elicitation requests
func (m *MockServer) GetElicitationRequests() []ElicitationRequest {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]ElicitationRequest{}, m.elicitationRequests...)
}

// SetElicitationResponse configures the mock to return a specific response
func (m *MockServer) SetElicitationResponse(action string, content map[string]interface{}) {
	m.ElicitationFn = func(ctx context.Context, message string, schema map[string]interface{}) (*mcp.ElicitationResponse, error) {
		return &mcp.ElicitationResponse{
			Action:  action,
			Content: content,
		}, nil
	}
}

// SetElicitationError configures the mock to return an error
func (m *MockServer) SetElicitationError(err error) {
	m.ElicitationFn = func(ctx context.Context, message string, schema map[string]interface{}) (*mcp.ElicitationResponse, error) {
		return nil, err
	}
}

// AssertToolResult is a helper to validate ToolResult structure
func AssertToolResult(t interface{ Fatalf(string, ...interface{}) }, result *mcp.ToolResult, expectedType string, expectedTextContains string) {
	if result == nil {
		t.Fatalf("ToolResult is nil")
	}

	if len(result.Content) == 0 {
		t.Fatalf("ToolResult has no content items")
	}

	firstContent := result.Content[0]
	if firstContent.Type != expectedType {
		t.Fatalf("Expected content type '%s', got '%s'", expectedType, firstContent.Type)
	}

	if expectedTextContains != "" && !contains(firstContent.Text, expectedTextContains) {
		t.Fatalf("Expected content to contain '%s', got: %s", expectedTextContains, firstContent.Text)
	}
}

// AssertElicitationMetadata validates elicitation metadata
func AssertElicitationMetadata(t interface{ Fatalf(string, ...interface{}) }, metadata json.RawMessage, expectedField string) {
	if len(metadata) == 0 {
		t.Fatalf("Metadata is empty")
	}

	var meta map[string]interface{}
	if err := json.Unmarshal(metadata, &meta); err != nil {
		t.Fatalf("Failed to parse metadata: %v", err)
	}

	field, ok := meta["field"]
	if !ok {
		t.Fatalf("Metadata missing 'field' property")
	}

	if field.(string) != expectedField {
		t.Fatalf("Expected field '%s', got '%s'", expectedField, field)
	}

	if _, ok := meta["prompt_id"]; !ok {
		t.Fatalf("Metadata missing 'prompt_id' property")
	}
}

// contains is a simple substring checker
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && (s[0:len(substr)] == substr || contains(s[1:], substr))))
}
