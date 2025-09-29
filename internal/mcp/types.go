// types.go - MCP protocol types
package mcp

import (
	"encoding/json"
	"github.com/oyin-bo/autoreply/pkg/errors"
)

// Request represents a JSON-RPC 2.0 MCP request
type Request struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

// Response represents a JSON-RPC 2.0 MCP response
type Response struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Result  interface{}     `json:"result,omitempty"`
	Error   *errors.McpError `json:"error,omitempty"`
}

// ToolCallParams represents parameters for tools/call method
type ToolCallParams struct {
	Name      string          `json:"name"`
	Arguments json.RawMessage `json:"arguments"`
}

// ToolsListResult represents the result of tools/list
type ToolsListResult struct {
	Tools interface{} `json:"tools"`
}

// InitializeParams represents parameters for initialize method
type InitializeParams struct {
	ProtocolVersion string      `json:"protocolVersion,omitempty"`
	Capabilities    interface{} `json:"capabilities,omitempty"`
	ClientInfo      interface{} `json:"clientInfo,omitempty"`
}

// InitializeResult represents the result of initialize
type InitializeResult struct {
	ProtocolVersion string          `json:"protocolVersion"`
	Capabilities    ServerCapabilities `json:"capabilities"`
	ServerInfo      ServerInfo      `json:"serverInfo"`
}

// ServerCapabilities represents server capabilities
type ServerCapabilities struct {
	Tools interface{} `json:"tools,omitempty"`
}

// ServerInfo represents server information
type ServerInfo struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// NewSuccessResponse creates a successful MCP response
func NewSuccessResponse(id json.RawMessage, result interface{}) *Response {
	return &Response{
		JSONRPC: "2.0",
		ID:      id,
		Result:  result,
	}
}

// NewErrorResponse creates an error MCP response
func NewErrorResponse(id json.RawMessage, err *errors.McpError) *Response {
	return &Response{
		JSONRPC: "2.0",
		ID:      id,
		Error:   err,
	}
}