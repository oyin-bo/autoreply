// Package errors provides error types and utilities for the autoreply MCP server
package errors

import "fmt"

// ErrorCode represents the type of error for MCP protocol
type ErrorCode string

const (
	InvalidInput     ErrorCode = "invalid_input"
	DIDResolveFailed ErrorCode = "did_resolve_failed"
	RepoFetchFailed  ErrorCode = "repo_fetch_failed"
	RepoParseFailed  ErrorCode = "repo_parse_failed"
	NotFound         ErrorCode = "not_found"
	Timeout          ErrorCode = "timeout"
	CacheError       ErrorCode = "cache_error"
	InternalError    ErrorCode = "internal_error"
)

// MCPError represents an error that can be returned via the MCP protocol
type MCPError struct {
	Code    ErrorCode   `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

// Error implements the error interface
func (e *MCPError) Error() string {
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// NewMCPError creates a new MCP error
func NewMCPError(code ErrorCode, message string) *MCPError {
	return &MCPError{
		Code:    code,
		Message: message,
	}
}

// NewMCPErrorWithData creates a new MCP error with additional data
func NewMCPErrorWithData(code ErrorCode, message string, data interface{}) *MCPError {
	return &MCPError{
		Code:    code,
		Message: message,
		Data:    data,
	}
}

// Wrap wraps an existing error with MCP error context
func Wrap(err error, code ErrorCode, message string) *MCPError {
	return &MCPError{
		Code:    code,
		Message: fmt.Sprintf("%s: %v", message, err),
	}
}
