// server.go - MCP protocol server implementation
package mcp

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"

	"github.com/oyin-bo/autoreply/internal/tools"
	"github.com/oyin-bo/autoreply/pkg/errors"
)

// Server implements the MCP protocol server
type Server struct {
	tools *tools.Manager
}

// NewServer creates a new MCP server
func NewServer() *Server {
	return &Server{
		tools: tools.NewManager(),
	}
}

// RunStdio runs the server in stdio mode
func (s *Server) RunStdio(ctx context.Context) error {
	scanner := bufio.NewScanner(os.Stdin)
	
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if !scanner.Scan() {
			if err := scanner.Err(); err != nil {
				return fmt.Errorf("failed to read from stdin: %w", err)
			}
			// EOF reached
			return nil
		}

		line := scanner.Text()
		if line == "" {
			continue
		}

		if err := s.handleRequest(ctx, line); err != nil {
			log.Printf("Error handling request: %v", err)
		}
	}
}

// handleRequest processes a single MCP request
func (s *Server) handleRequest(ctx context.Context, line string) error {
	var req Request
	if err := json.Unmarshal([]byte(line), &req); err != nil {
		// Send parse error response with null ID
		resp := NewErrorResponse(nil, errors.NewMcpError(errors.InvalidInput, "Invalid JSON"))
		return s.sendResponse(resp)
	}

	resp := s.dispatch(ctx, &req)
	return s.sendResponse(resp)
}

// sendResponse sends a response to stdout
func (s *Server) sendResponse(resp *Response) error {
	data, err := json.Marshal(resp)
	if err != nil {
		return fmt.Errorf("failed to marshal response: %w", err)
	}

	_, err = fmt.Fprintln(os.Stdout, string(data))
	return err
}

// dispatch routes requests to appropriate handlers
func (s *Server) dispatch(ctx context.Context, req *Request) *Response {
	switch req.Method {
	case "initialize":
		return s.handleInitialize(req)
	case "notifications/initialized":
		return s.handleInitialized(req)
	case "tools/list":
		return s.handleToolsList(req)
	case "tools/call":
		return s.handleToolsCall(ctx, req)
	default:
		return NewErrorResponse(req.ID, errors.NewMcpError(errors.InvalidInput, fmt.Sprintf("Method '%s' not found", req.Method)))
	}
}

// handleInitialize handles the initialize method
func (s *Server) handleInitialize(req *Request) *Response {
	result := InitializeResult{
		ProtocolVersion: "2024-11-05",
		Capabilities: ServerCapabilities{
			Tools: struct{}{},
		},
		ServerInfo: ServerInfo{
			Name:    "bluesky-mcp-go",
			Version: "0.1.0",
		},
	}
	return NewSuccessResponse(req.ID, result)
}

// handleInitialized handles the notifications/initialized notification
func (s *Server) handleInitialized(req *Request) *Response {
	// This is a notification, no response needed
	return nil
}

// handleToolsList handles the tools/list method
func (s *Server) handleToolsList(req *Request) *Response {
	tools := s.tools.ListTools()
	result := ToolsListResult{Tools: tools}
	return NewSuccessResponse(req.ID, result)
}

// handleToolsCall handles the tools/call method
func (s *Server) handleToolsCall(ctx context.Context, req *Request) *Response {
	var params ToolCallParams
	if err := json.Unmarshal(req.Params, &params); err != nil {
		return NewErrorResponse(req.ID, errors.NewMcpError(errors.InvalidInput, "Invalid tool call parameters"))
	}

	result, err := s.tools.CallTool(ctx, params.Name, params.Arguments)
	if err != nil {
		if mcpErr, ok := err.(*errors.McpError); ok {
			return NewErrorResponse(req.ID, mcpErr)
		}
		return NewErrorResponse(req.ID, errors.NewMcpError(errors.InternalError, err.Error()))
	}

	return NewSuccessResponse(req.ID, result)
}