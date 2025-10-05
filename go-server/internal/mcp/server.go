// Package mcp provides the MCP server implementation
package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"

	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// Tool represents a tool that can be called via MCP
type Tool interface {
	Name() string
	Description() string
	InputSchema() InputSchema
	Call(ctx context.Context, args map[string]interface{}, server *Server) (*ToolResult, error)
}

// ClientRPC provides the ability for server to send RPC requests to client
type ClientRPC interface {
	SendRequest(ctx context.Context, method string, params interface{}) (interface{}, error)
	SupportsElicitation() bool
	GetClientName() string
}

// Server represents an MCP server
type Server struct {
	tools            map[string]Tool
	clientInfo       *ClientInfo
	clientCapability *ClientCapabilities
}

// NewServer creates a new MCP server
func NewServer() (*Server, error) {
	return &Server{
		tools: make(map[string]Tool),
	}, nil
}

// RegisterTool registers a tool with the server
func (s *Server) RegisterTool(name string, tool Tool) {
	s.tools[name] = tool
	log.Printf("Registered tool: %s", name)
}

// RequestElicitation sends an elicitation/create request to the client
// Returns error if client doesn't support elicitation
func (s *Server) RequestElicitation(ctx context.Context, message string, schema map[string]interface{}) (*ElicitationResponse, error) {
	if s.clientCapability == nil || s.clientCapability.Elicitation == nil {
		return nil, fmt.Errorf("client does not support elicitation")
	}

	_ = ElicitationRequest{
		Message:         message,
		RequestedSchema: schema,
	}

	// This is a placeholder - actual implementation would need bidirectional transport
	// For now, we return an error to indicate elicitation is unavailable
	return nil, fmt.Errorf("elicitation transport not yet implemented")
}

// SupportsElicitation returns true if the connected client supports elicitation
func (s *Server) SupportsElicitation() bool {
	return s.clientCapability != nil && s.clientCapability.Elicitation != nil
}

// GetClientName returns the client name from initialization, or "Unknown Client"
func (s *Server) GetClientName() string {
	if s.clientInfo != nil && s.clientInfo.Name != "" {
		return s.clientInfo.Name
	}
	return "Unknown Client"
}

// ServeStdio starts the server in stdio mode
func (s *Server) ServeStdio(ctx context.Context) error {
	decoder := json.NewDecoder(os.Stdin)
	encoder := json.NewEncoder(os.Stdout)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		var request JSONRPCRequest
		if err := decoder.Decode(&request); err != nil {
			if err == io.EOF {
				return nil
			}
			log.Printf("Failed to decode request: %v", err)
			continue
		}

		response := s.handleRequest(ctx, &request)
		if err := encoder.Encode(response); err != nil {
			log.Printf("Failed to encode response: %v", err)
		}
	}
}

// handleRequest processes a JSON-RPC request
func (s *Server) handleRequest(ctx context.Context, req *JSONRPCRequest) *JSONRPCResponse {
	response := &JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      req.ID,
	}

	switch req.Method {
	case "initialize":
		var params InitializeParams
		if len(req.Params) > 0 {
			_ = json.Unmarshal(req.Params, &params)
		}

		// Store client info and capabilities
		s.clientInfo = params.ClientInfo
		s.clientCapability = params.Capabilities

		tools := s.listTools()
		response.Result = &InitializeResult{
			ServerInfo: ServerInfo{
				Name:    "autoreply",
				Version: "0.1.0",
			},
			Capabilities: Capabilities{
				Tools: ToolsCapability{List: true, Call: true},
			},
			Tools: tools.Tools,
		}
	case "tools/list":
		result := s.listTools()
		response.Result = result
	case "tools/call":
		result, err := s.callTool(ctx, req.Params)
		if err != nil {
			response.Error = &RPCError{Code: -32000, Message: err.Error(), Data: err}
		} else {
			response.Result = result
		}
	default:
		response.Error = &RPCError{Code: -32601, Message: fmt.Sprintf("Method not found: %s", req.Method)}
	}

	return response
}

// listTools returns information about all registered tools
func (s *Server) listTools() *ListToolsResult {
	tools := make([]ToolInfo, 0, len(s.tools))
	for _, tool := range s.tools {
		tools = append(tools, ToolInfo{
			Name:        tool.Name(),
			Description: tool.Description(),
			InputSchema: tool.InputSchema(),
		})
	}
	return &ListToolsResult{Tools: tools}
}

// callTool executes a tool call
func (s *Server) callTool(ctx context.Context, params json.RawMessage) (*ToolResult, error) {
	var toolParams ToolCallParams
	if err := json.Unmarshal(params, &toolParams); err != nil {
		return nil, errors.Wrap(err, errors.InvalidInput, "Failed to parse tool parameters")
	}

	tool, exists := s.tools[toolParams.Name]
	if !exists {
		return nil, errors.NewMCPError(errors.NotFound, fmt.Sprintf("Tool not found: %s", toolParams.Name))
	}

	return tool.Call(ctx, toolParams.Arguments, s)
}
