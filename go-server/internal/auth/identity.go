// Package auth provides authentication and credential management
package auth

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// DIDDocument represents a simplified DID document
type DIDDocument struct {
	Context []string               `json:"@context"`
	ID      string                 `json:"id"`
	Service []DIDServiceEndpoint   `json:"service,omitempty"`
	AlsoKnownAs []string            `json:"alsoKnownAs,omitempty"`
}

// DIDServiceEndpoint represents a service endpoint in a DID document
type DIDServiceEndpoint struct {
	ID              string `json:"id"`
	Type            string `json:"type"`
	ServiceEndpoint string `json:"serviceEndpoint"`
}

// ResolveHandle resolves an atproto handle to a DID
func ResolveHandle(ctx context.Context, handle string) (string, error) {
	// Normalize handle
	handle = strings.TrimPrefix(handle, "@")
	handle = strings.ToLower(handle)

	// Try HTTPS resolution first (most common)
	did, err := resolveHandleHTTPS(ctx, handle)
	if err == nil {
		// Verify bidirectional resolution
		if err := verifyHandleDID(ctx, handle, did); err != nil {
			return "", fmt.Errorf("handle verification failed: %w", err)
		}
		return did, nil
	}

	// Could add DNS resolution as fallback, but HTTPS is sufficient for most cases
	return "", fmt.Errorf("failed to resolve handle %s: %w", handle, err)
}

// resolveHandleHTTPS resolves a handle using HTTPS well-known endpoint
func resolveHandleHTTPS(ctx context.Context, handle string) (string, error) {
	url := fmt.Sprintf("https://%s/.well-known/atproto-did", handle)

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return "", err
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("HTTP %d from %s", resp.StatusCode, url)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 1024))
	if err != nil {
		return "", err
	}

	did := strings.TrimSpace(string(body))
	if !strings.HasPrefix(did, "did:") {
		return "", fmt.Errorf("invalid DID format: %s", did)
	}

	return did, nil
}

// verifyHandleDID verifies that a DID document claims the given handle
func verifyHandleDID(ctx context.Context, handle, did string) error {
	doc, err := ResolveDID(ctx, did)
	if err != nil {
		return err
	}

	// Check alsoKnownAs for the handle
	expectedHandle := "at://" + handle
	for _, aka := range doc.AlsoKnownAs {
		if aka == expectedHandle {
			return nil
		}
	}

	return fmt.Errorf("DID document does not claim handle %s", handle)
}

// ResolveDID resolves a DID to its DID document
func ResolveDID(ctx context.Context, did string) (*DIDDocument, error) {
	if strings.HasPrefix(did, "did:plc:") {
		return resolveDIDPLC(ctx, did)
	} else if strings.HasPrefix(did, "did:web:") {
		return resolveDIDWeb(ctx, did)
	}

	return nil, fmt.Errorf("unsupported DID method: %s", did)
}

// resolveDIDPLC resolves a did:plc DID using the PLC directory
func resolveDIDPLC(ctx context.Context, did string) (*DIDDocument, error) {
	url := fmt.Sprintf("https://plc.directory/%s", did)

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Accept", "application/json")

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return nil, fmt.Errorf("PLC resolution failed with status %d: %s", resp.StatusCode, string(body))
	}

	var doc DIDDocument
	if err := json.NewDecoder(resp.Body).Decode(&doc); err != nil {
		return nil, fmt.Errorf("failed to decode DID document: %w", err)
	}

	return &doc, nil
}

// resolveDIDWeb resolves a did:web DID
func resolveDIDWeb(ctx context.Context, did string) (*DIDDocument, error) {
	// did:web:example.com -> https://example.com/.well-known/did.json
	// did:web:example.com:path -> https://example.com/path/did.json
	parts := strings.Split(did, ":")
	if len(parts) < 3 {
		return nil, fmt.Errorf("invalid did:web format")
	}

	domain := parts[2]
	var path string
	if len(parts) > 3 {
		path = "/" + strings.Join(parts[3:], "/")
	} else {
		path = "/.well-known"
	}

	url := fmt.Sprintf("https://%s%s/did.json", domain, path)

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Accept", "application/json")

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("did:web resolution failed with status %d", resp.StatusCode)
	}

	var doc DIDDocument
	if err := json.NewDecoder(resp.Body).Decode(&doc); err != nil {
		return nil, fmt.Errorf("failed to decode DID document: %w", err)
	}

	return &doc, nil
}

// ResolvePDSFromDID extracts the PDS endpoint from a DID document
func ResolvePDSFromDID(ctx context.Context, did string) (string, error) {
	doc, err := ResolveDID(ctx, did)
	if err != nil {
		return "", err
	}

	// Look for atproto_pds service
	for _, service := range doc.Service {
		if service.Type == "AtprotoPersonalDataServer" || service.ID == "#atproto_pds" {
			return service.ServiceEndpoint, nil
		}
	}

	return "", fmt.Errorf("no PDS service found in DID document")
}
