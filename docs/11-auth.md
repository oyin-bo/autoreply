# Authentication Implementation Plan

This document provides research findings and implementation guidance for authentication in the autoreply MCP servers (Go and Rust).

## Implementation Summary

Both the Go and Rust servers provide a `login` command that supports both OAuth 2.0 and app password authentication. Both implementations use the OS keychain for secure storage and support managing multiple accounts.

The command-line usage is identical for both servers:
- **OAuth**: `autoreply login -u <handle>`
- **App Password**: `autoreply login -u <handle> -p` (prompts for password if not provided)

### Notes on Implementation

- **Token Refresh**: Both implementations store the refresh token, but automatic token refresh is not yet implemented. The tools rely on the initial token's lifetime.

## Authentication Methods

BlueSky AT Protocol supports three authentication approaches:

### OAuth 2.0 with DPoP and PKCE

OAuth 2.0 as used by the AT Protocol includes DPoP and PKCE. PAR may be used where supported.

Flow: client generates a key pair → authorization request → user authorizes in browser → code exchange → use DPoP-bound access tokens for requests.

Note: OAuth flows require browser interaction for the authorization step.

### App-Specific Passwords (app-password)

App-password login uses the `com.atproto.server.createSession` XRPC endpoint with an app-specific password created in BlueSky settings.

Flow: send identifier and app password → receive access token (example lifetime ~2h) and refresh token (longer lifetime).

Note: App-passwords do not require browser interaction.

### Library Ecosystem

### Rust

Libraries observed in `rust-server/Cargo.toml` (selected, authentication-related):

- `reqwest` — HTTP client used for network requests
- `keyring` — OS-native credential storage
- `atproto-oauth` — AT Protocol OAuth primitives
- `atproto-client` — AT Protocol client utilities
- `atproto-identity` — DID/identity resolution
- `p256`, `sha2`, `jsonwebtoken` — cryptography and JWT handling
- `webbrowser`, `axum`, `tower` — used by OAuth/browser flow helpers

### Go

Libraries observed in `go-server/go.mod` (selected, authentication-related):

- `github.com/bluesky-social/indigo` — AT Protocol primitives and xrpc utilities
- `github.com/99designs/keyring` / `github.com/99designs/go-keychain` — OS-native credential storage
- `github.com/danieljoos/wincred`, `github.com/go-libsecret` (indirect) — platform backends used by keyring

Only libraries actually present in the repository manifests are listed above.

## Implementation Guidance

### Storage Strategy

Storage approaches present in the codebase and used by the implementations:
- OS-native keyring / system credential store
- Encrypted file in user directory as a fallback when keyring is unavailable
- Plaintext file only with explicit user consent and strict permissions

Credentials are stored keyed by account handle (e.g., `alice.bsky.social`). Multiple accounts are supported.

### Token Management (current state)

- Both implementations store access and refresh tokens in the credential store.
- Automatic token refresh is not implemented in the runtime code; refresh tokens are stored and available for use.
- Tokens are not logged by the implementations.

### MCP Integration

Integration approaches present in the repository:
- MCP tool: the `login` tool can be invoked via MCP and supports app-password flows and interactive flows.
- Local callback server: a local HTTP server can be used to receive OAuth callbacks during browser-based flows.

Both approaches are implemented or referenced in the codebase.

...existing code...

### CLI Commands

The unified `login` command handles both OAuth and app password authentication:

```bash
# OAuth authentication (default - opens browser)
autoreply login -u alice.bsky.social

# App password authentication (use -p flag)
autoreply login -u alice.bsky.social -p          # prompts for password
autoreply login -u alice.bsky.social -p mypass   # inline password

# Interactive mode (prompts for method choice)
autoreply login

# Other account management
autoreply logout [--handle <handle>]             # Remove credentials
autoreply accounts list                          # Show authenticated accounts  
autoreply accounts --action set-default -u <handle>  # Set default account
```

### MCP Tool Usage

When used as an MCP tool, the `login` tool accepts:

- `handle` (required): Bluesky handle (e.g., alice.bsky.social)
- `password` (optional): If present (even empty string), forces app password mode and prompts for password if empty
- `port` (optional): Local callback server port for OAuth (default: 8080)

**OAuth mode**: Omit the `password` parameter entirely
```json
{"handle": "alice.bsky.social"}
```

**App password mode**: Include the `password` parameter
```json
{"handle": "alice.bsky.social", "password": "your-app-password"}
```

### Multi-Account Support

- Store separate credentials for each handle
- Support default account for commands without explicit handle
- Allow switching between accounts
- Provide account listing and management

## Security Considerations

- Use TLS for all authentication requests
- Never log tokens, passwords, or sensitive data
- Set restrictive file permissions (0600) for any file storage
- Zero sensitive memory when no longer needed (Rust: consider `zeroize` crate)
- Validate certificates properly
- Implement reasonable timeouts

## Testing Approach

- Mock OAuth/XRPC servers for integration tests
- Test credential storage/retrieval across platforms
- Verify token refresh logic
- Test multi-account scenarios
- Security: confirm no token leaks in logs/errors


## References

- [AT Protocol OAuth Spec](https://atproto.com/specs/oauth)
- [RFC 7636 - PKCE](https://datatracker.ietf.org/doc/html/rfc7636)
- [RFC 9449 - DPoP](https://datatracker.ietf.org/doc/html/rfc9449)
- [BlueSky OAuth Examples](https://github.com/bluesky-social/cookbook)
- [Email Transitional Scope : BlueSky blog 2025/06/12](https://github.com/bluesky-social/bsky-docs/blob/main/blog/2025-06-12-oauth-improvements.md)
- [Release of the initial specification of OAuth for AT Protocol : BlueSky blog 2024/09/25](https://github.com/bluesky-social/bsky-docs/blob/main/blog/2024-09-25-oauth-atproto.md)
- [OAuth Client Implementation : BlueSky guide](https://github.com/bluesky-social/bsky-docs/blob/main/docs/advanced-guides/oauth-client.md)