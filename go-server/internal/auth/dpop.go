// Package auth provides authentication and credential management
package auth

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"time"
)

// DPoPKey represents an ECDSA key pair for DPoP
type DPoPKey struct {
	PrivateKey *ecdsa.PrivateKey
	PublicKey  *ecdsa.PublicKey
}

// GenerateDPoPKey generates a new ECDSA key pair for DPoP
func GenerateDPoPKey() (*DPoPKey, error) {
	privateKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, fmt.Errorf("failed to generate ECDSA key: %w", err)
	}

	return &DPoPKey{
		PrivateKey: privateKey,
		PublicKey:  &privateKey.PublicKey,
	}, nil
}

// CreateDPoPProof creates a DPoP proof JWT for a request
func (k *DPoPKey) CreateDPoPProof(htm, htu string, accessToken string, nonce string) (string, error) {
	// Create JWK with properly padded coordinates (32 bytes for P-256)
	xBytes := k.PublicKey.X.Bytes()
	yBytes := k.PublicKey.Y.Bytes()

	// Pad to 32 bytes if needed
	xPadded := make([]byte, 32)
	yPadded := make([]byte, 32)
	copy(xPadded[32-len(xBytes):], xBytes)
	copy(yPadded[32-len(yBytes):], yBytes)

	jwk := map[string]interface{}{
		"kty": "EC",
		"crv": "P-256",
		"x":   base64.RawURLEncoding.EncodeToString(xPadded),
		"y":   base64.RawURLEncoding.EncodeToString(yPadded),
	}

	// Create header
	header := map[string]interface{}{
		"typ": "dpop+jwt",
		"alg": "ES256",
		"jwk": jwk,
	}

	// Create payload
	payload := map[string]interface{}{
		"htm": htm,
		"htu": htu,
		"jti": generateJTI(),
		"iat": time.Now().Unix(),
	}

	// Add server nonce if provided
	if nonce != "" {
		payload["nonce"] = nonce
	}

	// Add access token hash if provided
	if accessToken != "" {
		hash := sha256.Sum256([]byte(accessToken))
		payload["ath"] = base64.RawURLEncoding.EncodeToString(hash[:])
	}

	// Encode header and payload
	headerJSON, err := json.Marshal(header)
	if err != nil {
		return "", fmt.Errorf("failed to marshal header: %w", err)
	}

	payloadJSON, err := json.Marshal(payload)
	if err != nil {
		return "", fmt.Errorf("failed to marshal payload: %w", err)
	}

	headerB64 := base64.RawURLEncoding.EncodeToString(headerJSON)
	payloadB64 := base64.RawURLEncoding.EncodeToString(payloadJSON)

	// Create signing input
	signingInput := headerB64 + "." + payloadB64

	// Sign with ECDSA
	hash := sha256.Sum256([]byte(signingInput))
	r, s, err := ecdsa.Sign(rand.Reader, k.PrivateKey, hash[:])
	if err != nil {
		return "", fmt.Errorf("failed to sign DPoP proof: %w", err)
	}

	// Encode signature in IEEE P1363 format (concatenated r and s, 32 bytes each for P-256)
	signature := make([]byte, 64)
	rBytes := r.Bytes()
	sBytes := s.Bytes()

	// Pad r and s to 32 bytes if needed
	copy(signature[32-len(rBytes):32], rBytes)
	copy(signature[64-len(sBytes):64], sBytes)

	signatureB64 := base64.RawURLEncoding.EncodeToString(signature)

	// Create JWT
	jwt := signingInput + "." + signatureB64

	return jwt, nil
}

// generateJTI generates a unique identifier for the JWT
func generateJTI() string {
	b := make([]byte, 16)
	rand.Read(b)
	return base64.RawURLEncoding.EncodeToString(b)
}

// JWKThumbprint calculates the JWK thumbprint for the public key
func (k *DPoPKey) JWKThumbprint() (string, error) {
	// Pad coordinates to 32 bytes for P-256
	xBytes := k.PublicKey.X.Bytes()
	yBytes := k.PublicKey.Y.Bytes()

	xPadded := make([]byte, 32)
	yPadded := make([]byte, 32)
	copy(xPadded[32-len(xBytes):], xBytes)
	copy(yPadded[32-len(yBytes):], yBytes)

	jwk := map[string]interface{}{
		"crv": "P-256",
		"kty": "EC",
		"x":   base64.RawURLEncoding.EncodeToString(xPadded),
		"y":   base64.RawURLEncoding.EncodeToString(yPadded),
	}

	jwkJSON, err := json.Marshal(jwk)
	if err != nil {
		return "", fmt.Errorf("failed to marshal JWK: %w", err)
	}

	hash := sha256.Sum256(jwkJSON)
	return base64.RawURLEncoding.EncodeToString(hash[:]), nil
}
