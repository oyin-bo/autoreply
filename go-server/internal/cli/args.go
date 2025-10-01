// Package cli provides command-line interface support for trial mode
package cli

// ProfileArgs defines arguments for the profile tool
type ProfileArgs struct {
	Account string `json:"account" jsonschema:"required,description=Handle (alice.bsky.social) or DID (did:plc:...)" short:"a" long:"account"`
}

// SearchArgs defines arguments for the search tool
type SearchArgs struct {
	Account string `json:"account" jsonschema:"required,description=Handle (alice.bsky.social) or DID (did:plc:...)" short:"a" long:"account"`
	Query   string `json:"query" jsonschema:"required,description=Search terms (case-insensitive)" short:"q" long:"query"`
	Limit   int    `json:"limit,omitempty" jsonschema:"description=Maximum number of results (default 50 max 200)" short:"l" long:"limit"`
}

// LoginArgs defines arguments for the login tool
type LoginArgs struct {
	Handle   string `json:"handle,omitempty" jsonschema:"description=Bluesky handle (e.g. alice.bsky.social) (optional - will prompt if not provided)" short:"u" long:"handle"`
	Password string `json:"password,omitempty" jsonschema:"description=App password (generated in Bluesky settings) (optional - will prompt if not provided)" short:"p" long:"password"`
}

// LogoutArgs defines arguments for the logout tool
type LogoutArgs struct {
	Handle string `json:"handle,omitempty" jsonschema:"description=Bluesky handle to logout (uses default if not provided)" short:"u" long:"handle"`
}

// AccountsArgs defines arguments for the accounts tool
type AccountsArgs struct {
	Action string `json:"action,omitempty" jsonschema:"description=Action to perform: 'list' or 'set-default' (default: list)" short:"a" long:"action"`
	Handle string `json:"handle,omitempty" jsonschema:"description=Handle for set-default action" short:"u" long:"handle"`
}

// OAuthLoginArgs defines arguments for the OAuth login tool
type OAuthLoginArgs struct {
	ClientID string `json:"client_id,omitempty" jsonschema:"description=OAuth client ID (optional uses default if not provided)" short:"c" long:"client-id"`
	Port     int    `json:"port,omitempty" jsonschema:"description=Local callback server port (default: 8080)" short:"p" long:"port"`
}

// DeviceLoginArgs defines arguments for the device login tool
type DeviceLoginArgs struct {
	ClientID string `json:"client_id,omitempty" jsonschema:"description=OAuth client ID (optional uses default if not provided)" short:"c" long:"client-id"`
}
