// Package mcp provides the MCP server implementation
package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"sync"
	"sync/atomic"

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

	// For bidirectional RPC
	encoder          *json.Encoder
	encoderMu        sync.Mutex
	nextRequestID    int64
	pendingResponses map[interface{}]chan *JSONRPCResponse
	pendingMu        sync.Mutex
}

// NewServer creates a new MCP server
func NewServer() (*Server, error) {
	return &Server{
		tools:            make(map[string]Tool),
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
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

	if s.encoder == nil {
		return nil, fmt.Errorf("server not initialized with stdio transport")
	}

	// Generate unique request ID
	requestID := atomic.AddInt64(&s.nextRequestID, 1)

	// Create response channel
	responseChan := make(chan *JSONRPCResponse, 1)
	s.pendingMu.Lock()
	s.pendingResponses[requestID] = responseChan
	s.pendingMu.Unlock()

	// Cleanup on exit
	defer func() {
		s.pendingMu.Lock()
		delete(s.pendingResponses, requestID)
		s.pendingMu.Unlock()
	}()

	// Send elicitation/create request
	request := &JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      requestID,
		Method:  "elicitation/create",
		Params: func() json.RawMessage {
			params := ElicitationRequest{
				Message:         message,
				RequestedSchema: schema,
			}
			data, _ := json.Marshal(params)
			return data
		}(),
	}

	s.encoderMu.Lock()
	err := s.encoder.Encode(request)
	s.encoderMu.Unlock()

	if err != nil {
		return nil, fmt.Errorf("failed to send elicitation request: %w", err)
	}

	log.Printf("Sent elicitation/create request ID=%v", requestID)

	// Wait for response or context cancellation
	select {
	case response := <-responseChan:
		if response.Error != nil {
			return nil, fmt.Errorf("elicitation error: %s", response.Error.Message)
		}

		// Parse the response
		var elicitationResp ElicitationResponse
		responseBytes, _ := json.Marshal(response.Result)
		if err := json.Unmarshal(responseBytes, &elicitationResp); err != nil {
			return nil, fmt.Errorf("failed to parse elicitation response: %w", err)
		}

		return &elicitationResp, nil

	case <-ctx.Done():
		return nil, fmt.Errorf("elicitation cancelled: %w", ctx.Err())
	}
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
	s.encoder = json.NewEncoder(os.Stdout)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		var rawMsg json.RawMessage
		if err := decoder.Decode(&rawMsg); err != nil {
			if err == io.EOF {
				return nil
			}
			log.Printf("Failed to decode message: %v", err)
			continue
		}

		// Try to parse as response first (has "result" or "error" field)
		var possibleResponse JSONRPCResponse
		if err := json.Unmarshal(rawMsg, &possibleResponse); err == nil && possibleResponse.ID != nil && (possibleResponse.Result != nil || possibleResponse.Error != nil) {
			// This is a response to one of our requests
			s.handleClientResponse(&possibleResponse)
			continue
		}

		// Otherwise it's a request from client
		var request JSONRPCRequest
		if err := json.Unmarshal(rawMsg, &request); err != nil {
			log.Printf("Failed to parse as request: %v", err)
			continue
		}

		response := s.handleRequest(ctx, &request)

		s.encoderMu.Lock()
		err := s.encoder.Encode(response)
		s.encoderMu.Unlock()

		if err != nil {
			log.Printf("Failed to encode response: %v", err)
		}
	}
}

// handleClientResponse delivers a response from client to waiting goroutine
func (s *Server) handleClientResponse(response *JSONRPCResponse) {
	s.pendingMu.Lock()
	responseChan, exists := s.pendingResponses[response.ID]
	s.pendingMu.Unlock()

	if exists {
		responseChan <- response
		log.Printf("Delivered response for request ID=%v", response.ID)
	} else {
		log.Printf("Warning: Received response for unknown request ID=%v", response.ID)
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
