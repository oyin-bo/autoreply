// Package auth provides authentication and credential management
package auth

import (
	"context"
	"crypto/rand"
	"encoding/base64"
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
	ClientID       string
	RedirectURI    string
	Scope          string
	ServerMetadata *AuthorizationServerMetadata
}

// OAuthFlow handles OAuth 2.0 with PKCE and DPoP flow following AT Protocol spec
type OAuthFlow struct {
	config      *OAuthConfig
	dpopKey     *DPoPKey
	verifier    string
	challenge   string
	state       string
	httpClient  *http.Client
	authNonce   string // DPoP nonce for auth server
	pdsNonce    string // DPoP nonce for PDS
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

// GenerateState generates a secure random state parameter
func GenerateState() (string, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

// PushedAuthorizationResponse represents the PAR response
type PushedAuthorizationResponse struct {
	RequestURI string `json:"request_uri"`
	ExpiresIn  int    `json:"expires_in"`
}

// PushAuthorizationRequest makes a PAR request and returns the request_uri
func (f *OAuthFlow) PushAuthorizationRequest(ctx context.Context, loginHint string) (string, error) {
	parEndpoint := f.config.ServerMetadata.PushedAuthorizationRequestEndpoint

	// Build PAR request parameters
	params := url.Values{}
	params.Set("client_id", f.config.ClientID)
	params.Set("redirect_uri", f.config.RedirectURI)
	params.Set("response_type", "code")
	params.Set("scope", f.config.Scope)
	params.Set("state", f.state)
	params.Set("code_challenge", f.challenge)
	params.Set("code_challenge_method", "S256")
	
	if loginHint != "" {
		params.Set("login_hint", loginHint)
	}

	// Try PAR request (may need to discover nonce first)
	resp, err := f.makePARRequest(ctx, parEndpoint, params)
	if err != nil {
		return "", err
	}

	return resp.RequestURI, nil
}

// makePARRequest makes a PAR request with DPoP, handling nonce discovery
func (f *OAuthFlow) makePARRequest(ctx context.Context, parEndpoint string, params url.Values) (*PushedAuthorizationResponse, error) {
	// Try request with current nonce (empty on first attempt)
	resp, nonce, err := f.attemptPARRequest(ctx, parEndpoint, params, f.authNonce)
	if err != nil && strings.Contains(err.Error(), "use_dpop_nonce") && nonce != "" {
		// Server requires nonce, retry with the provided nonce
		f.authNonce = nonce
		resp, _, err = f.attemptPARRequest(ctx, parEndpoint, params, f.authNonce)
	}

	return resp, err
}

// attemptPARRequest attempts a single PAR request
func (f *OAuthFlow) attemptPARRequest(ctx context.Context, parEndpoint string, params url.Values, nonce string) (*PushedAuthorizationResponse, string, error) {
	// Create DPoP proof
	dpopProof, err := f.dpopKey.CreateDPoPProof("POST", parEndpoint, "", nonce)
	if err != nil {
		return nil, "", fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, "POST", parEndpoint, strings.NewReader(params.Encode()))
	if err != nil {
		return nil, "", err
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)

	// Make request
	httpResp, err := f.httpClient.Do(req)
	if err != nil {
		return nil, "", err
	}
	defer httpResp.Body.Close()

	// Check for DPoP nonce in response
	newNonce := httpResp.Header.Get("DPoP-Nonce")

	// Handle non-200 responses
	if httpResp.StatusCode != http.StatusOK && httpResp.StatusCode != http.StatusCreated {
		body, _ := io.ReadAll(io.LimitReader(httpResp.Body, 4096))
		
		// Check if error is about DPoP nonce
		var errResp struct {
			Error            string `json:"error"`
			ErrorDescription string `json:"error_description"`
		}
		json.Unmarshal(body, &errResp)
		
		if errResp.Error == "use_dpop_nonce" {
			return nil, newNonce, fmt.Errorf("use_dpop_nonce: %s", errResp.ErrorDescription)
		}
		
		return nil, newNonce, fmt.Errorf("PAR request failed with status %d: %s", httpResp.StatusCode, string(body))
	}

	// Parse response
	var parResp PushedAuthorizationResponse
	if err := json.NewDecoder(httpResp.Body).Decode(&parResp); err != nil {
		return nil, newNonce, fmt.Errorf("failed to decode PAR response: %w", err)
	}

	// Update nonce if provided
	if newNonce != "" {
		f.authNonce = newNonce
	}

	return &parResp, newNonce, nil
}

// GetAuthorizationURL returns the authorization URL for the user to visit
func (f *OAuthFlow) GetAuthorizationURL(requestURI string) string {
	params := url.Values{}
	params.Set("client_id", f.config.ClientID)
	params.Set("request_uri", requestURI)

	return f.config.ServerMetadata.AuthorizationEndpoint + "?" + params.Encode()
}

// TokenResponse represents the OAuth token response
type TokenResponse struct {
	AccessToken  string `json:"access_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int    `json:"expires_in"`
	RefreshToken string `json:"refresh_token,omitempty"`
	Scope        string `json:"scope,omitempty"`
	Sub          string `json:"sub"` // DID of the user
}

// ExchangeCode exchanges the authorization code for tokens
func (f *OAuthFlow) ExchangeCode(ctx context.Context, code, callbackState string) (*Credentials, error) {
	// Verify state
	if callbackState != f.state {
		return nil, fmt.Errorf("state mismatch: expected %s, got %s", f.state, callbackState)
	}

	// Build token request
	tokenEndpoint := f.config.ServerMetadata.TokenEndpoint
	params := url.Values{}
	params.Set("grant_type", "authorization_code")
	params.Set("code", code)
	params.Set("redirect_uri", f.config.RedirectURI)
	params.Set("code_verifier", f.verifier)
	params.Set("client_id", f.config.ClientID)

	// Make token request with DPoP
	tokenResp, err := f.makeTokenRequest(ctx, tokenEndpoint, params)
	if err != nil {
		return nil, err
	}

	// Create credentials
	creds := &Credentials{
		DID:          tokenResp.Sub,
		AccessToken:  tokenResp.AccessToken,
		RefreshToken: tokenResp.RefreshToken,
		TokenType:    "DPoP",
		ExpiresAt:    time.Now().Add(time.Duration(tokenResp.ExpiresIn) * time.Second),
		Scope:        tokenResp.Scope,
	}

	return creds, nil
}

// makeTokenRequest makes a token request with DPoP, handling nonce discovery
func (f *OAuthFlow) makeTokenRequest(ctx context.Context, tokenEndpoint string, params url.Values) (*TokenResponse, error) {
	// Try request with current nonce
	resp, nonce, err := f.attemptTokenRequest(ctx, tokenEndpoint, params, f.authNonce)
	if err != nil && strings.Contains(err.Error(), "use_dpop_nonce") && nonce != "" {
		// Server requires nonce, retry with the provided nonce
		f.authNonce = nonce
		resp, _, err = f.attemptTokenRequest(ctx, tokenEndpoint, params, f.authNonce)
	}

	return resp, err
}

// attemptTokenRequest attempts a single token request
func (f *OAuthFlow) attemptTokenRequest(ctx context.Context, tokenEndpoint string, params url.Values, nonce string) (*TokenResponse, string, error) {
	// Create DPoP proof
	dpopProof, err := f.dpopKey.CreateDPoPProof("POST", tokenEndpoint, "", nonce)
	if err != nil {
		return nil, "", fmt.Errorf("failed to create DPoP proof: %w", err)
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, "POST", tokenEndpoint, strings.NewReader(params.Encode()))
	if err != nil {
		return nil, "", err
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("DPoP", dpopProof)

	// Make request
	httpResp, err := f.httpClient.Do(req)
	if err != nil {
		return nil, "", err
	}
	defer httpResp.Body.Close()

	// Check for DPoP nonce in response
	newNonce := httpResp.Header.Get("DPoP-Nonce")

	// Handle non-200 responses
	if httpResp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(httpResp.Body, 4096))
		
		// Check if error is about DPoP nonce
		var errResp struct {
			Error            string `json:"error"`
			ErrorDescription string `json:"error_description"`
		}
		json.Unmarshal(body, &errResp)
		
		if errResp.Error == "use_dpop_nonce" {
			return nil, newNonce, fmt.Errorf("use_dpop_nonce: %s", errResp.ErrorDescription)
		}
		
		return nil, newNonce, fmt.Errorf("token request failed with status %d: %s", httpResp.StatusCode, string(body))
	}

	// Parse response
	var tokenResp TokenResponse
	if err := json.NewDecoder(httpResp.Body).Decode(&tokenResp); err != nil {
		return nil, newNonce, fmt.Errorf("failed to decode token response: %w", err)
	}

	// Update nonce if provided
	if newNonce != "" {
		f.authNonce = newNonce
	}

	return &tokenResp, newNonce, nil
}

// GetState returns the state parameter for verification
func (f *OAuthFlow) GetState() string {
	return f.state
}

// GetDPoPKey returns the DPoP key for making authenticated requests
func (f *OAuthFlow) GetDPoPKey() *DPoPKey {
	return f.dpopKey
}

