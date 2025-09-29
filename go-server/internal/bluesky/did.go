// Package bluesky provides DID resolution functionality
package bluesky

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// DIDResolver handles handle-to-DID resolution with caching
type DIDResolver struct {
	client   *http.Client
	cache    sync.Map
	cacheTTL time.Duration
}

// CacheEntry represents a cached DID resolution result
type CacheEntry struct {
	DID       string
	ExpiresAt time.Time
}

// ResolveHandleResponse represents the response from XRPC DID resolution
type ResolveHandleResponse struct {
	DID string `json:"did"`
}

// DID format validation regex - DIDs are 32 characters total with did:plc: prefix (8 chars) + 24 base32 chars
var didRegex = regexp.MustCompile(`^did:plc:[a-z2-7]{24}$`)

// NewDIDResolver creates a new DID resolver with default configuration
func NewDIDResolver() *DIDResolver {
	return &DIDResolver{
		client: &http.Client{
			Timeout: 10 * time.Second,
		},
		cacheTTL: 1 * time.Hour,
	}
}

// ResolveHandle converts a handle to a DID, with caching
func (r *DIDResolver) ResolveHandle(ctx context.Context, account string) (string, error) {
	// If it's already a DID, validate and return it
	if strings.HasPrefix(account, "did:plc:") {
		if !IsValidDID(account) {
			return "", errors.NewMCPError(errors.InvalidInput, "Invalid DID format")
		}
		return account, nil
	}

	// Check cache first
	if cached, ok := r.cache.Load(account); ok {
		entry := cached.(CacheEntry)
		if time.Now().Before(entry.ExpiresAt) {
			return entry.DID, nil
		}
		// Remove expired entry
		r.cache.Delete(account)
	}

	// Clean handle (remove @ prefix if present)
	handle := strings.TrimPrefix(account, "@")

	// Extract hostname for resolution endpoint
	hostname := "bsky.social" // Default fallback
	if strings.Contains(handle, ".") {
		parts := strings.Split(handle, ".")
		if len(parts) >= 2 {
			hostname = strings.Join(parts[1:], ".")
		}
	}

	// Resolve via XRPC
	did, err := r.resolveViaXRPC(ctx, handle, hostname)
	if err != nil {
		return "", err
	}

	// Validate DID format
	if !IsValidDID(did) {
		return "", errors.NewMCPError(errors.DIDResolveFailed, 
			fmt.Sprintf("Invalid DID format returned: %s", did))
	}

	// Cache the result
	r.cache.Store(account, CacheEntry{
		DID:       did,
		ExpiresAt: time.Now().Add(r.cacheTTL),
	})

	return did, nil
}

// resolveViaXRPC performs the actual XRPC resolution
func (r *DIDResolver) resolveViaXRPC(ctx context.Context, handle, hostname string) (string, error) {
	// Build resolution URL
	resolveURL := fmt.Sprintf("https://%s/xrpc/com.atproto.identity.resolveHandle", hostname)
	
	// Add query parameters
	u, err := url.Parse(resolveURL)
	if err != nil {
		return "", errors.Wrap(err, errors.InternalError, "Failed to parse resolution URL")
	}
	
	query := u.Query()
	query.Set("handle", handle)
	u.RawQuery = query.Encode()

	// Create request with context
	req, err := http.NewRequestWithContext(ctx, "GET", u.String(), nil)
	if err != nil {
		return "", errors.Wrap(err, errors.InternalError, "Failed to create HTTP request")
	}

	// Set User-Agent header
	req.Header.Set("User-Agent", "bluesky-mcp-server/1.0")

	// Make the request
	resp, err := r.client.Do(req)
	if err != nil {
		return "", errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve handle")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", errors.NewMCPError(errors.DIDResolveFailed, 
			fmt.Sprintf("DID resolution failed with status %d", resp.StatusCode))
	}

	// Parse response
	var resolveResponse ResolveHandleResponse
	if err := json.NewDecoder(resp.Body).Decode(&resolveResponse); err != nil {
		return "", errors.Wrap(err, errors.DIDResolveFailed, "Failed to parse DID resolution response")
	}

	return resolveResponse.DID, nil
}

// IsValidDID validates DID format
func IsValidDID(did string) bool {
	return didRegex.MatchString(did)
}

// CleanupCache removes expired entries from the cache
func (r *DIDResolver) CleanupCache() {
	now := time.Now()
	r.cache.Range(func(key, value interface{}) bool {
		entry := value.(CacheEntry)
		if now.After(entry.ExpiresAt) {
			r.cache.Delete(key)
		}
		return true
	})
}