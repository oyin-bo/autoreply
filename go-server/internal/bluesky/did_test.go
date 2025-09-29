package bluesky

import (
	"context"
	"testing"
	"time"
)

func TestNewDIDResolver(t *testing.T) {
	resolver := NewDIDResolver()
	if resolver == nil {
		t.Fatal("Failed to create DID resolver")
	}
	
	if resolver.client == nil {
		t.Error("HTTP client is nil")
	}
	
	if resolver.cacheTTL != 1*time.Hour {
		t.Errorf("Expected cache TTL to be 1 hour, got %v", resolver.cacheTTL)
	}
}

func TestIsValidDID(t *testing.T) {
	tests := []struct {
		name string
		did  string
		want bool
	}{
		{
			name: "Valid DID",
			did:  "did:plc:abcdefghijklmnopqrstuvwx",
			want: true,
		},
		{
			name: "Valid did:web",
			did:  "did:web:example.com",
			want: true,
		},
		{
			name: "Too short",
			did:  "did:plc:abc123",
			want: false,
		},
		{
			name: "Too long",
			did:  "did:plc:abc123xyz789defghi1234567890",
			want: false,
		},
		{
			name: "Invalid characters",
			did:  "did:plc:abc123xyz789DEFGHI123456",
			want: false,
		},
		{
			name: "Empty string",
			did:  "",
			want: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsValidDID(tt.did)
			if got != tt.want {
				t.Errorf("IsValidDID(%q) = %v, want %v", tt.did, got, tt.want)
			}
		})
	}
}

func TestResolveHandle_AlreadyDID(t *testing.T) {
	resolver := NewDIDResolver()
	ctx := context.Background()

	tests := []struct {
		name    string
		account string
		want    string
		wantErr bool
	}{
		{
			name:    "Valid DID",
			account: "did:plc:abcdefghijklmnopqrstuvwx",
			want:    "did:plc:abcdefghijklmnopqrstuvwx",
			wantErr: false,
		},
		{
			name:    "Invalid DID format",
			account: "did:plc:invalid",
			want:    "",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := resolver.ResolveHandle(ctx, tt.account)
			
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
			
			if got != tt.want {
				t.Errorf("ResolveHandle(%q) = %q, want %q", tt.account, got, tt.want)
			}
		})
	}
}

func TestCleanupCache(t *testing.T) {
	resolver := NewDIDResolver()
	
	// Add some cache entries (simulating expired entries)
	testAccount := "test.bsky.social"
	testDID := "did:plc:abcdefghijklmnopqrstuvwx"
	
	// Set a very short TTL to test expiration
	resolver.cacheTTL = time.Nanosecond
	
	// Add entry to cache
	resolver.cache.Store(testAccount, CacheEntry{
		DID:       testDID,
		ExpiresAt: time.Now().Add(-time.Hour), // Already expired
	})
	
	// Verify entry exists before cleanup
	if _, ok := resolver.cache.Load(testAccount); !ok {
		t.Error("Cache entry should exist before cleanup")
	}
	
	// Run cleanup
	resolver.CleanupCache()
	
	// Verify entry is gone after cleanup
	if _, ok := resolver.cache.Load(testAccount); ok {
		t.Error("Cache entry should be removed after cleanup")
	}
}