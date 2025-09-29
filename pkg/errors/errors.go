// errors.go - Error types and utilities for BlueSky MCP Server
package errors

import (
	"fmt"
	"regexp"
	"strings"
)

// ErrorCode represents standardized error codes for MCP responses
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

// McpError represents an MCP protocol error
type McpError struct {
	Code    ErrorCode   `json:"code"`
	Message string      `json:"message"`
	Data    interface{} `json:"data,omitempty"`
}

func (e *McpError) Error() string {
	return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

// NewMcpError creates a new MCP error
func NewMcpError(code ErrorCode, message string) *McpError {
	return &McpError{
		Code:    code,
		Message: message,
	}
}

// NewMcpErrorWithData creates a new MCP error with additional data
func NewMcpErrorWithData(code ErrorCode, message string, data interface{}) *McpError {
	return &McpError{
		Code:    code,
		Message: message,
		Data:    data,
	}
}

// Input validation utilities

var (
	// Handle format validation - name.host.tld
	handleRegex = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9.-]*\.[a-zA-Z]{2,}$`)
	
	// DID format validation - did:plc:[a-z2-7]{24}
	didRegex = regexp.MustCompile(`^did:plc:[a-z2-7]{24}$`)
)

// ValidateAccount validates an account parameter (handle or DID)
func ValidateAccount(account string) error {
	if account == "" {
		return NewMcpError(InvalidInput, "Account parameter is required")
	}

	account = strings.TrimSpace(account)
	
	// Check if it's a DID
	if strings.HasPrefix(account, "did:plc:") {
		if !didRegex.MatchString(account) {
			return NewMcpError(InvalidInput, "Invalid DID format. Expected did:plc:[a-z2-7]{24}")
		}
		return nil
	}

	// Check if it's a handle
	if !handleRegex.MatchString(account) {
		return NewMcpError(InvalidInput, "Invalid handle format. Expected name.host.tld")
	}

	return nil
}

// ValidateQuery validates a search query parameter
func ValidateQuery(query string) error {
	if query == "" {
		return NewMcpError(InvalidInput, "Query parameter is required")
	}

	query = strings.TrimSpace(query)
	
	if len(query) > 500 {
		return NewMcpError(InvalidInput, "Query too long. Maximum 500 characters allowed")
	}

	return nil
}