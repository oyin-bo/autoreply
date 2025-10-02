// Package auth provides authentication and credential management
package auth

import (
	"strings"
	"testing"
)

func TestGeneratePKCEChallenge(t *testing.T) {
	verifier, challenge, err := GeneratePKCEChallenge()
	if err != nil {
		t.Fatalf("Failed to generate PKCE challenge: %v", err)
	}

	if verifier == "" {
		t.Error("Expected non-empty verifier")
	}

	if challenge == "" {
		t.Error("Expected non-empty challenge")
	}

	// Verify verifier is base64url encoded
	if strings.Contains(verifier, "+") || strings.Contains(verifier, "/") || strings.Contains(verifier, "=") {
		t.Error("Verifier should be base64url encoded without padding")
	}

	// Verify challenge is base64url encoded
	if strings.Contains(challenge, "+") || strings.Contains(challenge, "/") || strings.Contains(challenge, "=") {
		t.Error("Challenge should be base64url encoded without padding")
	}

	// Verify verifier and challenge are different
	if verifier == challenge {
		t.Error("Verifier and challenge should be different")
	}
}

func TestGenerateState(t *testing.T) {
	state, err := GenerateState()
	if err != nil {
		t.Fatalf("Failed to generate state: %v", err)
	}

	if state == "" {
		t.Error("Expected non-empty state")
	}

	// Verify state is base64url encoded
	if strings.Contains(state, "+") || strings.Contains(state, "/") || strings.Contains(state, "=") {
		t.Error("State should be base64url encoded without padding")
	}

	// Generate another state and verify they're different
	state2, err := GenerateState()
	if err != nil {
		t.Fatalf("Failed to generate second state: %v", err)
	}

	if state == state2 {
		t.Error("Generated states should be different")
	}
}

func TestPKCEUniqueness(t *testing.T) {
	// Generate multiple PKCE challenges and verify they're all unique
	seen := make(map[string]bool)

	for i := 0; i < 10; i++ {
		verifier, challenge, err := GeneratePKCEChallenge()
		if err != nil {
			t.Fatalf("Failed to generate PKCE challenge: %v", err)
		}

		if seen[verifier] {
			t.Error("Generated duplicate verifier")
		}
		seen[verifier] = true

		if seen[challenge] {
			t.Error("Generated duplicate challenge")
		}
		seen[challenge] = true
	}
}
