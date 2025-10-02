package auth

import (
	"strings"
	"testing"
)

func TestGenerateDPoPKeyPair(t *testing.T) {
	kp, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	if kp.PrivateKey == nil {
		t.Error("Private key is nil")
	}

	if kp.PublicKey == nil {
		t.Error("Public key is nil")
	}

	if kp.JWKThumbprint == "" {
		t.Error("JWK thumbprint is empty")
	}
}

func TestDPoPKeyPairPEMRoundTrip(t *testing.T) {
	// Generate key pair
	kp1, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	// Export to PEM
	pem, err := kp1.ToPEM()
	if err != nil {
		t.Fatalf("Failed to export to PEM: %v", err)
	}

	if !strings.Contains(pem, "BEGIN EC PRIVATE KEY") {
		t.Error("PEM does not contain expected header")
	}

	// Import from PEM
	kp2, err := DPoPKeyPairFromPEM(pem)
	if err != nil {
		t.Fatalf("Failed to import from PEM: %v", err)
	}

	// Check thumbprints match
	if kp1.JWKThumbprint != kp2.JWKThumbprint {
		t.Errorf("JWK thumbprints don't match: %s vs %s", kp1.JWKThumbprint, kp2.JWKThumbprint)
	}
}

func TestPublicJWK(t *testing.T) {
	kp, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	jwk, err := kp.PublicJWK()
	if err != nil {
		t.Fatalf("Failed to get public JWK: %v", err)
	}

	// Check required fields
	if jwk["kty"] != "EC" {
		t.Error("JWK kty should be EC")
	}

	if jwk["crv"] != "P-256" {
		t.Error("JWK crv should be P-256")
	}

	if jwk["alg"] != "ES256" {
		t.Error("JWK alg should be ES256")
	}

	if jwk["x"] == nil || jwk["x"] == "" {
		t.Error("JWK x coordinate is missing")
	}

	if jwk["y"] == nil || jwk["y"] == "" {
		t.Error("JWK y coordinate is missing")
	}
}

func TestCreateDPoPProof(t *testing.T) {
	kp, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	// Create proof without nonce or ath
	proof, err := kp.CreateDPoPProof("POST", "https://server.example.com/token", nil, nil)
	if err != nil {
		t.Fatalf("Failed to create DPoP proof: %v", err)
	}

	// Basic validation - should have 3 parts (header.payload.signature)
	parts := strings.Split(proof, ".")
	if len(parts) != 3 {
		t.Errorf("DPoP proof should have 3 parts, got %d", len(parts))
	}

	if proof == "" {
		t.Error("DPoP proof is empty")
	}
}

func TestCreateDPoPProofWithNonce(t *testing.T) {
	kp, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	nonce := "test-nonce-123"
	proof, err := kp.CreateDPoPProof("POST", "https://server.example.com/token", &nonce, nil)
	if err != nil {
		t.Fatalf("Failed to create DPoP proof with nonce: %v", err)
	}

	if len(proof) < 100 {
		t.Error("DPoP proof is too short")
	}
}

func TestCreateDPoPProofWithAth(t *testing.T) {
	kp, err := GenerateDPoPKeyPair()
	if err != nil {
		t.Fatalf("Failed to generate key pair: %v", err)
	}

	ath := CalculateAccessTokenHash("test-access-token")
	proof, err := kp.CreateDPoPProof("GET", "https://api.example.com/data", nil, &ath)
	if err != nil {
		t.Fatalf("Failed to create DPoP proof with ath: %v", err)
	}

	if len(proof) < 100 {
		t.Error("DPoP proof is too short")
	}
}

func TestCalculateAccessTokenHash(t *testing.T) {
	token := "test-access-token"
	hash := CalculateAccessTokenHash(token)

	if hash == "" {
		t.Error("Access token hash is empty")
	}

	// Hash should be deterministic
	hash2 := CalculateAccessTokenHash(token)
	if hash != hash2 {
		t.Error("Access token hash is not deterministic")
	}

	// Different tokens should have different hashes
	hash3 := CalculateAccessTokenHash("different-token")
	if hash == hash3 {
		t.Error("Different tokens should have different hashes")
	}
}
