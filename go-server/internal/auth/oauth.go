package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// OAuthClient handles OAuth 2.0 authentication flows for BlueSky/AT Protocol
type OAuthClient struct {
	clientID     string
	redirectURI  string
	authEndpoint string
	tokenEndpoint string
	httpClient   *http.Client
}

// NewOAuthClient creates a new OAuth client for BlueSky authentication
func NewOAuthClient() *OAuthClient {
	return &OAuthClient{
		clientID:      "autoreply-mcp-client",
		redirectURI:   "http://localhost:8080/callback",
		authEndpoint:  "https://bsky.social/oauth/authorize",
		tokenEndpoint: "https://bsky.social/oauth/token",
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// PKCEParams holds PKCE challenge parameters
type PKCEParams struct {
	CodeVerifier  string
	CodeChallenge string
}

// GeneratePKCE creates PKCE code verifier and challenge
func GeneratePKCE() (*PKCEParams, error) {
	// Generate 32-byte random code verifier
	verifier := make([]byte, 32)
	if _, err := rand.Read(verifier); err != nil {
		return nil, fmt.Errorf("failed to generate code verifier: %w", err)
	}
	
	codeVerifier := base64.RawURLEncoding.EncodeToString(verifier)
	
	// Create SHA256 challenge
	h := sha256.New()
	h.Write([]byte(codeVerifier))
	challenge := base64.RawURLEncoding.EncodeToString(h.Sum(nil))
	
	return &PKCEParams{
		CodeVerifier:  codeVerifier,
		CodeChallenge: challenge,
	}, nil
}

// AuthorizationRequest contains parameters for starting OAuth flow
type AuthorizationRequest struct {
	Handle       string
	CallbackPort int
	State        string
	PKCEParams   *PKCEParams
}

// AuthorizationResponse contains the authorization URL and state
type AuthorizationResponse struct {
	AuthURL      string
	State        string
	CodeVerifier string
}

// StartAuthorizationFlow initiates PKCE OAuth flow
func (oc *OAuthClient) StartAuthorizationFlow(req *AuthorizationRequest) (*AuthorizationResponse, error) {
	if req.PKCEParams == nil {
		pkce, err := GeneratePKCE()
		if err != nil {
			return nil, err
		}
		req.PKCEParams = pkce
	}
	
	if req.State == "" {
		stateBytes := make([]byte, 16)
		if _, err := rand.Read(stateBytes); err != nil {
			return nil, fmt.Errorf("failed to generate state: %w", err)
		}
		req.State = base64.RawURLEncoding.EncodeToString(stateBytes)
	}
	
	if req.CallbackPort > 0 {
		oc.redirectURI = fmt.Sprintf("http://localhost:%d/callback", req.CallbackPort)
	}
	
	params := url.Values{}
	params.Set("response_type", "code")
	params.Set("client_id", oc.clientID)
	params.Set("redirect_uri", oc.redirectURI)
	params.Set("scope", "atproto transition:generic")
	params.Set("state", req.State)
	params.Set("code_challenge", req.PKCEParams.CodeChallenge)
	params.Set("code_challenge_method", "S256")
	
	if req.Handle != "" {
		params.Set("login_hint", req.Handle)
	}
	
	authURL := fmt.Sprintf("%s?%s", oc.authEndpoint, params.Encode())
	
	return &AuthorizationResponse{
		AuthURL:      authURL,
		State:        req.State,
		CodeVerifier: req.PKCEParams.CodeVerifier,
	}, nil
}

// TokenRequest contains parameters for token exchange
type TokenRequest struct {
	Code         string
	CodeVerifier string
}

// TokenResponse contains OAuth tokens
type TokenResponse struct {
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	TokenType    string    `json:"token_type"`
	ExpiresIn    int       `json:"expires_in"`
	Scope        string    `json:"scope"`
	DID          string    `json:"sub"`
	ExpiresAt    time.Time `json:"-"`
}

// ExchangeCodeForToken exchanges authorization code for access token
func (oc *OAuthClient) ExchangeCodeForToken(ctx context.Context, req *TokenRequest) (*TokenResponse, error) {
	data := url.Values{}
	data.Set("grant_type", "authorization_code")
	data.Set("code", req.Code)
	data.Set("redirect_uri", oc.redirectURI)
	data.Set("client_id", oc.clientID)
	data.Set("code_verifier", req.CodeVerifier)
	
	httpReq, err := http.NewRequestWithContext(ctx, "POST", oc.tokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create token request: %w", err)
	}
	
	httpReq.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	httpReq.Header.Set("Accept", "application/json")
	
	resp, err := oc.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("token request failed: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
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

// RefreshTokenRequest contains parameters for token refresh
type RefreshTokenRequest struct {
	RefreshToken string
}

// RefreshAccessToken refreshes an expired access token
func (oc *OAuthClient) RefreshAccessToken(ctx context.Context, req *RefreshTokenRequest) (*TokenResponse, error) {
	data := url.Values{}
	data.Set("grant_type", "refresh_token")
	data.Set("refresh_token", req.RefreshToken)
	data.Set("client_id", oc.clientID)
	
	httpReq, err := http.NewRequestWithContext(ctx, "POST", oc.tokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create refresh request: %w", err)
	}
	
	httpReq.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	httpReq.Header.Set("Accept", "application/json")
	
	resp, err := oc.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("refresh request failed: %w", err)
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

// DeviceAuthorizationRequest contains parameters for device flow
type DeviceAuthorizationRequest struct {
	Handle string
}

// DeviceAuthorizationResponse contains device flow details
type DeviceAuthorizationResponse struct {
	DeviceCode              string `json:"device_code"`
	UserCode                string `json:"user_code"`
	VerificationURI         string `json:"verification_uri"`
	VerificationURIComplete string `json:"verification_uri_complete,omitempty"`
	ExpiresIn               int    `json:"expires_in"`
	Interval                int    `json:"interval"`
}

// StartDeviceFlow initiates OAuth device authorization flow
func (oc *OAuthClient) StartDeviceFlow(ctx context.Context, req *DeviceAuthorizationRequest) (*DeviceAuthorizationResponse, error) {
	deviceEndpoint := strings.Replace(oc.tokenEndpoint, "/token", "/device/code", 1)
	
	data := url.Values{}
	data.Set("client_id", oc.clientID)
	data.Set("scope", "atproto transition:generic")
	
	if req.Handle != "" {
		data.Set("login_hint", req.Handle)
	}
	
	httpReq, err := http.NewRequestWithContext(ctx, "POST", deviceEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create device auth request: %w", err)
	}
	
	httpReq.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	httpReq.Header.Set("Accept", "application/json")
	
	resp, err := oc.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("device auth request failed: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("device authorization failed with status %d: %s", resp.StatusCode, string(body))
	}
	
	var deviceResp DeviceAuthorizationResponse
	if err := json.NewDecoder(resp.Body).Decode(&deviceResp); err != nil {
		return nil, fmt.Errorf("failed to decode device auth response: %w", err)
	}
	
	// Set default polling interval if not provided
	if deviceResp.Interval == 0 {
		deviceResp.Interval = 5
	}
	
	return &deviceResp, nil
}

// PollDeviceTokenRequest contains parameters for polling device token
type PollDeviceTokenRequest struct {
	DeviceCode string
}

// PollDeviceToken polls for device authorization completion
func (oc *OAuthClient) PollDeviceToken(ctx context.Context, req *PollDeviceTokenRequest) (*TokenResponse, error) {
	data := url.Values{}
	data.Set("grant_type", "urn:ietf:params:oauth:grant-type:device_code")
	data.Set("device_code", req.DeviceCode)
	data.Set("client_id", oc.clientID)
	
	httpReq, err := http.NewRequestWithContext(ctx, "POST", oc.tokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create poll request: %w", err)
	}
	
	httpReq.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	httpReq.Header.Set("Accept", "application/json")
	
	resp, err := oc.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("poll request failed: %w", err)
	}
	defer resp.Body.Close()
	
	body, _ := io.ReadAll(resp.Body)
	
	// Handle pending authorization
	if resp.StatusCode == http.StatusBadRequest {
		var errResp struct {
			Error string `json:"error"`
		}
		json.Unmarshal(body, &errResp)
		
		if errResp.Error == "authorization_pending" {
			return nil, ErrAuthorizationPending
		} else if errResp.Error == "slow_down" {
			return nil, ErrSlowDown
		} else if errResp.Error == "expired_token" {
			return nil, ErrExpiredToken
		} else if errResp.Error == "access_denied" {
			return nil, ErrAccessDenied
		}
	}
	
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("device token poll failed with status %d: %s", resp.StatusCode, string(body))
	}
	
	var tokenResp TokenResponse
	if err := json.Unmarshal(body, &tokenResp); err != nil {
		return nil, fmt.Errorf("failed to decode token response: %w", err)
	}
	
	// Calculate expiration time
	tokenResp.ExpiresAt = time.Now().Add(time.Duration(tokenResp.ExpiresIn) * time.Second)
	
	return &tokenResp, nil
}
