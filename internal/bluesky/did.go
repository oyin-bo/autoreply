// did.go - DID resolution implementation
package bluesky

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/pkg/errors"
)

// DidResolver handles DID resolution and PDS discovery
type DidResolver struct {
	client *Client
}

// NewDidResolver creates a new DID resolver
func NewDidResolver() *DidResolver {
	return &DidResolver{
		client: NewClient(),
	}
}

// HandleResolveResponse represents the response from handle resolution
type HandleResolveResponse struct {
	DID string `json:"did"`
}

// PLCOperation represents a PLC directory entry
type PLCOperation struct {
	Operation *PLCService `json:"operation,omitempty"`
}

// PLCService represents service configuration in PLC
type PLCService struct {
	Services map[string]*ServiceEndpoint `json:"services,omitempty"`
}

// ServiceEndpoint represents a service endpoint
type ServiceEndpoint struct {
	Endpoint string `json:"endpoint,omitempty"`
}

// PLCLogEntry represents a PLC log entry
type PLCLogEntry struct {
	Operation *PLCService `json:"operation,omitempty"`
}

// ResolveHandle resolves a handle to a DID
func (r *DidResolver) ResolveHandle(ctx context.Context, handle string) (string, error) {
	// If already a DID, return as-is
	if strings.HasPrefix(handle, "did:plc:") {
		if err := errors.ValidateAccount(handle); err != nil {
			return "", err
		}
		return handle, nil
	}

	// Validate handle format
	if err := errors.ValidateAccount(handle); err != nil {
		return "", err
	}

	// Extract hostname from handle
	parts := strings.Split(handle, ".")
	if len(parts) < 2 {
		return "", errors.NewMcpError(errors.InvalidInput, "Invalid handle format")
	}

	hostname := strings.Join(parts[1:], ".")
	
	// Resolve handle to DID
	url := fmt.Sprintf("https://%s/xrpc/com.atproto.identity.resolveHandle?handle=%s", hostname, handle)

	// Set DID resolution timeout to 10 seconds
	reqCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	resp, err := r.client.Get(reqCtx, url)
	if err != nil {
		return "", errors.NewMcpError(errors.DIDResolveFailed, fmt.Sprintf("Failed to resolve handle: %v", err))
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		body, _ := io.ReadAll(resp.Body)
		return "", errors.NewMcpError(errors.DIDResolveFailed, fmt.Sprintf("HTTP %d: %s", resp.StatusCode, string(body)))
	}

	var result HandleResolveResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", errors.NewMcpError(errors.DIDResolveFailed, fmt.Sprintf("Failed to parse response: %v", err))
	}

	if result.DID == "" {
		return "", errors.NewMcpError(errors.DIDResolveFailed, "Empty DID in response")
	}

	return result.DID, nil
}

// DiscoverPDS discovers the PDS endpoint for a DID
func (r *DidResolver) DiscoverPDS(ctx context.Context, did string) (string, error) {
	url := fmt.Sprintf("https://plc.directory/%s/log/audit", did)
	
	reqCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	resp, err := r.client.Get(reqCtx, url)
	if err != nil {
		return "", errors.NewMcpError(errors.DIDResolveFailed, fmt.Sprintf("Failed to fetch PLC log: %v", err))
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		// Fallback to default bsky.social if PLC directory is unavailable
		return "https://bsky.social", nil
	}

	var logEntries []PLCLogEntry
	if err := json.NewDecoder(resp.Body).Decode(&logEntries); err != nil {
		// Fallback to default on parse error
		return "https://bsky.social", nil
	}

	// Look for PDS endpoint in reverse chronological order
	for i := len(logEntries) - 1; i >= 0; i-- {
		entry := logEntries[i]
		if entry.Operation != nil && entry.Operation.Services != nil {
			if pds, exists := entry.Operation.Services["atproto_pds"]; exists && pds.Endpoint != "" {
				return pds.Endpoint, nil
			}
		}
	}

	// Fallback to default bsky.social
	return "https://bsky.social", nil
}