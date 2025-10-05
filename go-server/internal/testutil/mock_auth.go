// Package testutil provides testing utilities and mocks
package testutil

import (
	"context"
	"fmt"
	"sync"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
)

// MockSessionManager is a mock implementation of SessionManager for testing
type MockSessionManager struct {
	mu              sync.Mutex
	CreateSessionFn func(ctx context.Context, handle, password string) (*auth.Credentials, error)
	RefreshTokenFn  func(ctx context.Context, creds *auth.Credentials) (*auth.Credentials, error)
	createCalls     []CreateSessionCall
	refreshCalls    []RefreshTokenCall
}

// CreateSessionCall records a call to CreateSession
type CreateSessionCall struct {
	Handle   string
	Password string
}

// RefreshTokenCall records a call to RefreshToken
type RefreshTokenCall struct {
	Creds *auth.Credentials
}

// NewMockSessionManager creates a new mock session manager
func NewMockSessionManager() *MockSessionManager {
	return &MockSessionManager{
		createCalls:  []CreateSessionCall{},
		refreshCalls: []RefreshTokenCall{},
	}
}

// CreateSession mocks session creation
func (m *MockSessionManager) CreateSession(ctx context.Context, handle, password string) (*auth.Credentials, error) {
	m.mu.Lock()
	m.createCalls = append(m.createCalls, CreateSessionCall{Handle: handle, Password: password})
	m.mu.Unlock()

	if m.CreateSessionFn != nil {
		return m.CreateSessionFn(ctx, handle, password)
	}

	// Default success behavior
	return &auth.Credentials{
		Handle:       handle,
		DID:          "did:plc:test123456789",
		AccessToken:  "mock-access-token",
		RefreshToken: "mock-refresh-token",
	}, nil
}

// RefreshToken mocks token refresh
func (m *MockSessionManager) RefreshToken(ctx context.Context, creds *auth.Credentials) (*auth.Credentials, error) {
	m.mu.Lock()
	m.refreshCalls = append(m.refreshCalls, RefreshTokenCall{Creds: creds})
	m.mu.Unlock()

	if m.RefreshTokenFn != nil {
		return m.RefreshTokenFn(ctx, creds)
	}

	// Default success behavior
	return creds, nil
}

// GetCreateCalls returns all calls to CreateSession
func (m *MockSessionManager) GetCreateCalls() []CreateSessionCall {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]CreateSessionCall{}, m.createCalls...)
}

// GetRefreshCalls returns all calls to RefreshToken
func (m *MockSessionManager) GetRefreshCalls() []RefreshTokenCall {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]RefreshTokenCall{}, m.refreshCalls...)
}

// MockCredentialStore is a mock implementation of CredentialStore for testing
type MockCredentialStore struct {
	mu          sync.Mutex
	credentials map[string]*auth.Credentials
	defaultKey  string
	SaveFn      func(*auth.Credentials) error
	LoadFn      func(string) (*auth.Credentials, error)
	DeleteFn    func(string) error
}

// NewMockCredentialStore creates a new mock credential store
func NewMockCredentialStore() *MockCredentialStore {
	return &MockCredentialStore{
		credentials: make(map[string]*auth.Credentials),
	}
}

// Save mocks saving credentials
func (m *MockCredentialStore) Save(creds *auth.Credentials) error {
	if m.SaveFn != nil {
		return m.SaveFn(creds)
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	m.credentials[creds.Handle] = creds
	return nil
}

// Load mocks loading credentials
func (m *MockCredentialStore) Load(handle string) (*auth.Credentials, error) {
	if m.LoadFn != nil {
		return m.LoadFn(handle)
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	creds, ok := m.credentials[handle]
	if !ok {
		return nil, fmt.Errorf("credentials not found for handle: %s", handle)
	}
	return creds, nil
}

// Delete mocks deleting credentials
func (m *MockCredentialStore) Delete(handle string) error {
	if m.DeleteFn != nil {
		return m.DeleteFn(handle)
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.credentials, handle)
	return nil
}

// ListHandles returns all stored handles
func (m *MockCredentialStore) ListHandles() ([]string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	handles := make([]string, 0, len(m.credentials))
	for h := range m.credentials {
		handles = append(handles, h)
	}
	return handles, nil
}

// SetDefault sets the default handle
func (m *MockCredentialStore) SetDefault(handle string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.defaultKey = handle
	return nil
}

// GetDefault returns the default handle
func (m *MockCredentialStore) GetDefault() (string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.defaultKey == "" {
		return "", fmt.Errorf("no default handle set")
	}
	return m.defaultKey, nil
}

// AddTestCredentials adds test credentials to the store
func (m *MockCredentialStore) AddTestCredentials(handle string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.credentials[handle] = &auth.Credentials{
		Handle:       handle,
		DID:          "did:plc:test" + handle,
		AccessToken:  "token-" + handle,
		RefreshToken: "refresh-" + handle,
	}
}
