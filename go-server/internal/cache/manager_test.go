package cache

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewManager(t *testing.T) {
	manager, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create cache manager: %v", err)
	}
	
	if manager.cacheDir == "" {
		t.Error("Cache directory is empty")
	}
	
	// Check if cache directory exists
	if _, err := os.Stat(manager.cacheDir); os.IsNotExist(err) {
		t.Errorf("Cache directory does not exist: %s", manager.cacheDir)
	}
}

func TestGetCachePath(t *testing.T) {
	manager, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create cache manager: %v", err)
	}

	tests := []struct {
		name    string
		did     string
		wantErr bool
	}{
		{
			name:    "Valid DID",
			did:     "did:plc:abc123xyz789",
			wantErr: false,
		},
		{
			name:    "Empty DID",
			did:     "",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			path, err := manager.GetCachePath(tt.did)
			
			if tt.wantErr {
				if err == nil {
					t.Error("Expected error but got none")
				}
				return
			}
			
			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}
			
			if path == "" {
				t.Error("Cache path is empty")
			}
			
			// Should contain two-letter prefix and full DID
			if tt.did != "" && filepath.Base(path) != tt.did {
				t.Errorf("Cache path should end with DID %s, got %s", tt.did, path)
			}
		})
	}
}

func TestStoreCar(t *testing.T) {
	manager, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create cache manager: %v", err)
	}

	did := "did:plc:test123456789"
	testData := []byte("test car data")
	metadata := Metadata{
		DID:           did,
		CachedAt:      time.Now().Unix(),
		TTLHours:      24,
		ContentLength: func() *int64 { l := int64(len(testData)); return &l }(),
	}

	// Test storing
	err = manager.StoreCar(did, testData, metadata)
	if err != nil {
		t.Fatalf("Failed to store CAR: %v", err)
	}

	// Test reading back
	readData, err := manager.ReadCar(did)
	if err != nil {
		t.Fatalf("Failed to read CAR: %v", err)
	}

	if string(readData) != string(testData) {
		t.Errorf("Data mismatch: expected %s, got %s", testData, readData)
	}

	// Test metadata
	readMetadata, err := manager.GetMetadata(did)
	if err != nil {
		t.Fatalf("Failed to read metadata: %v", err)
	}

	if readMetadata.DID != did {
		t.Errorf("DID mismatch: expected %s, got %s", did, readMetadata.DID)
	}

	// Cleanup
	carPath, _, _ := manager.GetFilePaths(did)
	os.RemoveAll(filepath.Dir(carPath))
}

func TestIsCacheValid(t *testing.T) {
	manager, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create cache manager: %v", err)
	}

	did := "did:plc:test123456789"

	// Test non-existent cache
	if manager.IsCacheValid(did, 24) {
		t.Error("Expected cache to be invalid for non-existent DID")
	}

	// Create cache entry
	testData := []byte("test")
	metadata := Metadata{
		DID:      did,
		CachedAt: time.Now().Unix(),
		TTLHours: 1,
	}

	err = manager.StoreCar(did, testData, metadata)
	if err != nil {
		t.Fatalf("Failed to store CAR: %v", err)
	}

	// Test valid cache
	if !manager.IsCacheValid(did, 24) {
		t.Error("Expected cache to be valid")
	}

	// Test expired cache (with 0 TTL)
	if manager.IsCacheValid(did, 0) {
		t.Error("Expected cache to be invalid with 0 TTL")
	}

	// Cleanup
	carPath, _, _ := manager.GetFilePaths(did)
	os.RemoveAll(filepath.Dir(carPath))
}