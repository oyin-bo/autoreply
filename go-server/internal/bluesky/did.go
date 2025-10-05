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

// resolveWebDID resolves a did:web DID by fetching its did.json and extracting the PDS endpoint
func (r *DIDResolver) resolveWebDID(ctx context.Context, did string) (string, error) {
	// did:web:<host>[:<path>...]
	suffix := strings.TrimPrefix(did, "did:web:")
	parts := strings.Split(suffix, ":")
	if len(parts) == 0 || parts[0] == "" {
		return "", errors.NewMCPError(errors.DIDResolveFailed, "Invalid did:web format")
	}
	host := parts[0]
	// Build candidate URLs per did:web spec
	var candidates []string
	if len(parts) == 1 {
		candidates = []string{
			fmt.Sprintf("https://%s/.well-known/did.json", host),
			fmt.Sprintf("https://%s/did.json", host),
		}
	} else {
		path := strings.Join(parts[1:], "/")
		candidates = []string{fmt.Sprintf("https://%s/%s/did.json", host, path)}
	}

	var lastStatus int
	var lastErr error
	for _, urlStr := range candidates {
		req, err := http.NewRequestWithContext(ctx, "GET", urlStr, nil)
		if err != nil {
			lastErr = errors.Wrap(err, errors.InternalError, "Failed to create did:web request")
			continue
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; autoreply/1.0; +https://github.com/oyin-bo/autoreply)")
		req.Header.Set("Accept", "application/did+json, application/json")

		resp, err := r.client.Do(req)
		if err != nil {
			lastErr = errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve did:web document")
			continue
		}
		func() {
			defer resp.Body.Close()
			if resp.StatusCode != http.StatusOK {
				lastStatus = resp.StatusCode
				return
			}

			var didDoc DIDDocumentResponse
			if err := json.NewDecoder(resp.Body).Decode(&didDoc); err != nil {
				lastErr = errors.Wrap(err, errors.DIDResolveFailed, "Failed to parse did:web document")
				return
			}

			// Find PDS endpoint from service endpoints
			for _, service := range didDoc.Service {
				if service.Type == "AtprotoPersonalDataServer" || service.ID == "#atproto_pds" {
					lastErr = nil
					lastStatus = http.StatusOK
					// Return by capturing via panic/defer is overkill; use named return via closure result
					// Instead, set a temp variable and use an outer return
					candidates = []string{service.ServiceEndpoint}
					return
				}
			}
			lastErr = errors.NewMCPError(errors.DIDResolveFailed, "No PDS endpoint found in did:web document")
		}()
		// Special handling: closure uses candidates slice to carry the found endpoint
		if len(candidates) == 1 && strings.HasPrefix(candidates[0], "http") {
			return candidates[0], nil
		}
	}

	if lastErr != nil {
		return "", lastErr
	}
	if lastStatus != 0 {
		return "", errors.NewMCPError(errors.DIDResolveFailed,
			fmt.Sprintf("did:web document fetch failed with status %d", lastStatus))
	}
	return "", errors.NewMCPError(errors.DIDResolveFailed, "Failed to resolve did:web document")
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

// DIDDocumentResponse represents a DID document with service endpoints
type DIDDocumentResponse struct {
	Context            []string             `json:"@context"`
	ID                 string               `json:"id"`
	AlsoKnownAs        []string             `json:"alsoKnownAs,omitempty"`
	VerificationMethod []VerificationMethod `json:"verificationMethod,omitempty"`
	Service            []ServiceEndpoint    `json:"service,omitempty"`
}

// ServiceEndpoint represents a service endpoint in a DID document
type ServiceEndpoint struct {
	ID              string `json:"id"`
	Type            string `json:"type"`
	ServiceEndpoint string `json:"serviceEndpoint"`
}

// VerificationMethod represents a verification method in a DID document
type VerificationMethod struct {
	ID                 string `json:"id"`
	Type               string `json:"type"`
	Controller         string `json:"controller"`
	PublicKeyMultibase string `json:"publicKeyMultibase"`
}

// DID format validation regex
// did:plc are 32 characters total with did:plc: prefix (8 chars) + 24 base32 chars
var didPLCRegex = regexp.MustCompile(`^did:plc:[a-z2-7]{24}$`)

// Example: did:web:example.com or did:web:example.com:user:alice
var didWebRegex = regexp.MustCompile(`^did:web:[A-Za-z0-9.-]+(?::[A-Za-z0-9._%\-]+)*$`)

// NewDIDResolver creates a new DID resolver with default configuration
func NewDIDResolver() *DIDResolver {
	return &DIDResolver{
		client: &http.Client{
			Timeout: 10 * time.Second,
			Transport: &http.Transport{
				Proxy:               http.ProxyFromEnvironment,
				MaxIdleConns:        10,
				IdleConnTimeout:     30 * time.Second,
				DisableCompression:  false,
				MaxIdleConnsPerHost: 5,
			},
		},
		cacheTTL: 1 * time.Hour,
	}
}

// ResolveHandle converts a handle to a DID, with caching
func (r *DIDResolver) ResolveHandle(ctx context.Context, account string) (string, error) {
	// If it's already a DID, validate and return it
	if strings.HasPrefix(account, "did:") {
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

	// Resolve via network resolvers (api.bsky.app, bsky.social)
	did, err := r.resolveViaXRPC(ctx, handle)
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

// resolveViaXRPC performs the actual XRPC resolution via public resolvers
func (r *DIDResolver) resolveViaXRPC(ctx context.Context, handle string) (string, error) {
	endpoints := []string{
		"https://api.bsky.app/xrpc/com.atproto.identity.resolveHandle",
		"https://bsky.social/xrpc/com.atproto.identity.resolveHandle",
	}

	var lastErr error
	for _, endpoint := range endpoints {
		u, err := url.Parse(endpoint)
		if err != nil {
			lastErr = errors.Wrap(err, errors.InternalError, "Failed to parse resolution URL")
			continue
		}

		q := u.Query()
		q.Set("handle", handle)
		u.RawQuery = q.Encode()

		req, err := http.NewRequestWithContext(ctx, "GET", u.String(), nil)
		if err != nil {
			lastErr = errors.Wrap(err, errors.InternalError, "Failed to create HTTP request")
			continue
		}
		req.Header.Set("User-Agent", "autoreply/1.0")
		req.Header.Set("Accept", "application/json")

		resp, err := r.client.Do(req)
		if err != nil {
			lastErr = errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve handle")
			continue
		}

		if resp.StatusCode != http.StatusOK {
			resp.Body.Close()
			lastErr = errors.NewMCPError(errors.DIDResolveFailed,
				fmt.Sprintf("DID resolution failed with status %d", resp.StatusCode))
			continue
		}

		var resolveResponse ResolveHandleResponse
		decodeErr := json.NewDecoder(resp.Body).Decode(&resolveResponse)
		resp.Body.Close()
		if decodeErr != nil {
			lastErr = errors.Wrap(decodeErr, errors.DIDResolveFailed, "Failed to parse DID resolution response")
			continue
		}

		return resolveResponse.DID, nil
	}

	if lastErr != nil {
		return "", lastErr
	}
	return "", errors.NewMCPError(errors.DIDResolveFailed, "Unknown handle resolution error")
}

// IsValidDID validates DID format
func IsValidDID(did string) bool {
	// Accept common DID methods we support: did:plc and did:web
	return didPLCRegex.MatchString(did) || didWebRegex.MatchString(did)
}

// ResolvePDSEndpoint resolves a DID to find its PDS endpoint
func (r *DIDResolver) ResolvePDSEndpoint(ctx context.Context, did string) (string, error) {
	if strings.HasPrefix(did, "did:plc:") {
		return r.resolvePLCDID(ctx, did)
	}
	if strings.HasPrefix(did, "did:web:") {
		return r.resolveWebDID(ctx, did)
	}
	return "", errors.NewMCPError(errors.DIDResolveFailed,
		fmt.Sprintf("Unsupported DID method for PDS resolution: %s", did))
}

// resolvePLCDID resolves a did:plc DID to find its PDS endpoint
func (r *DIDResolver) resolvePLCDID(ctx context.Context, did string) (string, error) {
	// PLC directory service endpoint
	plcURL := fmt.Sprintf("https://plc.directory/%s", did)

	// Create request with context
	req, err := http.NewRequestWithContext(ctx, "GET", plcURL, nil)
	if err != nil {
		return "", errors.Wrap(err, errors.InternalError, "Failed to create DID document request")
	}

	req.Header.Set("User-Agent", "autoreply/1.0")
	req.Header.Set("Accept", "application/json")

	// Make the request
	resp, err := r.client.Do(req)
	if err != nil {
		return "", errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve DID document")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", errors.NewMCPError(errors.DIDResolveFailed,
			fmt.Sprintf("DID document resolution failed with status %d", resp.StatusCode))
	}

	// Parse response
	var didDoc DIDDocumentResponse
	if err := json.NewDecoder(resp.Body).Decode(&didDoc); err != nil {
		return "", errors.Wrap(err, errors.DIDResolveFailed, "Failed to parse DID document")
	}

	// Find PDS endpoint from service endpoints
	for _, service := range didDoc.Service {
		if service.Type == "AtprotoPersonalDataServer" || service.ID == "#atproto_pds" {
			return service.ServiceEndpoint, nil
		}
	}

	return "", errors.NewMCPError(errors.DIDResolveFailed,
		"No PDS endpoint found in DID document")
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
