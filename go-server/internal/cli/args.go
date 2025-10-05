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

// LoginArgs defines arguments for the unified login tool with subcommands (matching Rust LoginCommand)
type LoginArgs struct {
	Command  string `json:"command,omitempty" jsonschema:"description=Subcommand: 'list' 'default' 'delete' or omit for login (default: login)" long:"command"`
	Handle   string `json:"handle,omitempty" jsonschema:"description=Bluesky handle (e.g. alice.bsky.social) (optional - will prompt if not provided)" short:"u" long:"handle"`
	Password string `json:"password,omitempty" jsonschema:"description=App password (if flag present, uses app password mode; will prompt if empty)" short:"p" long:"password"`
	Port     int    `json:"port,omitempty" jsonschema:"description=Local callback server port for OAuth (default: 8080)" long:"port"`
	Service  string `json:"service,omitempty" jsonschema:"description=Service URL (defaults to https://bsky.social)" short:"s" long:"service"`
}
