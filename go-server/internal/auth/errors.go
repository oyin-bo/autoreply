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
	
	// ErrCodeAuthorizationPending indicates device authorization is still pending
	ErrCodeAuthorizationPending ErrorCode = "authorization_pending"
	
	// ErrCodeSlowDown indicates polling too frequently
	ErrCodeSlowDown ErrorCode = "slow_down"
	
	// ErrCodeExpiredToken indicates the device code has expired
	ErrCodeExpiredToken ErrorCode = "expired_token"
	
	// ErrCodeAccessDenied indicates the user denied authorization
	ErrCodeAccessDenied ErrorCode = "access_denied"
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

// Sentinel errors for device flow polling
var (
	// ErrAuthorizationPending is returned when device authorization is still pending
	ErrAuthorizationPending = NewAuthError(ErrCodeAuthorizationPending, "authorization pending", nil)
	
	// ErrSlowDown is returned when polling too frequently
	ErrSlowDown = NewAuthError(ErrCodeSlowDown, "slow down", nil)
	
	// ErrExpiredToken is returned when the device code has expired
	ErrExpiredToken = NewAuthError(ErrCodeExpiredToken, "device code expired", nil)
	
	// ErrAccessDenied is returned when the user denies authorization
	ErrAccessDenied = NewAuthError(ErrCodeAccessDenied, "access denied", nil)
)
