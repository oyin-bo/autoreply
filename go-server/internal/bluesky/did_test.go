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

func TestParseAccountReference(t *testing.T) {
	tests := []struct {
		name        string
		reference   string
		expectedDID string
	}{
		{
			name:        "Valid DID",
			reference:   "did:plc:abc123defghi456jklmno789",
			expectedDID: "did:plc:abc123defghi456jklmno789",
		},
		{
			name:        "Handle without @",
			reference:   "user.bsky.social",
			expectedDID: "user.bsky.social",
		},
		{
			name:        "Handle with @",
			reference:   "@user.bsky.social",
			expectedDID: "user.bsky.social",
		},
		{
			name:        "Empty string",
			reference:   "",
			expectedDID: "",
		},
		{
			name:        "Multiple @ symbols",
			reference:   "@@user.bsky.social",
			expectedDID: "@user.bsky.social",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ParseAccountReference(tt.reference)
			if result != tt.expectedDID {
				t.Errorf("ParseAccountReference(%q) = %q, want %q", tt.reference, result, tt.expectedDID)
			}
		})
	}
}

func TestResolveHandle_ErrorCases(t *testing.T) {
	resolver := NewDIDResolver()
	ctx := context.Background()

	tests := []struct {
		name    string
		account string
	}{
		{
			name:    "Empty string",
			account: "",
		},
		{
			name:    "Nonexistent handle",
			account: "nonexistent.handle.test.invalid",
		},
		{
			name:    "Invalid characters in handle",
			account: "test@invalid!handle",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := resolver.ResolveHandle(ctx, tt.account)
			if err == nil {
				t.Error("Expected error for invalid handle, got nil")
			}
		})
	}
}

func TestResolvePDSEndpoint_Variations(t *testing.T) {
	resolver := NewDIDResolver()
	ctx := context.Background()

	tests := []struct {
		name        string
		did         string
		expectError bool
	}{
		{
			name:        "Valid PLC DID",
			did:         "did:plc:abc123defghi456jklmno789",
			expectError: true, // Will fail without actual PLC server
		},
		{
			name:        "Valid Web DID",
			did:         "did:web:example.com",
			expectError: true, // Will fail without actual server
		},
		{
			name:        "Invalid DID",
			did:         "not-a-did",
			expectError: true,
		},
		{
			name:        "Empty DID",
			did:         "",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			endpoint, err := resolver.ResolvePDSEndpoint(ctx, tt.did)
			if tt.expectError {
				if err == nil {
					t.Error("Expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("Unexpected error: %v", err)
				}
				if endpoint == "" {
					t.Error("Expected non-empty endpoint")
				}
			}
		})
	}
}

func TestDIDResolver_CacheEviction(t *testing.T) {
	resolver := NewDIDResolver()

	// Set a very short TTL
	resolver.cacheTTL = time.Microsecond

	// Add multiple cache entries
	testEntries := map[string]string{
		"user1.bsky.social": "did:plc:user1abc123defghi456jkl",
		"user2.bsky.social": "did:plc:user2abc123defghi456jkl",
		"user3.bsky.social": "did:plc:user3abc123defghi456jkl",
	}

	for handle, did := range testEntries {
		resolver.cache.Store(handle, CacheEntry{
			DID:       did,
			ExpiresAt: time.Now().Add(-time.Hour), // Already expired
		})
	}

	// Verify entries exist
	count := 0
	resolver.cache.Range(func(key, value interface{}) bool {
		count++
		return true
	})

	if count != 3 {
		t.Errorf("Expected 3 cache entries, got %d", count)
	}

	// Run cleanup
	resolver.CleanupCache()

	// Verify all expired entries are gone
	count = 0
	resolver.cache.Range(func(key, value interface{}) bool {
		count++
		return true
	})

	if count != 0 {
		t.Errorf("Expected 0 cache entries after cleanup, got %d", count)
	}
}

func TestDIDResolver_NonExpiredCache(t *testing.T) {
	resolver := NewDIDResolver()

	// Add a non-expired entry
	testHandle := "fresh.bsky.social"
	testDID := "did:plc:freshabc123defghi456jkl"

	resolver.cache.Store(testHandle, CacheEntry{
		DID:       testDID,
		ExpiresAt: time.Now().Add(time.Hour), // Not expired
	})

	// Run cleanup
	resolver.CleanupCache()

	// Verify non-expired entry still exists
	if _, ok := resolver.cache.Load(testHandle); !ok {
		t.Error("Non-expired cache entry should not be removed")
	}
}
