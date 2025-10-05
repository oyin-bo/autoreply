// Package auth provides authentication and credential management
package auth

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"fmt"
)

// GeneratePKCEChallenge generates a PKCE code verifier and challenge
// as per RFC 7636
func GeneratePKCEChallenge() (verifier, challenge string, err error) {
	// Generate 32 bytes of random data for the verifier
	verifierBytes := make([]byte, 32)
	if _, err := rand.Read(verifierBytes); err != nil {
		return "", "", fmt.Errorf("failed to generate random bytes: %w", err)
	}

	// Base64 URL encode without padding
	verifier = base64.RawURLEncoding.EncodeToString(verifierBytes)

	// Create SHA256 hash of the verifier
	hash := sha256.Sum256([]byte(verifier))

	// Base64 URL encode the hash without padding
	challenge = base64.RawURLEncoding.EncodeToString(hash[:])

	return verifier, challenge, nil
}
