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

// OAuthConfig holds OAuth configuration
type OAuthConfig struct {
	AuthorizationEndpoint string
	TokenEndpoint         string
	ClientID              string
	RedirectURI           string
	Scope                 string
}

// OAuthFlow handles OAuth 2.0 with PKCE and DPoP flow
type OAuthFlow struct {
	config     *OAuthConfig
	dpopKey    *DPoPKey
	verifier   string
	challenge  string
	state      string
	httpClient *http.Client
}

// NewOAuthFlow creates a new OAuth flow
func NewOAuthFlow(config *OAuthConfig) (*OAuthFlow, error) {
	// Generate DPoP key
	dpopKey, err := GenerateDPoPKey()
	if err != nil {
		return nil, fmt.Errorf("failed to generate DPoP key: %w", err)
	}

	// Generate PKCE challenge
	verifier, challenge, err := GeneratePKCEChallenge()
	if err != nil {
		return nil, fmt.Errorf("failed to generate PKCE challenge: %w", err)
	}

	// Generate state
	state, err := GenerateState()
	if err != nil {
		return nil, fmt.Errorf("failed to generate state: %w", err)
	}

	return &OAuthFlow{
		config:     config,
		dpopKey:    dpopKey,
		verifier:   verifier,
		challenge:  challenge,
		state:      state,
		httpClient: &http.Client{Timeout: 30 * time.Second},
	}, nil
}

// GetAuthorizationURL returns the authorization URL for the user to visit
func (f *OAuthFlow) GetAuthorizationURL() string {
	params := url.Values{}
	params.Set("response_type", "code")
	params.Set("client_id", f.config.ClientID)
	params.Set("redirect_uri", f.config.RedirectURI)
	params.Set("scope", f.config.Scope)
	params.Set("state", f.state)
	params.Set("code_challenge", f.challenge)
	params.Set("code_challenge_method", "S256")

	return f.config.AuthorizationEndpoint + "?" + params.Encode()
}

// ExchangeCode exchanges the authorization code for tokens
func (f *OAuthFlow) ExchangeCode(ctx context.Context, code, state string) (*Credentials, error) {
	// Verify state
	if state != f.state {
		return nil, fmt.Errorf("state mismatch: expected %s, got %s", f.state, state)
	}

	// Create DPoP proof for token endpoint
	dpopProof, err := f.dpopKey.CreateDPoPProof("POST", f.config.TokenEndpoint, "")
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Prepare token request
	data := url.Values{}
	data.Set("grant_type", "authorization_code")
	data.Set("code", code)
	data.Set("redirect_uri", f.config.RedirectURI)
	data.Set("code_verifier", f.verifier)
	data.Set("client_id", f.config.ClientID)

	req, err := http.NewRequestWithContext(ctx, "POST", f.config.TokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create token request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)
	req.Header.Set("User-Agent", "autoreply/1.0")

	// Send request
	resp, err := f.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to exchange code: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("token exchange failed with status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var tokenResp struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
		TokenType    string `json:"token_type"`
		ExpiresIn    int    `json:"expires_in"`
		Scope        string `json:"scope"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, fmt.Errorf("failed to parse token response: %w", err)
	}

	// Verify token type
	if tokenResp.TokenType != "DPoP" {
		return nil, fmt.Errorf("unexpected token type: %s (expected DPoP)", tokenResp.TokenType)
	}

	// TODO: Extract handle and DID from token or make additional API call
	// For now, return credentials with tokens
	return &Credentials{
		Handle:       "", // Will need to be set by caller
		AccessToken:  tokenResp.AccessToken,
		RefreshToken: tokenResp.RefreshToken,
		DID:          "", // Will need to be set by caller
	}, nil
}

// RefreshToken refreshes an expired access token using the refresh token
func (f *OAuthFlow) RefreshToken(ctx context.Context, refreshToken string) (*Credentials, error) {
	// Create DPoP proof for token endpoint
	dpopProof, err := f.dpopKey.CreateDPoPProof("POST", f.config.TokenEndpoint, "")
	if err != nil {
		return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Prepare refresh request
	data := url.Values{}
	data.Set("grant_type", "refresh_token")
	data.Set("refresh_token", refreshToken)
	data.Set("client_id", f.config.ClientID)

	req, err := http.NewRequestWithContext(ctx, "POST", f.config.TokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create refresh request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)
	req.Header.Set("User-Agent", "autoreply/1.0")

	// Send request
	resp, err := f.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to refresh token: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("token refresh failed with status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var tokenResp struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
		TokenType    string `json:"token_type"`
		ExpiresIn    int    `json:"expires_in"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, fmt.Errorf("failed to parse refresh response: %w", err)
	}

	return &Credentials{
		AccessToken:  tokenResp.AccessToken,
		RefreshToken: tokenResp.RefreshToken,
	}, nil
}

// GetDPoPKey returns the DPoP key for making authenticated requests
func (f *OAuthFlow) GetDPoPKey() *DPoPKey {
	return f.dpopKey
}
