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

// DeviceAuthConfig holds device authorization configuration
type DeviceAuthConfig struct {
	DeviceAuthorizationEndpoint string
	TokenEndpoint               string
	ClientID                    string
	Scope                       string
}

// DeviceCodeResponse represents the device code response
type DeviceCodeResponse struct {
	DeviceCode              string `json:"device_code"`
	UserCode                string `json:"user_code"`
	VerificationURI         string `json:"verification_uri"`
	VerificationURIComplete string `json:"verification_uri_complete,omitempty"`
	ExpiresIn               int    `json:"expires_in"`
	Interval                int    `json:"interval"`
}

// DeviceAuthFlow handles Device Authorization Grant flow
type DeviceAuthFlow struct {
	config     *DeviceAuthConfig
	dpopKey    *DPoPKey
	httpClient *http.Client
}

// NewDeviceAuthFlow creates a new device authorization flow
func NewDeviceAuthFlow(config *DeviceAuthConfig) (*DeviceAuthFlow, error) {
	// Generate DPoP key
	dpopKey, err := GenerateDPoPKey()
	if err != nil {
		return nil, fmt.Errorf("failed to generate DPoP key: %w", err)
	}

	return &DeviceAuthFlow{
		config:     config,
		dpopKey:    dpopKey,
		httpClient: &http.Client{Timeout: 30 * time.Second},
	}, nil
}

// RequestDeviceCode requests a device code from the authorization server
func (f *DeviceAuthFlow) RequestDeviceCode(ctx context.Context) (*DeviceCodeResponse, error) {
	// Prepare request
	data := url.Values{}
	data.Set("client_id", f.config.ClientID)
	data.Set("scope", f.config.Scope)

	req, err := http.NewRequestWithContext(ctx, "POST", f.config.DeviceAuthorizationEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, fmt.Errorf("failed to create device code request: %w", err)
	}

	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	req.Header.Set("User-Agent", "autoreply/1.0")

	// Send request
	resp, err := f.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to request device code: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("device code request failed with status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var deviceResp DeviceCodeResponse
	if err := json.NewDecoder(resp.Body).Decode(&deviceResp); err != nil {
		return nil, fmt.Errorf("failed to parse device code response: %w", err)
	}

	// Set default interval if not provided
	if deviceResp.Interval == 0 {
		deviceResp.Interval = 5
	}

	return &deviceResp, nil
}

// PollForToken polls for the authorization token
func (f *DeviceAuthFlow) PollForToken(ctx context.Context, deviceCode string, interval int) (*Credentials, error) {
	ticker := time.NewTicker(time.Duration(interval) * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("polling cancelled: %w", ctx.Err())
		case <-ticker.C:
			// Create DPoP proof for token endpoint (with empty nonce for now)
			dpopProof, err := f.dpopKey.CreateDPoPProof("POST", f.config.TokenEndpoint, "", "")
			if err != nil {
				return nil, fmt.Errorf("failed to create DPoP proof: %w", err)
			}

			// Prepare token request
			data := url.Values{}
			data.Set("grant_type", "urn:ietf:params:oauth:grant-type:device_code")
			data.Set("device_code", deviceCode)
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
				return nil, fmt.Errorf("failed to poll for token: %w", err)
			}

			// Read response body
			body, err := io.ReadAll(resp.Body)
			resp.Body.Close()
			if err != nil {
				return nil, fmt.Errorf("failed to read response: %w", err)
			}

			// Handle different status codes
			if resp.StatusCode == http.StatusOK {
				// Success - parse token response
				var tokenResp struct {
					AccessToken  string `json:"access_token"`
					RefreshToken string `json:"refresh_token"`
					TokenType    string `json:"token_type"`
					ExpiresIn    int    `json:"expires_in"`
				}

				if err := json.Unmarshal(body, &tokenResp); err != nil {
					return nil, fmt.Errorf("failed to parse token response: %w", err)
				}

				// Verify token type
				if tokenResp.TokenType != "DPoP" {
					return nil, fmt.Errorf("unexpected token type: %s (expected DPoP)", tokenResp.TokenType)
				}

				// TODO: Extract handle and DID from token or make additional API call
				return &Credentials{
					Handle:       "", // Will need to be set by caller
					AccessToken:  tokenResp.AccessToken,
					RefreshToken: tokenResp.RefreshToken,
					DID:          "", // Will need to be set by caller
				}, nil
			}

			// Parse error response
			var errorResp struct {
				Error            string `json:"error"`
				ErrorDescription string `json:"error_description,omitempty"`
			}
			if err := json.Unmarshal(body, &errorResp); err != nil {
				return nil, fmt.Errorf("failed to parse error response: %w", err)
			}

			// Handle specific error codes
			switch errorResp.Error {
			case "authorization_pending":
				// User hasn't authorized yet, continue polling
				continue
			case "slow_down":
				// Increase polling interval
				interval += 5
				ticker.Reset(time.Duration(interval) * time.Second)
				continue
			case "expired_token":
				return nil, fmt.Errorf("device code expired")
			case "access_denied":
				return nil, fmt.Errorf("user denied authorization")
			default:
				return nil, fmt.Errorf("token request failed: %s - %s", errorResp.Error, errorResp.ErrorDescription)
			}
		}
	}
}

// GetDPoPKey returns the DPoP key for making authenticated requests
func (f *DeviceAuthFlow) GetDPoPKey() *DPoPKey {
	return f.dpopKey
}
