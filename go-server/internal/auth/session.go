// Package auth provides authentication and credential management
package auth

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"
)

// SessionManager handles AT Protocol authentication sessions
type SessionManager struct {
	client *http.Client
}

// NewSessionManager creates a new session manager
func NewSessionManager() *SessionManager {
	return &SessionManager{
		client: &http.Client{
			Timeout: 120 * time.Second,
		},
	}
}

// CreateSessionRequest represents the request to create a session
type CreateSessionRequest struct {
	Identifier string `json:"identifier"`
	Password   string `json:"password"`
}

// CreateSessionResponse represents the response from creating a session
type CreateSessionResponse struct {
	AccessJwt  string `json:"accessJwt"`
	RefreshJwt string `json:"refreshJwt"`
	Handle     string `json:"handle"`
	DID        string `json:"did"`
	Email      string `json:"email,omitempty"`
}

// RefreshSessionResponse represents the response from refreshing a session
type RefreshSessionResponse struct {
	AccessJwt  string `json:"accessJwt"`
	RefreshJwt string `json:"refreshJwt"`
	Handle     string `json:"handle"`
	DID        string `json:"did"`
}

// CreateSession authenticates with identifier and password (app password)
func (m *SessionManager) CreateSession(ctx context.Context, identifier, password string) (*Credentials, error) {
	// Use bsky.social as the default PDS for session creation
	// In production, you might want to resolve the user's actual PDS
	endpoint := "https://bsky.social/xrpc/com.atproto.server.createSession"

	reqBody := CreateSessionRequest{
		Identifier: identifier,
		Password:   password,
	}

	jsonData, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", endpoint, bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := m.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to create session: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, fmt.Errorf("authentication failed with status %d: %v", resp.StatusCode, errorResp)
	}

	var sessionResp CreateSessionResponse
	if err := json.NewDecoder(resp.Body).Decode(&sessionResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &Credentials{
		Handle:       sessionResp.Handle,
		AccessToken:  sessionResp.AccessJwt,
		RefreshToken: sessionResp.RefreshJwt,
		DID:          sessionResp.DID,
	}, nil
}

// RefreshSession refreshes an expired session using a refresh token
func (m *SessionManager) RefreshSession(ctx context.Context, refreshToken string) (*Credentials, error) {
	endpoint := "https://bsky.social/xrpc/com.atproto.server.refreshSession"

	req, err := http.NewRequestWithContext(ctx, "POST", endpoint, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", refreshToken))
	req.Header.Set("User-Agent", "autoreply/1.0")

	resp, err := m.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to refresh session: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errorResp map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&errorResp)
		return nil, fmt.Errorf("refresh failed with status %d: %v", resp.StatusCode, errorResp)
	}

	var sessionResp RefreshSessionResponse
	if err := json.NewDecoder(resp.Body).Decode(&sessionResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &Credentials{
		Handle:       sessionResp.Handle,
		AccessToken:  sessionResp.AccessJwt,
		RefreshToken: sessionResp.RefreshJwt,
		DID:          sessionResp.DID,
	}, nil
}
