// Package auth provides authentication and credential management
package auth

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/99designs/keyring"
)

// fakeKeyring is a minimal in-memory keyring used only in tests.
type fakeKeyring struct {
	store map[string]keyring.Item
}

func newFakeKeyring() *fakeKeyring { return &fakeKeyring{store: make(map[string]keyring.Item)} }

func (f *fakeKeyring) Set(item keyring.Item) error {
	f.store[item.Key] = item
	return nil
}

func (f *fakeKeyring) Get(key string) (keyring.Item, error) {
	it, ok := f.store[key]
	if !ok {
		return keyring.Item{}, keyring.ErrKeyNotFound
	}
	return it, nil
}

func (f *fakeKeyring) Remove(key string) error {
	if _, ok := f.store[key]; !ok {
		return keyring.ErrKeyNotFound
	}
	delete(f.store, key)
	return nil
}

func (f *fakeKeyring) Keys() ([]string, error) {
	var keys []string
	for k := range f.store {
		keys = append(keys, k)
	}
	return keys, nil
}

// GetMetadata satisfies the keyring.Keyring interface but is unused in tests.
func (f *fakeKeyring) GetMetadata(service string) (keyring.Metadata, error) {
	_ = service
	return keyring.Metadata{}, nil
}

// createInMemoryStore returns a CredentialStore backed by an in-memory keyring
// so tests are fully isolated and deterministic.
func createInMemoryStore(t *testing.T) *CredentialStore {
	t.Helper()
	return &CredentialStore{ring: newFakeKeyring()}
}

func TestCredentialStore(t *testing.T) {
	// Create a temporary directory for test credentials
	tmpDir, err := os.MkdirTemp("", "autoreply-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	// Set HOME for the test
	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpDir)
	defer os.Setenv("HOME", oldHome)

	// Create credential store (in-memory for tests)
	store := createInMemoryStore(t)

	// Test save and load
	testCreds := &Credentials{
		Handle:       "alice.bsky.social",
		AccessToken:  "test-access-token",
		RefreshToken: "test-refresh-token",
		DID:          "did:plc:test123",
	}

	err = store.Save(testCreds)
	if err != nil {
		t.Fatalf("Failed to save credentials: %v", err)
	}

	loaded, err := store.Load("alice.bsky.social")
	if err != nil {
		t.Fatalf("Failed to load credentials: %v", err)
	}

	if loaded.Handle != testCreds.Handle {
		t.Errorf("Expected handle %s, got %s", testCreds.Handle, loaded.Handle)
	}
	if loaded.AccessToken != testCreds.AccessToken {
		t.Errorf("Expected access token %s, got %s", testCreds.AccessToken, loaded.AccessToken)
	}
	if loaded.RefreshToken != testCreds.RefreshToken {
		t.Errorf("Expected refresh token %s, got %s", testCreds.RefreshToken, loaded.RefreshToken)
	}
	if loaded.DID != testCreds.DID {
		t.Errorf("Expected DID %s, got %s", testCreds.DID, loaded.DID)
	}
}

func TestDefaultHandle(t *testing.T) {
	// Create a temporary directory for test credentials
	tmpDir, err := os.MkdirTemp("", "autoreply-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	// Set HOME for the test
	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpDir)
	defer os.Setenv("HOME", oldHome)

	// Create credential store (in-memory for tests)
	store := createInMemoryStore(t)

	// Test set and get default
	testHandle := "alice.bsky.social"

	err = store.SetDefault(testHandle)
	if err != nil {
		t.Fatalf("Failed to set default handle: %v", err)
	}

	loaded, err := store.GetDefault()
	if err != nil {
		t.Fatalf("Failed to get default handle: %v", err)
	}

	if loaded != testHandle {
		t.Errorf("Expected default handle %s, got %s", testHandle, loaded)
	}
}

func TestDeleteCredentials(t *testing.T) {
	// Create a temporary directory for test credentials
	tmpDir, err := os.MkdirTemp("", "autoreply-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	// Set HOME for the test
	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpDir)
	defer os.Setenv("HOME", oldHome)

	// Create credential store (in-memory for tests)
	store := createInMemoryStore(t)

	// Save credentials
	testCreds := &Credentials{
		Handle:       "alice.bsky.social",
		AccessToken:  "test-access-token",
		RefreshToken: "test-refresh-token",
		DID:          "did:plc:test123",
	}

	err = store.Save(testCreds)
	if err != nil {
		t.Fatalf("Failed to save credentials: %v", err)
	}

	// Delete credentials
	err = store.Delete("alice.bsky.social")
	if err != nil {
		t.Fatalf("Failed to delete credentials: %v", err)
	}

	// Try to load - should fail
	_, err = store.Load("alice.bsky.social")
	if err == nil {
		t.Error("Expected error when loading deleted credentials, got nil")
	}
}

func TestListHandles(t *testing.T) {
	// Create a temporary directory for test credentials
	tmpDir, err := os.MkdirTemp("", "autoreply-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	// Set HOME for the test
	oldHome := os.Getenv("HOME")
	testHome := filepath.Join(tmpDir, "home")
	os.MkdirAll(testHome, 0755)
	os.Setenv("HOME", testHome)
	defer os.Setenv("HOME", oldHome)

	// Create credential store (in-memory for tests)
	store := createInMemoryStore(t)

	// Save multiple credentials
	handles := []string{"alice.bsky.social", "bob.bsky.social"}
	for _, handle := range handles {
		creds := &Credentials{
			Handle:       handle,
			AccessToken:  "test-access-token-" + handle,
			RefreshToken: "test-refresh-token-" + handle,
			DID:          "did:plc:test-" + handle,
		}
		err = store.Save(creds)
		if err != nil {
			t.Fatalf("Failed to save credentials for %s: %v", handle, err)
		}
	}

	// List handles
	listed, err := store.ListHandles()
	if err != nil {
		t.Fatalf("Failed to list handles: %v", err)
	}

	t.Logf("listed handles: %v", listed)

	if len(listed) != len(handles) {
		t.Errorf("Expected %d handles, got %d", len(handles), len(listed))
	}

	// Check that all handles are present
	handleMap := make(map[string]bool)
	for _, h := range listed {
		handleMap[h] = true
	}

	for _, h := range handles {
		if !handleMap[h] {
			t.Errorf("Expected handle %s in list, but not found", h)
		}
	}
}
