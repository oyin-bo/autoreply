package auth

import (
	"context"
	"fmt"
	"sync"
	"time"
)

// CredentialManager manages authentication credentials for multiple accounts
type CredentialManager struct {
	keyring *KeyringBackend
	config  *Config
	mu      sync.RWMutex
}

// NewCredentialManager creates a new credential manager
func NewCredentialManager() (*CredentialManager, error) {
	config, err := LoadConfig()
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}
	
	keyring := NewKeyringBackend()
	
	return &CredentialManager{
		keyring: keyring,
		config:  config,
	}, nil
}

// StoreCredentials stores credentials for an account
func (cm *CredentialManager) StoreCredentials(ctx context.Context, handle string, creds *Credentials) error {
	cm.mu.Lock()
	defer cm.mu.Unlock()
	
	// Try to store in keyring
	keyringAvailable := cm.keyring.IsAvailable()
	storageRef := "keyring"
	
	if keyringAvailable {
		if err := cm.keyring.StoreCredentials(handle, creds); err != nil {
			return fmt.Errorf("failed to store credentials in keyring: %w", err)
		}
	} else {
		// TODO: Implement encrypted file storage fallback
		return fmt.Errorf("keyring not available and encrypted file storage not yet implemented")
	}
	
	// Update or add account metadata
	account := cm.config.GetAccount(handle)
	if account == nil {
		account = &Account{
			Handle:     handle,
			StorageRef: storageRef,
			CreatedAt:  time.Now(),
			LastUsed:   time.Now(),
		}
	} else {
		account.LastUsed = time.Now()
	}
	
	cm.config.AddAccount(*account)
	
	// Save config
	if err := cm.config.Save(); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}
	
	return nil
}

// GetCredentials retrieves credentials for an account
func (cm *CredentialManager) GetCredentials(ctx context.Context, handle string) (*Credentials, error) {
	cm.mu.RLock()
	defer cm.mu.RUnlock()
	
	account := cm.config.GetAccount(handle)
	if account == nil {
		return nil, fmt.Errorf("account not found: %s", handle)
	}
	
	// Try to get from keyring
	if account.StorageRef == "keyring" {
		creds, err := cm.keyring.GetCredentials(handle)
		if err != nil {
			return nil, fmt.Errorf("failed to get credentials from keyring: %w", err)
		}
		
		// Update last used timestamp
		go func() {
			cm.mu.Lock()
			defer cm.mu.Unlock()
			cm.config.UpdateLastUsed(handle)
			_ = cm.config.Save()
		}()
		
		return creds, nil
	}
	
	// TODO: Implement encrypted file storage fallback retrieval
	return nil, fmt.Errorf("storage backend %s not yet implemented", account.StorageRef)
}

// DeleteCredentials removes credentials for an account
func (cm *CredentialManager) DeleteCredentials(ctx context.Context, handle string) error {
	cm.mu.Lock()
	defer cm.mu.Unlock()
	
	account := cm.config.GetAccount(handle)
	if account == nil {
		return fmt.Errorf("account not found: %s", handle)
	}
	
	// Delete from keyring
	if account.StorageRef == "keyring" {
		if err := cm.keyring.DeleteCredentials(handle); err != nil {
			return fmt.Errorf("failed to delete credentials from keyring: %w", err)
		}
	}
	
	// Remove from config
	cm.config.RemoveAccount(handle)
	
	// If this was the default account, clear it
	if cm.config.DefaultAccount != nil && *cm.config.DefaultAccount == handle {
		cm.config.DefaultAccount = nil
	}
	
	// Save config
	if err := cm.config.Save(); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}
	
	return nil
}

// ListAccounts returns all authenticated accounts
func (cm *CredentialManager) ListAccounts(ctx context.Context) ([]Account, error) {
	cm.mu.RLock()
	defer cm.mu.RUnlock()
	
	return cm.config.Accounts, nil
}

// SetDefaultAccount sets the default account
func (cm *CredentialManager) SetDefaultAccount(ctx context.Context, handle string) error {
	cm.mu.Lock()
	defer cm.mu.Unlock()
	
	account := cm.config.GetAccount(handle)
	if account == nil {
		return fmt.Errorf("account not found: %s", handle)
	}
	
	cm.config.DefaultAccount = &handle
	
	if err := cm.config.Save(); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}
	
	return nil
}

// GetDefaultAccount returns the default account handle
func (cm *CredentialManager) GetDefaultAccount(ctx context.Context) (*string, error) {
	cm.mu.RLock()
	defer cm.mu.RUnlock()
	
	return cm.config.DefaultAccount, nil
}
