# Authentication Implementation Plan

This document provides research findings and implementation guidance for authentication in the autoreply MCP servers (Go and Rust).

## Authentication Methods

BlueSky AT Protocol supports three authentication approaches:

### OAuth 2.0 with DPoP and PKCE (Preferred)

**Best for:** Interactive clients (desktop apps, browser-based tools)

AT Protocol OAuth extends standard OAuth 2.0 with:
- **DPoP (Demonstrating Proof-of-Possession):** Binds tokens to client cryptographic keys, preventing token theft
- **PKCE:** Protects authorization code exchange
- **PAR (Pushed Authorization Requests):** Optional security enhancement

**Flow:** Client generates key pair → authorization request → user approves in browser → token exchange → authenticated requests with DPoP proof

**Advantages:** Most secure, standard protocol, revocable per-application
**Considerations:** Requires browser access, more complex implementation

### Device Authorization Grant

**Best for:** Headless environments (remote servers, CI/CD, agents without browsers)

User visits verification URL on another device and enters code displayed by CLI.

**Flow:** Request device code → display verification URL and code → poll for authorization → receive tokens

**Advantages:** Works without browser on target device, good UX for CLI tools
**Considerations:** Requires user to switch devices

### App-Specific Passwords (Fallback)

**Best for:** Simple use cases, environments where OAuth isn't feasible

Uses `com.atproto.server.createSession` XRPC endpoint with app password (created in BlueSky settings).

**Flow:** Send identifier and app password → receive access token (2h lifetime) and refresh token (90d)

**Advantages:** Simplest implementation, no browser required, works everywhere
**Considerations:** Less secure than OAuth, user manages passwords manually, app passwords are less preferred by the protocol

## Library Ecosystem

### Rust

**AT Protocol OAuth:**
- `atproto-oauth` crate: DPoP, PKCE, JWT/JWK primitives, PAR workflow
- `atproto-client`: HTTP client with DPoP authentication
- `atproto-identity`: DID resolution

**Strengths:** Complete OAuth implementation, well-documented, active maintenance
**Integration:** Can build full OAuth flows with these crates

**Credential Storage:**
- `keyring` crate v2.3+: Cross-platform (macOS Keychain, Windows Credential Manager, Linux Secret Service)

**Strengths:** Mature, simple API, widely used
**Integration:** Store tokens per account handle

**Alternative for Simple Auth:**
- `reqwest` with direct XRPC calls for app password flow

### Go

**AT Protocol OAuth:**
- `bluesky-social/indigo`: Official packages for xrpc, identity, crypto
- `haileyok/atproto-oauth-golang`: Complete OAuth reference (archived but functional)
- `streamplace/atproto-oauth-golang`: Active community fork

**Strengths:** Official support via indigo, complete examples in community code
**Integration:** Use indigo primitives; reference archived implementations for patterns
**Considerations:** May need to adapt community OAuth code or implement from indigo primitives

**Credential Storage:**
- `99designs/keyring` v1.2+: Cross-platform keyring access

**Strengths:** Well-maintained, multiple backend support
**Integration:** Similar API to Rust keyring crate

## Implementation Guidance

### Storage Strategy

**Primary:** OS-native keyring (best security, native platform integration)
**Fallback:** Encrypted file in user directory when keyring unavailable (use platform crypto APIs)
**Last Resort:** Plaintext file with explicit user consent and strict permissions

Store credentials keyed by account handle (e.g., `alice.bsky.social`). Support multiple simultaneous accounts.

### Token Management

- Check token expiry before API calls
- Refresh automatically using refresh token
- Handle refresh failures gracefully (prompt re-authentication)
- Don't log tokens or expose in environment

### MCP Integration

**Option 1 - MCP Tool (recommended start):**
Add `login` tool to MCP protocol for app password authentication. Simple integration, works in all MCP environments.

**Option 2 - Separate Service (for OAuth):**
Run local HTTP server for OAuth callbacks. Better for browser-based flows.

**Recommendation:** Start with Option 1 for app passwords, evaluate Option 2 when adding OAuth.

### CLI Commands

Provide standalone commands independent of MCP:
- `autoreply login [--handle <handle>]` - Authenticate (prompt for method)
- `autoreply logout [--handle <handle>]` - Remove credentials
- `autoreply accounts list` - Show authenticated accounts
- `autoreply accounts default <handle>` - Set default account

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

## Implementation Sequence

The implementation can proceed flexibly based on needs:

1. **App password authentication** provides quickest path to working auth
2. **OAuth PKCE** adds best security for interactive use
3. **Device flow** enables headless environments

Each can be implemented independently. Start with the flow that best serves the primary use case.

## Open Questions

- Should OAuth use dynamic client registration or require pre-configured client credentials?
- How should token refresh work in long-running server vs short-lived CLI?
- Should MCP tool handle OAuth flows, or delegate to separate service?

These decisions can be made during implementation based on practical constraints and use case requirements.

## References

- [AT Protocol OAuth Spec](https://atproto.com/specs/oauth)
- [RFC 7636 - PKCE](https://datatracker.ietf.org/doc/html/rfc7636)
- [RFC 8628 - Device Authorization Grant](https://datatracker.ietf.org/doc/html/rfc8628)
- [RFC 9449 - DPoP](https://datatracker.ietf.org/doc/html/rfc9449)
- [BlueSky OAuth Examples](https://github.com/bluesky-social/cookbook)
