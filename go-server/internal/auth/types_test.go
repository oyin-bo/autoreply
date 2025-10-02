package auth

import (
	"testing"
	"time"
)

func TestDefaultConfig(t *testing.T) {
	config := DefaultConfig()
	
	if config.Version != "2.0" {
		t.Errorf("Expected version 2.0, got %s", config.Version)
	}
	
	if len(config.Accounts) != 0 {
		t.Errorf("Expected empty accounts, got %d", len(config.Accounts))
	}
	
	if !config.Settings.AutoRefresh {
		t.Error("Expected AutoRefresh to be true")
	}
	
	if config.Settings.RefreshThresholdMinutes != 5 {
		t.Errorf("Expected RefreshThresholdMinutes 5, got %d", config.Settings.RefreshThresholdMinutes)
	}
}

func TestConfigGetAccount(t *testing.T) {
	config := DefaultConfig()
	
	// Test getting non-existent account
	account := config.GetAccount("test.bsky.social")
	if account != nil {
		t.Error("Expected nil for non-existent account")
	}
	
	// Add an account
	testAccount := Account{
		Handle:     "test.bsky.social",
		DID:        "did:plc:test123",
		PDS:        "https://bsky.social",
		StorageRef: "keyring",
		CreatedAt:  time.Now(),
		LastUsed:   time.Now(),
	}
	config.AddAccount(testAccount)
	
	// Test getting existing account
	account = config.GetAccount("test.bsky.social")
	if account == nil {
		t.Fatal("Expected to find account")
	}
	if account.Handle != "test.bsky.social" {
		t.Errorf("Expected handle test.bsky.social, got %s", account.Handle)
	}
}

func TestConfigAddAccount(t *testing.T) {
	config := DefaultConfig()
	
	account1 := Account{
		Handle:     "alice.bsky.social",
		DID:        "did:plc:alice123",
		PDS:        "https://bsky.social",
		StorageRef: "keyring",
		CreatedAt:  time.Now(),
		LastUsed:   time.Now(),
	}
	
	config.AddAccount(account1)
	if len(config.Accounts) != 1 {
		t.Errorf("Expected 1 account, got %d", len(config.Accounts))
	}
	
	// Test updating existing account
	account1.DID = "did:plc:alice456"
	config.AddAccount(account1)
	if len(config.Accounts) != 1 {
		t.Errorf("Expected 1 account after update, got %d", len(config.Accounts))
	}
	
	updated := config.GetAccount("alice.bsky.social")
	if updated.DID != "did:plc:alice456" {
		t.Errorf("Expected updated DID, got %s", updated.DID)
	}
}

func TestConfigRemoveAccount(t *testing.T) {
	config := DefaultConfig()
	
	account := Account{
		Handle:    "test.bsky.social",
		DID:       "did:plc:test123",
		CreatedAt: time.Now(),
		LastUsed:  time.Now(),
	}
	config.AddAccount(account)
	
	// Test removing existing account
	removed := config.RemoveAccount("test.bsky.social")
	if !removed {
		t.Error("Expected account to be removed")
	}
	if len(config.Accounts) != 0 {
		t.Errorf("Expected 0 accounts, got %d", len(config.Accounts))
	}
	
	// Test removing non-existent account
	removed = config.RemoveAccount("nonexistent.bsky.social")
	if removed {
		t.Error("Expected removal to return false for non-existent account")
	}
}

func TestConfigUpdateLastUsed(t *testing.T) {
	config := DefaultConfig()
	
	oldTime := time.Now().Add(-1 * time.Hour)
	account := Account{
		Handle:    "test.bsky.social",
		DID:       "did:plc:test123",
		CreatedAt: oldTime,
		LastUsed:  oldTime,
	}
	config.AddAccount(account)
	
	// Update last used
	config.UpdateLastUsed("test.bsky.social")
	
	updated := config.GetAccount("test.bsky.social")
	if updated.LastUsed.Before(oldTime) || updated.LastUsed.Equal(oldTime) {
		t.Error("Expected LastUsed to be updated to more recent time")
	}
}

func TestCredentialsNeedsRefresh(t *testing.T) {
	tests := []struct {
		name                 string
		expiresIn            time.Duration
		thresholdMinutes     int
		expectedNeedsRefresh bool
	}{
		{
			name:                 "Expired credentials",
			expiresIn:            -5 * time.Minute,
			thresholdMinutes:     5,
			expectedNeedsRefresh: true,
		},
		{
			name:                 "Within threshold",
			expiresIn:            3 * time.Minute,
			thresholdMinutes:     5,
			expectedNeedsRefresh: true,
		},
		{
			name:                 "Outside threshold",
			expiresIn:            10 * time.Minute,
			thresholdMinutes:     5,
			expectedNeedsRefresh: false,
		},
		{
			name:                 "Just at threshold",
			expiresIn:            5*time.Minute + 1*time.Second, // Just over threshold
			thresholdMinutes:     5,
			expectedNeedsRefresh: false,
		},
	}
	
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			creds := &Credentials{
				AccessToken:  "test_token",
				RefreshToken: "test_refresh",
				ExpiresAt:    time.Now().Add(tt.expiresIn),
			}
			
			needsRefresh := creds.NeedsRefresh(tt.thresholdMinutes)
			if needsRefresh != tt.expectedNeedsRefresh {
				t.Errorf("Expected NeedsRefresh=%v, got %v", tt.expectedNeedsRefresh, needsRefresh)
			}
		})
	}
}
