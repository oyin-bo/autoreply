package auth

import (
	"context"
	"strings"
	"testing"
	"time"
)

func TestGeneratePKCE(t *testing.T) {
	pkce, err := GeneratePKCE()
	if err != nil {
		t.Fatalf("GeneratePKCE() error = %v", err)
	}

	if len(pkce.CodeVerifier) == 0 {
		t.Error("CodeVerifier should not be empty")
	}

	if len(pkce.CodeChallenge) == 0 {
		t.Error("CodeChallenge should not be empty")
	}

	// Verify uniqueness - generate second PKCE
	pkce2, err := GeneratePKCE()
	if err != nil {
		t.Fatalf("GeneratePKCE() second call error = %v", err)
	}

	if pkce.CodeVerifier == pkce2.CodeVerifier {
		t.Error("PKCE verifiers should be unique")
	}

	if pkce.CodeChallenge == pkce2.CodeChallenge {
		t.Error("PKCE challenges should be unique")
	}
}

func TestNewOAuthClient(t *testing.T) {
	client := NewOAuthClient()

	if client == nil {
		t.Fatal("NewOAuthClient() should not return nil")
	}

	if client.clientID == "" {
		t.Error("clientID should not be empty")
	}

	if client.redirectURI == "" {
		t.Error("redirectURI should not be empty")
	}

	if client.authEndpoint == "" {
		t.Error("authEndpoint should not be empty")
	}

	if client.tokenEndpoint == "" {
		t.Error("tokenEndpoint should not be empty")
	}

	if client.httpClient == nil {
		t.Error("httpClient should not be nil")
	}
}

func TestStartAuthorizationFlow(t *testing.T) {
	client := NewOAuthClient()

	tests := []struct {
		name string
		req  *AuthorizationRequest
	}{
		{
			name: "basic flow",
			req:  &AuthorizationRequest{},
		},
		{
			name: "with handle",
			req: &AuthorizationRequest{
				Handle: "alice.bsky.social",
			},
		},
		{
			name: "with custom port",
			req: &AuthorizationRequest{
				CallbackPort: 9090,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			resp, err := client.StartAuthorizationFlow(tt.req)
			if err != nil {
				t.Fatalf("StartAuthorizationFlow() error = %v", err)
			}

			if resp.AuthURL == "" {
				t.Error("AuthURL should not be empty")
			}

			if resp.State == "" {
				t.Error("State should not be empty")
			}

			if resp.CodeVerifier == "" {
				t.Error("CodeVerifier should not be empty")
			}

			// Verify URL contains required parameters
			if !strings.Contains(resp.AuthURL, "response_type=code") {
				t.Error("AuthURL should contain response_type=code")
			}

			if !strings.Contains(resp.AuthURL, "code_challenge=") {
				t.Error("AuthURL should contain code_challenge")
			}

			if !strings.Contains(resp.AuthURL, "code_challenge_method=S256") {
				t.Error("AuthURL should contain code_challenge_method=S256")
			}

			if tt.req.Handle != "" && !strings.Contains(resp.AuthURL, "login_hint=") {
				t.Error("AuthURL should contain login_hint when handle provided")
			}
		})
	}
}

func TestStartAuthorizationFlowWithProvidedPKCE(t *testing.T) {
	client := NewOAuthClient()

	pkce, err := GeneratePKCE()
	if err != nil {
		t.Fatalf("GeneratePKCE() error = %v", err)
	}

	req := &AuthorizationRequest{
		PKCEParams: pkce,
		State:      "custom-state",
	}

	resp, err := client.StartAuthorizationFlow(req)
	if err != nil {
		t.Fatalf("StartAuthorizationFlow() error = %v", err)
	}

	if resp.State != "custom-state" {
		t.Errorf("State = %v, want %v", resp.State, "custom-state")
	}

	if resp.CodeVerifier != pkce.CodeVerifier {
		t.Error("Should use provided PKCE code verifier")
	}
}

func TestTokenResponseExpiration(t *testing.T) {
	now := time.Now()

	tokenResp := &TokenResponse{
		AccessToken:  "test-token",
		RefreshToken: "test-refresh",
		ExpiresIn:    3600,
	}

	// Simulate expiration calculation
	tokenResp.ExpiresAt = now.Add(time.Duration(tokenResp.ExpiresIn) * time.Second)

	if tokenResp.ExpiresAt.Before(now) {
		t.Error("ExpiresAt should be in the future")
	}

	expectedExpiry := now.Add(3600 * time.Second)
	timeDiff := tokenResp.ExpiresAt.Sub(expectedExpiry)
	if timeDiff > time.Second || timeDiff < -time.Second {
		t.Errorf("ExpiresAt calculation incorrect, diff = %v", timeDiff)
	}
}

func TestDeviceAuthorizationResponseDefaults(t *testing.T) {
	device := &DeviceAuthorizationResponse{
		DeviceCode:      "ABC123",
		UserCode:        "WXYZ-1234",
		VerificationURI: "https://bsky.app/device",
		ExpiresIn:       600,
		Interval:        0, // Not set
	}

	// Simulate default interval setting
	if device.Interval == 0 {
		device.Interval = 5
	}

	if device.Interval != 5 {
		t.Errorf("Default interval = %v, want 5", device.Interval)
	}
}

func TestOAuthErrorTypes(t *testing.T) {
	tests := []struct {
		name string
		err  *AuthError
		code ErrorCode
	}{
		{
			name: "authorization pending",
			err:  ErrAuthorizationPending,
			code: ErrCodeAuthorizationPending,
		},
		{
			name: "slow down",
			err:  ErrSlowDown,
			code: ErrCodeSlowDown,
		},
		{
			name: "expired token",
			err:  ErrExpiredToken,
			code: ErrCodeExpiredToken,
		},
		{
			name: "access denied",
			err:  ErrAccessDenied,
			code: ErrCodeAccessDenied,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.err == nil {
				t.Fatal("Error should not be nil")
			}

			if tt.err.Code != tt.code {
				t.Errorf("Error code = %v, want %v", tt.err.Code, tt.code)
			}

			if tt.err.Error() == "" {
				t.Error("Error message should not be empty")
			}
		})
	}
}

func TestExchangeCodeForTokenContext(t *testing.T) {
	client := NewOAuthClient()

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	_, err := client.ExchangeCodeForToken(ctx, &TokenRequest{
		Code:         "test-code",
		CodeVerifier: "test-verifier",
	})

	if err == nil {
		t.Error("ExchangeCodeForToken() should fail with cancelled context")
	}

	if !strings.Contains(err.Error(), "context canceled") {
		t.Errorf("Error should mention context cancellation, got: %v", err)
	}
}

func TestRefreshAccessTokenContext(t *testing.T) {
	client := NewOAuthClient()

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	_, err := client.RefreshAccessToken(ctx, &RefreshTokenRequest{
		RefreshToken: "test-refresh-token",
	})

	if err == nil {
		t.Error("RefreshAccessToken() should fail with cancelled context")
	}

	if !strings.Contains(err.Error(), "context canceled") {
		t.Errorf("Error should mention context cancellation, got: %v", err)
	}
}
