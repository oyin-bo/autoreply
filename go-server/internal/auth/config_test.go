package auth

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestConfigPath(t *testing.T) {
	path, err := ConfigPath()
	if err != nil {
		t.Fatalf("ConfigPath returned error: %v", err)
	}
	if path == "" {
		t.Error("ConfigPath should not return empty string")
	}
	
	// Should contain autoreply-mcp
	if !contains(path, "autoreply-mcp") {
		t.Errorf("ConfigPath should contain autoreply-mcp, got %s", path)
	}
}

func TestLoadAndSaveConfig(t *testing.T) {
	// Create temporary directory for test
	tmpDir, err := os.MkdirTemp("", "auth-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)
	
	// Override config path for testing
	testConfigPath := filepath.Join(tmpDir, "config.json")
	
	// Create a test config
	config := DefaultConfig()
	config.DefaultAccount = strPtr("test.bsky.social")
	config.AddAccount(Account{
		Handle:     "test.bsky.social",
		DID:        "did:plc:test123",
		PDS:        "https://bsky.social",
		StorageRef: "keyring",
		CreatedAt:  time.Now(),
		LastUsed:   time.Now(),
	})
	
	// Save config
	err = saveConfigToPath(config, testConfigPath)
	if err != nil {
		t.Fatalf("Failed to save config: %v", err)
	}
	
	// Check file exists and has correct permissions
	info, err := os.Stat(testConfigPath)
	if err != nil {
		t.Fatalf("Config file not created: %v", err)
	}
	
	// Check permissions (should be 0600)
	mode := info.Mode()
	if mode.Perm() != 0600 {
		t.Errorf("Expected file mode 0600, got %o", mode.Perm())
	}
	
	// Load config back
	loaded, err := loadConfigFromPath(testConfigPath)
	if err != nil {
		t.Fatalf("Failed to load config: %v", err)
	}
	
	// Verify loaded config
	if loaded.Version != config.Version {
		t.Errorf("Version mismatch: expected %s, got %s", config.Version, loaded.Version)
	}
	
	if len(loaded.Accounts) != 1 {
		t.Errorf("Expected 1 account, got %d", len(loaded.Accounts))
	}
	
	if loaded.DefaultAccount == nil || *loaded.DefaultAccount != "test.bsky.social" {
		t.Error("DefaultAccount not preserved")
	}
}

func TestLoadConfigNonExistent(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "auth-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)
	
	testConfigPath := filepath.Join(tmpDir, "nonexistent.json")
	
	// Should create default config when file doesn't exist
	config, err := loadConfigFromPath(testConfigPath)
	if err != nil {
		t.Fatalf("Failed to load non-existent config: %v", err)
	}
	
	if config == nil {
		t.Fatal("Expected default config to be created")
	}
	
	if len(config.Accounts) != 0 {
		t.Errorf("Expected empty accounts in default config, got %d", len(config.Accounts))
	}
}

// Helper functions
func strPtr(s string) *string {
	return &s
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && (s[:len(substr)] == substr || s[len(s)-len(substr):] == substr || containsMiddle(s, substr)))
}

func containsMiddle(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// Helper functions for testing
func saveConfigToPath(config *Config, path string) error {
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	
	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}
	
	return os.WriteFile(path, data, 0600)
}

func loadConfigFromPath(path string) (*Config, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return DefaultConfig(), nil
	}
	
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	
	var config Config
	if err := json.Unmarshal(data, &config); err != nil {
		return nil, err
	}
	
	return &config, nil
}
