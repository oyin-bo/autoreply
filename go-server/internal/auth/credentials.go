// Package auth provides authentication and credential management
package auth

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/99designs/keyring"
)

// Credentials stores user authentication information
type Credentials struct {
	Handle       string    `json:"handle"`
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	DID          string    `json:"did"`
	TokenType    string    `json:"token_type,omitempty"`
	ExpiresAt    time.Time `json:"expires_at,omitempty"`
	Scope        string    `json:"scope,omitempty"`
}

// CredentialStore manages secure credential storage
type CredentialStore struct {
	ring keyring.Keyring
}

// NewCredentialStore creates a new credential store
func NewCredentialStore() (*CredentialStore, error) {
	// Try to use OS keyring with fallback to encrypted file
	ring, err := keyring.Open(keyring.Config{
		ServiceName:              "autoreply",
		KeychainName:             "autoreply",
		FileDir:                  filepath.Join(os.Getenv("HOME"), ".autoreply"),
		FilePasswordFunc:         keyring.FixedStringPrompt("autoreply-default-key"),
		AllowedBackends:          []keyring.BackendType{keyring.KeychainBackend, keyring.SecretServiceBackend, keyring.WinCredBackend, keyring.FileBackend},
		KeychainTrustApplication: true,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to open keyring: %w", err)
	}

	return &CredentialStore{ring: ring}, nil
}

// Save stores credentials for a handle
func (s *CredentialStore) Save(creds *Credentials) error {
	data, err := json.Marshal(creds)
	if err != nil {
		return fmt.Errorf("failed to marshal credentials: %w", err)
	}

	if err := s.ring.Set(keyring.Item{
		Key:  fmt.Sprintf("user:%s", creds.Handle),
		Data: data,
	}); err != nil {
		return fmt.Errorf("failed to save credentials: %w", err)
	}

	return nil
}

// Load retrieves credentials for a handle
func (s *CredentialStore) Load(handle string) (*Credentials, error) {
	item, err := s.ring.Get(fmt.Sprintf("user:%s", handle))
	if err != nil {
		if err == keyring.ErrKeyNotFound {
			return nil, fmt.Errorf("no credentials found for handle: %s", handle)
		}
		return nil, fmt.Errorf("failed to load credentials: %w", err)
	}

	var creds Credentials
	if err := json.Unmarshal(item.Data, &creds); err != nil {
		return nil, fmt.Errorf("failed to unmarshal credentials: %w", err)
	}

	return &creds, nil
}

// Delete removes credentials for a handle
func (s *CredentialStore) Delete(handle string) error {
	if err := s.ring.Remove(fmt.Sprintf("user:%s", handle)); err != nil {
		if err == keyring.ErrKeyNotFound {
			return fmt.Errorf("no credentials found for handle: %s", handle)
		}
		return fmt.Errorf("failed to delete credentials: %w", err)
	}
	return nil
}

// SetDefault sets the default handle
func (s *CredentialStore) SetDefault(handle string) error {
	if err := s.ring.Set(keyring.Item{
		Key:  "default_handle",
		Data: []byte(handle),
	}); err != nil {
		return fmt.Errorf("failed to set default handle: %w", err)
	}
	return nil
}

// GetDefault retrieves the default handle
func (s *CredentialStore) GetDefault() (string, error) {
	item, err := s.ring.Get("default_handle")
	if err != nil {
		if err == keyring.ErrKeyNotFound {
			return "", fmt.Errorf("no default handle set")
		}
		return "", fmt.Errorf("failed to get default handle: %w", err)
	}
	return string(item.Data), nil
}

// ListHandles returns all stored handles
func (s *CredentialStore) ListHandles() ([]string, error) {
	keys, err := s.ring.Keys()
	if err != nil {
		return nil, fmt.Errorf("failed to list keys: %w", err)
	}

	var handles []string
	for _, key := range keys {
		if len(key) > 5 && key[:5] == "user:" {
			handles = append(handles, key[5:])
		}
	}

	return handles, nil
}

// Session stores active AT Protocol session data
type Session struct {
	Handle       string    `json:"handle"`
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	DID          string    `json:"did"`
	ExpiresAt    time.Time `json:"expires_at,omitempty"`
}

// SaveSession stores a session for a handle
func (s *CredentialStore) SaveSession(session *Session) error {
	data, err := json.Marshal(session)
	if err != nil {
		return fmt.Errorf("failed to marshal session: %w", err)
	}

	sessionKey := fmt.Sprintf("session:%s", session.Handle)
	if err := s.ring.Set(keyring.Item{
		Key:  sessionKey,
		Data: data,
	}); err != nil {
		return fmt.Errorf("failed to save session: %w", err)
	}

	return nil
}

// LoadSession retrieves a session for a handle
func (s *CredentialStore) LoadSession(handle string) (*Session, error) {
	sessionKey := fmt.Sprintf("session:%s", handle)
	item, err := s.ring.Get(sessionKey)
	if err != nil {
		if err == keyring.ErrKeyNotFound {
			return nil, nil // No session found, not an error
		}
		return nil, fmt.Errorf("failed to load session: %w", err)
	}

	var session Session
	if err := json.Unmarshal(item.Data, &session); err != nil {
		return nil, fmt.Errorf("failed to unmarshal session: %w", err)
	}

	return &session, nil
}

// DeleteSession removes a session for a handle
func (s *CredentialStore) DeleteSession(handle string) error {
	sessionKey := fmt.Sprintf("session:%s", handle)
	if err := s.ring.Remove(sessionKey); err != nil {
		if err != keyring.ErrKeyNotFound {
			return fmt.Errorf("failed to delete session: %w", err)
		}
	}
	return nil
}
