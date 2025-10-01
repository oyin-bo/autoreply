// Package auth provides authentication and credential management
package auth

import (
	"strings"
	"testing"
)

func TestGenerateDPoPKey(t *testing.T) {
	key, err := GenerateDPoPKey()
	if err != nil {
		t.Fatalf("Failed to generate DPoP key: %v", err)
	}

	if key == nil {
		t.Fatal("Expected non-nil key")
	}

	if key.PrivateKey == nil {
		t.Error("Expected non-nil private key")
	}

	if key.PublicKey == nil {
		t.Error("Expected non-nil public key")
	}

	// Verify public key matches private key
	if key.PrivateKey.PublicKey.X.Cmp(key.PublicKey.X) != 0 {
		t.Error("Public key X coordinate mismatch")
	}

	if key.PrivateKey.PublicKey.Y.Cmp(key.PublicKey.Y) != 0 {
		t.Error("Public key Y coordinate mismatch")
	}
}

func TestCreateDPoPProof(t *testing.T) {
	key, err := GenerateDPoPKey()
	if err != nil {
		t.Fatalf("Failed to generate DPoP key: %v", err)
	}

	// Create DPoP proof without access token
	proof, err := key.CreateDPoPProof("POST", "https://example.com/token", "")
	if err != nil {
		t.Fatalf("Failed to create DPoP proof: %v", err)
	}

	if proof == "" {
		t.Error("Expected non-empty DPoP proof")
	}

	// Verify JWT structure (header.payload.signature)
	parts := strings.Split(proof, ".")
	if len(parts) != 3 {
		t.Errorf("Expected 3 parts in JWT, got %d", len(parts))
	}

	// Verify each part is base64url encoded
	for i, part := range parts {
		if strings.Contains(part, "+") || strings.Contains(part, "/") || strings.Contains(part, "=") {
			t.Errorf("Part %d should be base64url encoded without padding", i)
		}
	}
}

func TestCreateDPoPProofWithAccessToken(t *testing.T) {
	key, err := GenerateDPoPKey()
	if err != nil {
		t.Fatalf("Failed to generate DPoP key: %v", err)
	}

	// Create DPoP proof with access token
	accessToken := "test-access-token"
	proof, err := key.CreateDPoPProof("GET", "https://example.com/api", accessToken)
	if err != nil {
		t.Fatalf("Failed to create DPoP proof: %v", err)
	}

	if proof == "" {
		t.Error("Expected non-empty DPoP proof")
	}

	// Verify JWT structure
	parts := strings.Split(proof, ".")
	if len(parts) != 3 {
		t.Errorf("Expected 3 parts in JWT, got %d", len(parts))
	}
}

func TestJWKThumbprint(t *testing.T) {
	key, err := GenerateDPoPKey()
	if err != nil {
		t.Fatalf("Failed to generate DPoP key: %v", err)
	}

	thumbprint, err := key.JWKThumbprint()
	if err != nil {
		t.Fatalf("Failed to generate JWK thumbprint: %v", err)
	}

	if thumbprint == "" {
		t.Error("Expected non-empty thumbprint")
	}

	// Verify thumbprint is base64url encoded
	if strings.Contains(thumbprint, "+") || strings.Contains(thumbprint, "/") || strings.Contains(thumbprint, "=") {
		t.Error("Thumbprint should be base64url encoded without padding")
	}

	// Generate thumbprint again and verify it's the same (deterministic)
	thumbprint2, err := key.JWKThumbprint()
	if err != nil {
		t.Fatalf("Failed to generate second JWK thumbprint: %v", err)
	}

	if thumbprint != thumbprint2 {
		t.Error("JWK thumbprint should be deterministic")
	}
}

func TestDPoPKeyUniqueness(t *testing.T) {
	// Generate multiple keys and verify they're all unique
	seen := make(map[string]bool)

	for i := 0; i < 5; i++ {
		key, err := GenerateDPoPKey()
		if err != nil {
			t.Fatalf("Failed to generate DPoP key: %v", err)
		}

		thumbprint, err := key.JWKThumbprint()
		if err != nil {
			t.Fatalf("Failed to generate JWK thumbprint: %v", err)
		}

		if seen[thumbprint] {
			t.Error("Generated duplicate DPoP key")
		}
		seen[thumbprint] = true
	}
}
