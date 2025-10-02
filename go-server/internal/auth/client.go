// Package auth provides authentication and credential management
package auth

// ClientMetadata represents OAuth client metadata per AT Protocol spec
// This should be hosted at a public HTTPS URL which becomes the client_id
type ClientMetadata struct {
	ClientID                      string   `json:"client_id"`
	ApplicationType               string   `json:"application_type,omitempty"` // "web" or "native"
	ClientName                    string   `json:"client_name,omitempty"`
	ClientURI                     string   `json:"client_uri,omitempty"`
	LogoURI                       string   `json:"logo_uri,omitempty"`
	TOSURI                        string   `json:"tos_uri,omitempty"`
	PolicyURI                     string   `json:"policy_uri,omitempty"`
	GrantTypes                    []string `json:"grant_types"`
	ResponseTypes                 []string `json:"response_types"`
	Scope                         string   `json:"scope"`
	RedirectURIs                  []string `json:"redirect_uris"`
	DPoPBoundAccessTokens         bool     `json:"dpop_bound_access_tokens"`
	TokenEndpointAuthMethod       string   `json:"token_endpoint_auth_method"`
	TokenEndpointAuthSigningAlg   string   `json:"token_endpoint_auth_signing_alg,omitempty"`
}

// GetDefaultClientMetadata returns default client metadata for CLI use
// Note: In production, this should be hosted at a public URL
func GetDefaultClientMetadata(redirectURI string) *ClientMetadata {
	return &ClientMetadata{
		// For now, use a placeholder. In production, this should be:
		// - A real HTTPS URL where the metadata JSON is hosted
		// - The same URL as client_id
		ClientID:                "http://localhost/client-metadata.json",
		ApplicationType:         "native",
		ClientName:              "Autoreply CLI",
		GrantTypes:              []string{"authorization_code", "refresh_token"},
		ResponseTypes:           []string{"code"},
		Scope:                   "atproto transition:generic",
		RedirectURIs:            []string{redirectURI},
		DPoPBoundAccessTokens:   true,
		TokenEndpointAuthMethod: "none", // Public client
	}
}
