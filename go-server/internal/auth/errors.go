package auth

import "fmt"

// ErrorCode represents an authentication error code
type ErrorCode string

const (
	// ErrCodeKeyringUnavailable indicates the OS keyring is not available
	ErrCodeKeyringUnavailable ErrorCode = "keyring_unavailable"
	
	// ErrCodeInvalidCredentials indicates invalid credentials were provided
	ErrCodeInvalidCredentials ErrorCode = "invalid_credentials"
	
	// ErrCodeOAuthFailed indicates OAuth flow failed
	ErrCodeOAuthFailed ErrorCode = "oauth_failed"
	
	// ErrCodeNetworkError indicates a network error occurred
	ErrCodeNetworkError ErrorCode = "network_error"
	
	// ErrCodeStorageError indicates a storage operation failed
	ErrCodeStorageError ErrorCode = "storage_error"
	
	// ErrCodeAuthRequired indicates authentication is required
	ErrCodeAuthRequired ErrorCode = "auth_required"
	
	// ErrCodeAuthExpired indicates token expired and refresh failed
	ErrCodeAuthExpired ErrorCode = "auth_expired"
)

// AuthError represents an authentication error
type AuthError struct {
	Code    ErrorCode
	Message string
	Err     error
}

// Error implements the error interface
func (e *AuthError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("%s: %s: %v", e.Code, e.Message, e.Err)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// Unwrap returns the wrapped error
func (e *AuthError) Unwrap() error {
	return e.Err
}

// NewAuthError creates a new authentication error
func NewAuthError(code ErrorCode, message string, err error) *AuthError {
	return &AuthError{
		Code:    code,
		Message: message,
		Err:     err,
	}
}
