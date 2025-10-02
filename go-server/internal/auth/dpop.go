package auth

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"crypto/x509"
	"encoding/base64"
	"encoding/pem"
	"fmt"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

// DPoPKeyPair represents an ES256 key pair for DPoP
type DPoPKeyPair struct {
	PrivateKey     *ecdsa.PrivateKey
	PublicKey      *ecdsa.PublicKey
	JWKThumbprint  string
}

// GenerateDPoPKeyPair generates a new ES256 key pair for DPoP
func GenerateDPoPKeyPair() (*DPoPKeyPair, error) {
	privateKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, fmt.Errorf("failed to generate private key: %w", err)
	}

	publicKey := &privateKey.PublicKey

	// Calculate JWK thumbprint
	thumbprint, err := calculateJWKThumbprint(publicKey)
	if err != nil {
		return nil, fmt.Errorf("failed to calculate JWK thumbprint: %w", err)
	}

	return &DPoPKeyPair{
		PrivateKey:    privateKey,
		PublicKey:     publicKey,
		JWKThumbprint: thumbprint,
	}, nil
}

// FromPEM loads a DPoP key pair from PEM-encoded private key
func DPoPKeyPairFromPEM(pemData string) (*DPoPKeyPair, error) {
	block, _ := pem.Decode([]byte(pemData))
	if block == nil {
		return nil, fmt.Errorf("failed to decode PEM block")
	}

	privateKey, err := x509.ParseECPrivateKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("failed to parse EC private key: %w", err)
	}

	publicKey := &privateKey.PublicKey

	// Calculate JWK thumbprint
	thumbprint, err := calculateJWKThumbprint(publicKey)
	if err != nil {
		return nil, fmt.Errorf("failed to calculate JWK thumbprint: %w", err)
	}

	return &DPoPKeyPair{
		PrivateKey:    privateKey,
		PublicKey:     publicKey,
		JWKThumbprint: thumbprint,
	}, nil
}

// ToPEM exports the private key as PEM
func (kp *DPoPKeyPair) ToPEM() (string, error) {
	keyBytes, err := x509.MarshalECPrivateKey(kp.PrivateKey)
	if err != nil {
		return "", fmt.Errorf("failed to marshal private key: %w", err)
	}

	block := &pem.Block{
		Type:  "EC PRIVATE KEY",
		Bytes: keyBytes,
	}

	return string(pem.EncodeToMemory(block)), nil
}

// PublicJWK returns the public key as a JWK
func (kp *DPoPKeyPair) PublicJWK() (map[string]interface{}, error) {
	x := base64.RawURLEncoding.EncodeToString(kp.PublicKey.X.Bytes())
	y := base64.RawURLEncoding.EncodeToString(kp.PublicKey.Y.Bytes())

	return map[string]interface{}{
		"kty": "EC",
		"crv": "P-256",
		"x":   x,
		"y":   y,
		"use": "sig",
		"alg": "ES256",
	}, nil
}

// calculateJWKThumbprint calculates the JWK thumbprint (SHA-256 of canonical JWK)
func calculateJWKThumbprint(publicKey *ecdsa.PublicKey) (string, error) {
	x := base64.RawURLEncoding.EncodeToString(publicKey.X.Bytes())
	y := base64.RawURLEncoding.EncodeToString(publicKey.Y.Bytes())

	// Canonical JWK (RFC 7638) - fields in lexicographic order
	canonical := fmt.Sprintf(`{"crv":"P-256","kty":"EC","x":"%s","y":"%s"}`, x, y)

	hash := sha256.Sum256([]byte(canonical))
	return base64.RawURLEncoding.EncodeToString(hash[:]), nil
}

// CreateDPoPProof creates a DPoP proof JWT
func (kp *DPoPKeyPair) CreateDPoPProof(htm, htu string, nonce, ath *string) (string, error) {
	now := time.Now().Unix()
	jti := uuid.New().String()

	// Create claims
	claims := jwt.MapClaims{
		"jti": jti,
		"htm": htm,
		"htu": htu,
		"iat": now,
	}

	if nonce != nil {
		claims["nonce"] = *nonce
	}

	if ath != nil {
		claims["ath"] = *ath
	}

	// Get public JWK
	jwk, err := kp.PublicJWK()
	if err != nil {
		return "", fmt.Errorf("failed to get public JWK: %w", err)
	}
	claims["jwk"] = jwk

	// Create token with custom header
	token := jwt.NewWithClaims(jwt.SigningMethodES256, claims)
	token.Header["typ"] = "dpop+jwt"

	// Sign the token
	tokenString, err := token.SignedString(kp.PrivateKey)
	if err != nil {
		return "", fmt.Errorf("failed to sign DPoP JWT: %w", err)
	}

	return tokenString, nil
}

// CalculateAccessTokenHash calculates the access token hash for DPoP ath claim
func CalculateAccessTokenHash(accessToken string) string {
	hash := sha256.Sum256([]byte(accessToken))
	return base64.RawURLEncoding.EncodeToString(hash[:])
}
