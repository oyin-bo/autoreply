package auth

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"
)

// AtProtoOAuthClient handles AT Protocol OAuth flows with DPoP
type AtProtoOAuthClient struct {
	clientID   string
	httpClient *http.Client
}

// NewAtProtoOAuthClient creates a new AT Protocol OAuth client
func NewAtProtoOAuthClient(clientID string) *AtProtoOAuthClient {
	return &AtProtoOAuthClient{
		clientID: clientID,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// OAuthServerMetadata represents OAuth server metadata from .well-known discovery
type OAuthServerMetadata struct {
	Issuer                                string   `json:"issuer"`
	AuthorizationEndpoint                 string   `json:"authorization_endpoint"`
	TokenEndpoint                         string   `json:"token_endpoint"`
	PushedAuthorizationRequestEndpoint    *string  `json:"pushed_authorization_request_endpoint,omitempty"`
	DPoPSigningAlgValuesSupported         []string `json:"dpop_signing_alg_values_supported,omitempty"`
	ResponseTypesSupported                []string `json:"response_types_supported"`
	GrantTypesSupported                   []string `json:"grant_types_supported"`
	CodeChallengeMethodsSupported         []string `json:"code_challenge_methods_supported"`
}

// PARResponse represents a Pushed Authorization Request response
type PARResponse struct {
	RequestURI string `json:"request_uri"`
	ExpiresIn  int64  `json:"expires_in"`
}

// DiscoverMetadata discovers OAuth server metadata for a PDS
func (c *AtProtoOAuthClient) DiscoverMetadata(ctx context.Context, pdsURL string) (*OAuthServerMetadata, error) {
	metadataURL := fmt.Sprintf("%s/.well-known/oauth-authorization-server", pdsURL)

	// Create request with timeout context
	timeoutCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	req, err := http.NewRequestWithContext(timeoutCtx, "GET", metadataURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		// Provide more detailed error message
		if timeoutCtx.Err() == context.DeadlineExceeded {
			return nil, fmt.Errorf("OAuth metadata discovery timed out after 10 seconds. The server at %s may not support AT Protocol OAuth yet, or the endpoint is unreachable. Please use --method password instead", pdsURL)
		}
		return nil, fmt.Errorf("failed to fetch OAuth metadata from %s: %w. The server may not support AT Protocol OAuth yet. Please try --method password instead", metadataURL, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("OAuth metadata discovery failed with status %d from %s. Response: %s. The server may not support AT Protocol OAuth yet. Please try --method password instead", resp.StatusCode, metadataURL, string(body))
	}

	var metadata OAuthServerMetadata
	if err := json.NewDecoder(resp.Body).Decode(&metadata); err != nil {
		return nil, fmt.Errorf("failed to decode OAuth metadata from %s: %w", metadataURL, err)
	}

	return &metadata, nil
}

// SendPAR sends a Pushed Authorization Request with DPoP
func (c *AtProtoOAuthClient) SendPAR(
	ctx context.Context,
	metadata *OAuthServerMetadata,
	pkce *PKCEParams,
	dpopKeyPair *DPoPKeyPair,
	handle string,
) (*PARResponse, error) {
	if metadata.PushedAuthorizationRequestEndpoint == nil {
		return nil, fmt.Errorf("PAR endpoint not found in metadata")
	}

	parEndpoint := *metadata.PushedAuthorizationRequestEndpoint

	// Create DPoP proof for PAR request
	dpopProof, err := dpopKeyPair.CreateDPoPProof("POST", parEndpoint, nil, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Build PAR request parameters
	data := url.Values{}
	data.Set("response_type", "code")
	data.Set("client_id", c.clientID)
	data.Set("code_challenge", pkce.CodeChallenge)
	data.Set("code_challenge_method", "S256")
	data.Set("scope", "atproto transition:generic")
	data.Set("login_hint", handle)

	req, err := http.NewRequestWithContext(ctx, "POST", parEndpoint, bytes.NewBufferString(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create PAR request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("PAR request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("PAR failed with status %d: %s", resp.StatusCode, string(body))
	}

	var parResponse PARResponse
	if err := json.NewDecoder(resp.Body).Decode(&parResponse); err != nil {
		return nil, fmt.Errorf("failed to decode PAR response: %w", err)
	}

	return &parResponse, nil
}

// BuildAuthorizationURL builds authorization URL using PAR response
func (c *AtProtoOAuthClient) BuildAuthorizationURL(
	metadata *OAuthServerMetadata,
	parResponse *PARResponse,
) string {
	return fmt.Sprintf(
		"%s?client_id=%s&request_uri=%s",
		metadata.AuthorizationEndpoint,
		url.QueryEscape(c.clientID),
		url.QueryEscape(parResponse.RequestURI),
	)
}

// ExchangeCodeForTokens exchanges authorization code for tokens using DPoP
func (c *AtProtoOAuthClient) ExchangeCodeForTokens(
	ctx context.Context,
	metadata *OAuthServerMetadata,
	code string,
	codeVerifier string,
	dpopKeyPair *DPoPKeyPair,
	redirectURI string,
) (*TokenResponse, error) {
	return c.exchangeCodeWithNonce(ctx, metadata, code, codeVerifier, dpopKeyPair, redirectURI, nil)
}

// exchangeCodeWithNonce exchanges code with optional DPoP nonce
func (c *AtProtoOAuthClient) exchangeCodeWithNonce(
	ctx context.Context,
	metadata *OAuthServerMetadata,
	code string,
	codeVerifier string,
	dpopKeyPair *DPoPKeyPair,
	redirectURI string,
	nonce *string,
) (*TokenResponse, error) {
	// Create DPoP proof for token request
	dpopProof, err := dpopKeyPair.CreateDPoPProof("POST", metadata.TokenEndpoint, nonce, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Build token request parameters
	data := url.Values{}
	data.Set("grant_type", "authorization_code")
	data.Set("code", code)
	data.Set("redirect_uri", redirectURI)
	data.Set("client_id", c.clientID)
	data.Set("code_verifier", codeVerifier)

	req, err := http.NewRequestWithContext(ctx, "POST", metadata.TokenEndpoint, bytes.NewBufferString(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create token request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("token request failed: %w", err)
	}
	defer resp.Body.Close()

	// Extract DPoP nonce if present (for retry)
	dpopNonce := resp.Header.Get("dpop-nonce")

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)

		// If we got a nonce and 401, retry with nonce
		if resp.StatusCode == http.StatusUnauthorized && dpopNonce != "" && nonce == nil {
			return c.exchangeCodeWithNonce(ctx, metadata, code, codeVerifier, dpopKeyPair, redirectURI, &dpopNonce)
		}

		return nil, fmt.Errorf("token exchange failed with status %d: %s", resp.StatusCode, string(body))
	}

	var tokenResp TokenResponse
	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, fmt.Errorf("failed to decode token response: %w", err)
	}

	// Calculate expiration time
	tokenResp.ExpiresAt = time.Now().Add(time.Duration(tokenResp.ExpiresIn) * time.Second)

	return &tokenResp, nil
}

// RefreshToken refreshes access token using refresh token with DPoP
func (c *AtProtoOAuthClient) RefreshToken(
	ctx context.Context,
	metadata *OAuthServerMetadata,
	refreshToken string,
	dpopKeyPair *DPoPKeyPair,
) (*TokenResponse, error) {
	// Create DPoP proof for token refresh
	dpopProof, err := dpopKeyPair.CreateDPoPProof("POST", metadata.TokenEndpoint, nil, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Build refresh request parameters
	data := url.Values{}
	data.Set("grant_type", "refresh_token")
	data.Set("refresh_token", refreshToken)
	data.Set("client_id", c.clientID)

	req, err := http.NewRequestWithContext(ctx, "POST", metadata.TokenEndpoint, bytes.NewBufferString(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create refresh request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("token refresh failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("token refresh failed with status %d: %s", resp.StatusCode, string(body))
	}

	var tokenResp TokenResponse
	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, fmt.Errorf("failed to decode refresh response: %w", err)
	}

	// Calculate expiration time
	tokenResp.ExpiresAt = time.Now().Add(time.Duration(tokenResp.ExpiresIn) * time.Second)

	return &tokenResp, nil
}

// MakeAuthenticatedRequest makes an authenticated API request with DPoP
func (c *AtProtoOAuthClient) MakeAuthenticatedRequest(
	ctx context.Context,
	method string,
	apiURL string,
	accessToken string,
	dpopKeyPair *DPoPKeyPair,
	body io.Reader,
) (*http.Response, error) {
	// Calculate access token hash for DPoP
	ath := CalculateAccessTokenHash(accessToken)

	// Create DPoP proof
	dpopProof, err := dpopKeyPair.CreateDPoPProof(method, apiURL, nil, &ath)
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, method, apiURL, body)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("DPoP %s", accessToken))
	req.Header.Set("DPoP", dpopProof)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("authenticated request failed: %w", err)
	}

	return resp, nil
}
