package auth

import (
	"fmt"

	"github.com/zalando/go-keyring"
)

const (
	// ServiceName is the service name used for keyring storage
	ServiceName = "autoreply-mcp"
)

// KeyringBackend provides OS keyring storage for credentials
type KeyringBackend struct {
	serviceName string
}

// NewKeyringBackend creates a new keyring backend
func NewKeyringBackend() *KeyringBackend {
	return &KeyringBackend{
		serviceName: ServiceName,
	}
}

// Store stores a value in the keyring
func (kb *KeyringBackend) Store(account, key, value string) error {
	fullKey := fmt.Sprintf("%s/%s", account, key)
	return keyring.Set(kb.serviceName, fullKey, value)
}

// Get retrieves a value from the keyring
func (kb *KeyringBackend) Get(account, key string) (string, error) {
	fullKey := fmt.Sprintf("%s/%s", account, key)
	return keyring.Get(kb.serviceName, fullKey)
}

// Delete removes a value from the keyring
func (kb *KeyringBackend) Delete(account, key string) error {
	fullKey := fmt.Sprintf("%s/%s", account, key)
	return keyring.Delete(kb.serviceName, fullKey)
}

// StoreCredentials stores all credentials for an account in the keyring
func (kb *KeyringBackend) StoreCredentials(account string, creds *Credentials) error {
	if err := kb.Store(account, "access_token", creds.AccessToken); err != nil {
		return fmt.Errorf("failed to store access token: %w", err)
	}
	
	if err := kb.Store(account, "refresh_token", creds.RefreshToken); err != nil {
		return fmt.Errorf("failed to store refresh token: %w", err)
	}
	
	if err := kb.Store(account, "dpop_key", creds.DPoPKey); err != nil {
		return fmt.Errorf("failed to store DPoP key: %w", err)
	}
	
	return nil
}

// GetCredentials retrieves all credentials for an account from the keyring
func (kb *KeyringBackend) GetCredentials(account string) (*Credentials, error) {
	accessToken, err := kb.Get(account, "access_token")
	if err != nil {
		return nil, fmt.Errorf("failed to get access token: %w", err)
	}
	
	refreshToken, err := kb.Get(account, "refresh_token")
	if err != nil {
		return nil, fmt.Errorf("failed to get refresh token: %w", err)
	}
	
	dpopKey, err := kb.Get(account, "dpop_key")
	if err != nil {
		return nil, fmt.Errorf("failed to get DPoP key: %w", err)
	}
	
	creds := &Credentials{
		AccessToken:  accessToken,
		RefreshToken: refreshToken,
		DPoPKey:      dpopKey,
	}
	
	return creds, nil
}

// DeleteCredentials removes all credentials for an account from the keyring
func (kb *KeyringBackend) DeleteCredentials(account string) error {
	// Try to delete all keys, collecting errors
	var errs []error
	
	if err := kb.Delete(account, "access_token"); err != nil {
		errs = append(errs, fmt.Errorf("access_token: %w", err))
	}
	
	if err := kb.Delete(account, "refresh_token"); err != nil {
		errs = append(errs, fmt.Errorf("refresh_token: %w", err))
	}
	
	if err := kb.Delete(account, "dpop_key"); err != nil {
		errs = append(errs, fmt.Errorf("dpop_key: %w", err))
	}
	
	if len(errs) > 0 {
		return fmt.Errorf("failed to delete some credentials: %v", errs)
	}
	
	return nil
}

// IsAvailable checks if the OS keyring is available
func (kb *KeyringBackend) IsAvailable() bool {
	// Try to perform a test operation
	testKey := "_test_availability"
	testValue := "test"
	
	err := keyring.Set(kb.serviceName, testKey, testValue)
	if err != nil {
		return false
	}
	
	// Clean up test entry
	_ = keyring.Delete(kb.serviceName, testKey)
	return true
}
