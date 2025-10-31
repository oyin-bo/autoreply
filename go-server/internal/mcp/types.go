// Package mcp provides JSON-RPC 2.0 protocol types for Model Context Protocol
package mcp

import "encoding/json"

// JSONRPCRequest represents a JSON-RPC 2.0 request
type JSONRPCRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      interface{}     `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

// JSONRPCResponse represents a JSON-RPC 2.0 response
type JSONRPCResponse struct {
	JSONRPC string      `json:"jsonrpc"`
	ID      interface{} `json:"id"`
	Result  interface{} `json:"result,omitempty"`
	Error   *RPCError   `json:"error,omitempty"`
}

// RPCError represents a JSON-RPC 2.0 error
type RPCError struct {
	Code    int         `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

// ToolCallParams represents the parameters for a tools/call request
type ToolCallParams struct {
	Name      string                 `json:"name"`
	Arguments map[string]interface{} `json:"arguments"`
}

// ToolResult represents the result of a tool call
type ToolResult struct {
	Content []ContentItem `json:"content"`
	IsError bool          `json:"isError,omitempty"`
}

// ContentItem represents a content item in a tool result
type ContentItem struct {
	Type string `json:"type"`
	Text string `json:"text"`
	// Metadata holds optional, tool-specific JSON metadata for the content item
	// This field is optional and preserved as raw JSON so MCP clients can parse it as needed.
	Metadata json.RawMessage `json:"metadata,omitempty"`
}

// ListToolsResult represents the result of a tools/list request
type ListToolsResult struct {
	Tools []ToolInfo `json:"tools"`
}

// ToolInfo represents information about a tool
type ToolInfo struct {
	Name        string      `json:"name"`
	Description string      `json:"description"`
	InputSchema InputSchema `json:"inputSchema"`
}

// InputSchema represents the JSON schema for tool input
type InputSchema struct {
	Type       string                    `json:"type"`
	Properties map[string]PropertySchema `json:"properties"`
	Required   []string                  `json:"required,omitempty"`
}

// PropertySchema represents a property in the input schema
type PropertySchema struct {
	Type        string `json:"type"`
	Description string `json:"description,omitempty"`
	// Items describes the element type when Type is "array"; omitted otherwise
	Items *PropertySchema `json:"items,omitempty"`
}

// InitializeParams represents params accepted by the initialize method
type InitializeParams struct {
	ClientInfo   *ClientInfo         `json:"clientInfo,omitempty"`
	Capabilities *ClientCapabilities `json:"capabilities,omitempty"`
}

// ClientInfo describes the client calling the server
type ClientInfo struct {
	Name    string `json:"name,omitempty"`
	Version string `json:"version,omitempty"`
}

// ClientCapabilities describes what features the client supports
type ClientCapabilities struct {
	Elicitation *ElicitationCapability `json:"elicitation,omitempty"`
}

// ElicitationCapability indicates client supports elicitation/create requests
type ElicitationCapability struct {
	// Empty struct - presence indicates support
}

// ElicitationRequest represents an elicitation/create request from server to client
type ElicitationRequest struct {
	Message         string                 `json:"message"`
	RequestedSchema map[string]interface{} `json:"requestedSchema"`
}

// ElicitationResponse represents the client's response to elicitation/create
type ElicitationResponse struct {
	Action  string                 `json:"action"` // "accept", "decline", "cancel"
	Content map[string]interface{} `json:"content,omitempty"`
}

// InitializeResult is returned from initialize
type InitializeResult struct {
	// ProtocolVersion indicates the server's MCP protocol version.
	// Standard MCP protocol version is "2024-11-05".
	ProtocolVersion string `json:"protocolVersion"`
	ServerInfo   ServerInfo   `json:"serverInfo"`
	Capabilities Capabilities `json:"capabilities"`
	Tools        []ToolInfo   `json:"tools,omitempty"`
}

// ServerInfo describes this MCP server
type ServerInfo struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// Capabilities advertises supported features
type Capabilities struct {
	Tools ToolsCapability `json:"tools"`
}

// ToolsCapability indicates what tool operations are supported
type ToolsCapability struct {
	List bool `json:"list"`
	Call bool `json:"call"`
}
