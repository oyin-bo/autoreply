package auth

import (
	"testing"
)

func TestNewAtProtoOAuthClient(t *testing.T) {
	client := NewAtProtoOAuthClient("test-client-id")

	if client == nil {
		t.Fatal("Client is nil")
	}

	if client.clientID != "test-client-id" {
		t.Errorf("Expected client ID 'test-client-id', got '%s'", client.clientID)
	}

	if client.httpClient == nil {
		t.Error("HTTP client is nil")
	}
}

func TestBuildAuthorizationURL(t *testing.T) {
	client := NewAtProtoOAuthClient("test-client")

	metadata := &OAuthServerMetadata{
		Issuer:                "https://example.com",
		AuthorizationEndpoint: "https://example.com/authorize",
		TokenEndpoint:         "https://example.com/token",
		ResponseTypesSupported: []string{"code"},
		GrantTypesSupported: []string{"authorization_code"},
		CodeChallengeMethodsSupported: []string{"S256"},
	}

	parResponse := &PARResponse{
		RequestURI: "urn:ietf:params:oauth:request_uri:test123",
		ExpiresIn:  90,
	}

	url := client.BuildAuthorizationURL(metadata, parResponse)

	if url == "" {
		t.Error("Authorization URL is empty")
	}

	if !urlContains(url, "request_uri=") {
		t.Error("URL should contain request_uri parameter")
	}

	if !urlContains(url, "client_id=") {
		t.Error("URL should contain client_id parameter")
	}

	if !urlContains(url, "https://example.com/authorize") {
		t.Error("URL should start with authorization endpoint")
	}
}

func urlContains(s, substr string) bool {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
