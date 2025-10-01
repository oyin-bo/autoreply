// Package auth provides authentication and credential management
package auth

import (
	"os"
	"path/filepath"
	"testing"
)

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

	// Create credential store
	store, err := NewCredentialStore()
	if err != nil {
		t.Fatalf("Failed to create credential store: %v", err)
	}

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

	// Create credential store
	store, err := NewCredentialStore()
	if err != nil {
		t.Fatalf("Failed to create credential store: %v", err)
	}

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

	// Create credential store
	store, err := NewCredentialStore()
	if err != nil {
		t.Fatalf("Failed to create credential store: %v", err)
	}

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

	// Create credential store
	store, err := NewCredentialStore()
	if err != nil {
		t.Fatalf("Failed to create credential store: %v", err)
	}

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
