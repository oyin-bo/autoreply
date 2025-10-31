// Package auth provides authentication and credential management
package auth

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// ProtectedResourceMetadata represents the OAuth protected resource metadata
type ProtectedResourceMetadata struct {
	Resource               string   `json:"resource"`
	AuthorizationServers   []string `json:"authorization_servers"`
	BearerMethodsSupported []string `json:"bearer_methods_supported,omitempty"`
}

// AuthorizationServerMetadata represents the OAuth authorization server metadata
type AuthorizationServerMetadata struct {
	Issuer                             string   `json:"issuer"`
	AuthorizationEndpoint              string   `json:"authorization_endpoint"`
	TokenEndpoint                      string   `json:"token_endpoint"`
	PushedAuthorizationRequestEndpoint string   `json:"pushed_authorization_request_endpoint"`
	RegistrationEndpoint               string   `json:"registration_endpoint,omitempty"`
	JWKSEndpoint                       string   `json:"jwks_uri,omitempty"`
	ScopesSupported                    []string `json:"scopes_supported"`
	ResponseTypesSupported             []string `json:"response_types_supported"`
	GrantTypesSupported                []string `json:"grant_types_supported"`
	TokenEndpointAuthMethodsSupported  []string `json:"token_endpoint_auth_methods_supported,omitempty"`
	DPoPSigningAlgValuesSupported      []string `json:"dpop_signing_alg_values_supported,omitempty"`
	CodeChallengeMethodsSupported      []string `json:"code_challenge_methods_supported,omitempty"`
	RequirePushedAuthorizationRequests bool     `json:"require_pushed_authorization_requests,omitempty"`
}

// MetadataDiscovery handles OAuth server metadata discovery
type MetadataDiscovery struct {
	httpClient *http.Client
}

// NewMetadataDiscovery creates a new metadata discovery client
func NewMetadataDiscovery() *MetadataDiscovery {
	return &MetadataDiscovery{
		httpClient: &http.Client{
			Timeout: 10 * time.Second,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				// Limit redirects to prevent abuse
				if len(via) >= 3 {
					return fmt.Errorf("too many redirects")
				}
				return nil
			},
		},
	}
}

// DiscoverFromPDS discovers OAuth metadata starting from a PDS URL
func (d *MetadataDiscovery) DiscoverFromPDS(ctx context.Context, pdsURL string) (*AuthorizationServerMetadata, error) {
	// Ensure URL is properly formatted
	if !strings.HasPrefix(pdsURL, "http://") && !strings.HasPrefix(pdsURL, "https://") {
		pdsURL = "https://" + pdsURL
	}

	parsedURL, err := url.Parse(pdsURL)
	if err != nil {
		return nil, fmt.Errorf("invalid PDS URL: %w", err)
	}

	// Construct the protected resource metadata URL
	resourceMetadataURL := fmt.Sprintf("%s://%s/.well-known/oauth-protected-resource",
		parsedURL.Scheme, parsedURL.Host)

	// Fetch protected resource metadata
	resourceMeta, err := d.fetchProtectedResourceMetadata(ctx, resourceMetadataURL)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch protected resource metadata: %w", err)
	}

	// Get the first authorization server
	if len(resourceMeta.AuthorizationServers) == 0 {
		return nil, fmt.Errorf("no authorization servers found in metadata")
	}

	authServerIssuer := resourceMeta.AuthorizationServers[0]

	// Fetch authorization server metadata
	return d.fetchAuthorizationServerMetadata(ctx, authServerIssuer)
}

// DiscoverFromHandle discovers OAuth metadata starting from an atproto handle
func (d *MetadataDiscovery) DiscoverFromHandle(ctx context.Context, handle string) (*AuthorizationServerMetadata, string, error) {
	// Resolve handle to DID
	did, err := ResolveHandle(ctx, handle)
	if err != nil {
		return nil, "", err // Don't wrap - preserve original error
	}

	// Resolve DID to get PDS endpoint
	pdsURL, err := ResolvePDSFromDID(ctx, did)
	if err != nil {
		return nil, "", err // Don't wrap - preserve original error
	}

	// Discover OAuth metadata from PDS
	metadata, err := d.DiscoverFromPDS(ctx, pdsURL)
	if err != nil {
		return nil, "", err
	}

	return metadata, did, nil
}

// fetchProtectedResourceMetadata fetches the protected resource metadata
func (d *MetadataDiscovery) fetchProtectedResourceMetadata(ctx context.Context, metadataURL string) (*ProtectedResourceMetadata, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", metadataURL, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Accept", "application/json")

	resp, err := d.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return nil, fmt.Errorf("metadata fetch failed with status %d: %s", resp.StatusCode, string(body))
	}

	var metadata ProtectedResourceMetadata
	if err := json.NewDecoder(resp.Body).Decode(&metadata); err != nil {
		return nil, fmt.Errorf("failed to decode metadata: %w", err)
	}

	return &metadata, nil
}

// fetchAuthorizationServerMetadata fetches the authorization server metadata
func (d *MetadataDiscovery) fetchAuthorizationServerMetadata(ctx context.Context, issuer string) (*AuthorizationServerMetadata, error) {
	// Construct the authorization server metadata URL
	metadataURL := issuer + "/.well-known/oauth-authorization-server"

	req, err := http.NewRequestWithContext(ctx, "GET", metadataURL, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Accept", "application/json")

	resp, err := d.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return nil, fmt.Errorf("metadata fetch failed with status %d: %s", resp.StatusCode, string(body))
	}

	var metadata AuthorizationServerMetadata
	if err := json.NewDecoder(resp.Body).Decode(&metadata); err != nil {
		return nil, fmt.Errorf("failed to decode metadata: %w", err)
	}

	// Validate required fields
	if metadata.Issuer != issuer {
		return nil, fmt.Errorf("issuer mismatch: expected %s, got %s", issuer, metadata.Issuer)
	}

	if metadata.PushedAuthorizationRequestEndpoint == "" {
		return nil, fmt.Errorf("pushed_authorization_request_endpoint is required")
	}

	return &metadata, nil
}

// DiscoverServerMetadataFromHandle is a convenience function for discovering server metadata from a handle
func DiscoverServerMetadataFromHandle(ctx context.Context, handle string) (*AuthorizationServerMetadata, error) {
	discovery := NewMetadataDiscovery()
	metadata, _, err := discovery.DiscoverFromHandle(ctx, handle)
	return metadata, err
}

// DiscoverServerMetadataFromIssuer is a convenience function for discovering server metadata from an issuer (entryway)
// Use this when you want to connect to a known entryway like https://bsky.social without a specific handle
func DiscoverServerMetadataFromIssuer(ctx context.Context, issuer string) (*AuthorizationServerMetadata, error) {
	discovery := NewMetadataDiscovery()
	return discovery.fetchAuthorizationServerMetadata(ctx, issuer)
}
