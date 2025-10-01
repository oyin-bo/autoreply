# Authentication Implementation Plan for BlueSky MCP Servers

This document provides a practical, concrete plan for implementing authentication in the autoreply MCP servers (Go and Rust), based on the requirements in [11-login.md](./11-login.md) and research into BlueSky's AT Protocol authentication, available libraries, and credential storage solutions.

## Executive Summary

**Recommended Approach:**
- **Primary Authentication:** AT Protocol OAuth with DPoP (Demonstrating Proof-of-Possession) and PKCE
- **Fallback Authentication:** App-specific password (username/password) for environments where OAuth is not feasible
- **Credential Storage:** OS-native keyring (primary), encrypted file (fallback), plaintext with consent (last resort)
- **Implementation Strategy:** Phased approach starting with app password support, followed by OAuth flows

## BlueSky AT Protocol Authentication Overview

### Authentication Methods Supported by BlueSky

1. **OAuth 2.0 with DPoP and PKCE** (Modern, Preferred)
   - Uses AT Protocol's OAuth implementation with Demonstrating Proof-of-Possession (DPoP)
   - Includes PKCE (Proof Key for Code Exchange) for public clients
   - Supports PAR (Pushed Authorization Requests) for enhanced security
   - Requires dynamic client registration or pre-registered client credentials
   - Best for: Interactive desktop clients, web applications

2. **Device Authorization Grant** (OAuth 2.0 Device Flow)
   - Designed for input-constrained devices and headless environments
   - User authenticates on a separate device (browser) using a code
   - Best for: CLIs on remote servers, CI/CD environments, agents

3. **App-Specific Passwords**
   - User creates app-specific password in BlueSky settings
   - Authenticated via `com.atproto.server.createSession` XRPC endpoint
   - Returns access token and refresh token
   - Best for: Initial implementation, fallback method, simple use cases

### AT Protocol OAuth Flow Specifics

**Key Components:**
- **DPoP (Demonstrating Proof-of-Possession):** Binds access tokens to a specific client key, preventing token theft
- **PKCE:** Code challenge/verifier prevents authorization code interception
- **PAR (Pushed Authorization Requests):** Pre-registers authorization requests for security
- **Client Assertion:** For confidential clients using private_key_jwt

**Token Types:**
- **Access Token:** Short-lived (typically 2 hours), used for API requests
- **Refresh Token:** Long-lived (typically 90 days), used to obtain new access tokens
- **DPoP Token:** JWT proving possession of the private key

## Library Recommendations

### Rust Libraries

#### OAuth and AT Protocol
- **Primary:** `atproto-oauth` (v0.1.x on crates.io)
  - Modules: `dpop`, `pkce`, `jwk`, `jwt`, `workflow`, `storage`, `resources`
  - Comprehensive AT Protocol OAuth implementation
  - Well-documented on docs.rs
  
- **HTTP Client:** `atproto-client` (v0.1.x on crates.io)
  - Built-in DPoP authentication support
  - CLI helpers via `atproto-client-auth` and `atproto-client-dpop`
  - Integrates with `atproto-oauth`

- **Identity:** `atproto-identity` (v0.1.x on crates.io)
  - DID resolution and validation
  - Key utilities for OAuth flow

- **Alternative for Simple Auth:** `reqwest` with manual XRPC calls
  - For app password flow without OAuth complexity
  - Already used in current implementation

#### Credential Storage
- **Primary:** `keyring` (v2.3+)
  - Cross-platform: macOS Keychain, Windows Credential Manager, Linux Secret Service
  - Simple API: `Entry::new(service, username)`, `set_password()`, `get_password()`
  - Well-maintained, 1M+ downloads/month
  - Example:
    ```rust
    use keyring::Entry;
    
    let entry = Entry::new("autoreply-mcp", "alice.bsky.social")?;
    entry.set_password("app-password-or-tokens")?;
    let password = entry.get_password()?;
    ```

- **Fallback:** `rpassword` + `aes-gcm` for encrypted file storage
  - Use for environments without OS keyring support
  - Encrypt tokens with user-derived key (argon2 for key derivation)
  - Store in `~/.config/autoreply/credentials.enc`

### Go Libraries

#### OAuth and AT Protocol
- **Primary for Native OAuth:** `bluesky-social/indigo` + community OAuth helpers
  - `indigo/xrpc`: XRPC client implementation
  - `indigo/identity`: DID resolution
  - `indigo/crypto`: Cryptographic primitives for AT Protocol
  - Combine with archived but functional `haileyok/atproto-oauth-golang` as reference
  
- **Alternative:** Fork/vendor `haileyok/atproto-oauth-golang` 
  - Complete implementation of PAR, PKCE, DPoP, client assertion
  - Archived but production-ready code
  - Actively maintained fork: `streamplace/atproto-oauth-golang`

- **Simple Auth:** Standard library `net/http` + custom XRPC client
  - For app password flow
  - Endpoint: `https://bsky.social/xrpc/com.atproto.server.createSession`

#### Credential Storage
- **Primary:** `99designs/keyring` (v1.2+)
  - Cross-platform keyring access (macOS, Windows, Linux)
  - Multiple backends: Keychain, WinCred, SecretService, pass, kwallet
  - Example:
    ```go
    import "github.com/99designs/keyring"
    
    ring, _ := keyring.Open(keyring.Config{
        ServiceName: "autoreply-mcp",
    })
    
    ring.Set(keyring.Item{
        Key:  "alice.bsky.social",
        Data: []byte("app-password-or-tokens"),
    })
    
    item, _ := ring.Get("alice.bsky.social")
    ```

- **Fallback:** `crypto/aes` (stdlib) + `golang.org/x/crypto/argon2` for encrypted files
  - Built-in encryption capabilities
  - Store in `~/.config/autoreply/credentials.enc`

## Implementation Architecture

### Credential Storage Schema

Store credentials as JSON with the following structure:

```json
{
  "version": "1.0",
  "accounts": {
    "alice.bsky.social": {
      "provider": "bsky.social",
      "did": "did:plc:abc123...",
      "auth_method": "app_password",
      "access_token": "...",
      "refresh_token": "...",
      "token_expires_at": "2024-01-15T10:30:00Z",
      "dpop_key": null,
      "created_at": "2024-01-01T00:00:00Z",
      "last_used": "2024-01-10T12:00:00Z"
    },
    "bob.example.com": {
      "provider": "example.com",
      "did": "did:plc:def456...",
      "auth_method": "oauth_pkce",
      "access_token": "...",
      "refresh_token": "...",
      "token_expires_at": "2024-01-15T11:00:00Z",
      "dpop_key": {
        "kid": "...",
        "private_key": "..."
      },
      "created_at": "2024-01-02T00:00:00Z",
      "last_used": "2024-01-10T12:30:00Z"
    }
  }
}
```

### Authentication Flow Sequence

#### Phase 1: App Password Authentication

**CLI Login Flow:**
```
User: autoreply login --handle alice.bsky.social
CLI: Password (app password from BlueSky settings): ***
CLI: → POST /xrpc/com.atproto.server.createSession
     { identifier: "alice.bsky.social", password: "***" }
Server: ← { accessJwt: "...", refreshJwt: "...", did: "did:plc:..." }
CLI: Store tokens in keyring/secure storage
CLI: ✓ Logged in as @alice.bsky.social (did:plc:...)
```

**MCP Server Login Flow (via MCP Tool):**
```
Client: tools/call login { handle: "alice.bsky.social", password: "***" }
Server: → Authenticate via createSession
Server: → Store tokens in keyring
Server: ← Return success with handle and DID
```

#### Phase 2: OAuth PKCE Flow (Interactive)

**CLI OAuth Flow:**
```
User: autoreply login --handle alice.bsky.social --oauth
CLI: → Resolve DID and discover authorization server
CLI: → Generate PKCE code verifier and challenge
CLI: → (Optional) Send PAR request
CLI: → Open browser: https://bsky.social/oauth/authorize?...
CLI: Waiting for authorization... (listening on localhost:8080)
Browser: User logs in and approves
Browser: → Redirect to http://localhost:8080/callback?code=...
CLI: ← Receive authorization code
CLI: → POST /oauth/token with code + code_verifier
Server: ← { access_token: "...", refresh_token: "...", ... }
CLI: Store tokens in keyring
CLI: ✓ Logged in as @alice.bsky.social via OAuth
```

#### Phase 3: Device Authorization Flow (Headless)

**CLI Device Flow:**
```
User: autoreply login --handle alice.bsky.social --device
CLI: → POST /oauth/device/authorize
Server: ← { device_code: "...", user_code: "ABC-123", 
           verification_uri: "https://bsky.social/device" }
CLI: ═══════════════════════════════════════════
     Visit: https://bsky.social/device
     Enter code: ABC-123
     ═══════════════════════════════════════════
CLI: → Poll /oauth/token (every 5 seconds)
User: (Opens browser, enters code, approves)
Server: ← { access_token: "...", refresh_token: "..." }
CLI: ✓ Logged in as @alice.bsky.social
```

### Token Refresh Strategy

**Automatic Refresh:**
```
Client: tools/call profile { account: "alice.bsky.social" }
Server: → Load credentials for alice.bsky.social
Server: → Check if access_token expired
Server: → If expired: POST /xrpc/com.atproto.server.refreshSession
         with refresh_token
Server: ← Receive new access_token and refresh_token
Server: → Update stored credentials
Server: → Proceed with profile request
```

**Refresh Failure Handling:**
```
Server: → Attempt token refresh
Server: ← 400 Bad Request (invalid refresh token)
Server: → Mark credentials as invalid
Server: → Return error to client: "Re-authentication required"
Client: Display: "Session expired for alice.bsky.social. 
                  Run: autoreply login --handle alice.bsky.social"
```

### Multi-Account Management

**Storage Strategy:**
- Keyring: One entry per account handle (key = handle)
- In-memory cache: Map of handle → credentials (during server lifetime)
- Default account: Store in keyring with key "default"

**Account Selection:**
```rust
// Rust example
struct AuthManager {
    cache: HashMap<String, Credentials>,
    keyring_service: String,
    default_handle: Option<String>,
}

impl AuthManager {
    async fn get_credentials(&mut self, handle: Option<String>) 
        -> Result<&Credentials> {
        let handle = handle.or(self.default_handle.clone())
            .ok_or("No handle specified and no default set")?;
        
        if !self.cache.contains_key(&handle) {
            let creds = self.load_from_keyring(&handle).await?;
            self.cache.insert(handle.clone(), creds);
        }
        
        let creds = self.cache.get_mut(&handle).unwrap();
        if creds.is_expired() {
            self.refresh_token(creds).await?;
        }
        
        Ok(creds)
    }
}
```

### MCP Server API Design

**Option A: Login as MCP Tool (Recommended for Phase 1)**

Advantages:
- Simple integration with existing MCP protocol
- Works in all MCP client environments
- No protocol extensions needed

Disadvantages:
- Credentials passed through MCP protocol (requires encryption)
- Not ideal for OAuth flows requiring browser redirect

**Tool Definition:**
```json
{
  "name": "login",
  "description": "Authenticate with BlueSky using app password",
  "inputSchema": {
    "type": "object",
    "properties": {
      "handle": {
        "type": "string",
        "description": "BlueSky handle (e.g., alice.bsky.social)"
      },
      "password": {
        "type": "string",
        "description": "App-specific password from BlueSky settings"
      }
    },
    "required": ["handle", "password"]
  }
}
```

**Option B: Separate Authentication Service (OAuth-friendly)**

Advantages:
- Proper OAuth flow support with browser redirects
- Keeps sensitive credentials out of MCP protocol
- More secure for production use

Disadvantages:
- Requires additional service/port
- More complex deployment

**Recommended Approach:**
- Phase 1: Implement Option A (MCP tool) for app passwords
- Phase 2: Add Option B for OAuth flows, keep Option A as fallback

### CLI Commands

```bash
# Basic login (app password)
autoreply login --handle alice.bsky.social
# Prompts for password

# OAuth login (opens browser)
autoreply login --handle alice.bsky.social --oauth

# Device flow (for headless environments)
autoreply login --handle alice.bsky.social --device

# List accounts
autoreply accounts list
# Output:
# Accounts:
# * alice.bsky.social (did:plc:abc123...) [default]
#   bob.example.com (did:plc:def456...)

# Set default account
autoreply accounts set-default bob.example.com

# Logout
autoreply logout --handle alice.bsky.social

# Logout all
autoreply logout --all

# Test authentication
autoreply auth test --handle alice.bsky.social
# Output: ✓ alice.bsky.social: Token valid, expires in 1h 23m
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)

**Goals:** Basic app password authentication with secure storage

**Rust Implementation:**
1. Add dependencies to `Cargo.toml`:
   ```toml
   keyring = "2.3"
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   chrono = "0.4"
   ```

2. Create `src/auth/mod.rs`:
   - `Credentials` struct
   - `AuthManager` for credential management
   - `KeyringStorage` trait with OS keyring and file fallback

3. Create `src/auth/app_password.rs`:
   - `authenticate()` function calling `createSession`
   - Token refresh logic
   - Error handling

4. Add CLI commands in `src/cli.rs`:
   - `login` subcommand
   - `logout` subcommand
   - `accounts` subcommand group

5. Add MCP tool in `src/tools/login.rs`:
   - `login` tool implementation
   - Integration with `AuthManager`

**Go Implementation:**
1. Add dependencies to `go.mod`:
   ```
   go get github.com/99designs/keyring
   ```

2. Create `internal/auth/`:
   - `credentials.go`: Credential types and JSON marshaling
   - `manager.go`: AuthManager struct and methods
   - `storage.go`: Keyring and file storage implementation

3. Create `internal/auth/app_password.go`:
   - `Authenticate()` function
   - Token refresh logic

4. Add CLI commands in `cmd/bluesky-mcp/`:
   - `login.go`: Login command implementation
   - `accounts.go`: Account management commands

5. Add MCP tool in `internal/tools/login.go`:
   - Login tool for MCP protocol

**Testing:**
- Unit tests for credential storage/retrieval
- Integration tests for createSession flow
- CLI smoke tests
- MCP protocol tests for login tool

**Deliverables:**
- Working app password authentication
- Secure credential storage (keyring + fallback)
- CLI login/logout commands
- MCP login tool
- Multi-account support
- Automatic token refresh

### Phase 2: OAuth PKCE (Week 3-4)

**Goals:** Full OAuth implementation with browser-based authentication

**Rust Implementation:**
1. Add dependencies:
   ```toml
   atproto-oauth = "0.1"
   atproto-client = "0.1"
   atproto-identity = "0.1"
   tokio = { version = "1", features = ["full"] }
   ```

2. Create `src/auth/oauth.rs`:
   - OAuth client registration/discovery
   - PKCE code challenge/verifier generation
   - Authorization URL construction
   - Token exchange implementation
   - DPoP key generation and JWT signing

3. Create `src/auth/callback_server.rs`:
   - Tiny HTTP server for OAuth callback (port 8080)
   - Receives authorization code
   - Returns success page to browser

4. Update CLI:
   - Add `--oauth` flag to login command
   - Handle browser launch and callback

**Go Implementation:**
1. Add dependencies:
   ```
   go get github.com/bluesky-social/indigo
   ```

2. Create `internal/auth/oauth.go`:
   - OAuth flow implementation using indigo primitives
   - Reference archived `haileyok/atproto-oauth-golang` for patterns
   - PKCE, DPoP, PAR implementation

3. Create `internal/auth/callback_server.go`:
   - HTTP server for OAuth callback

4. Update CLI with OAuth support

**Testing:**
- OAuth flow integration tests (mock server)
- PKCE challenge/verifier validation
- DPoP JWT generation tests
- Browser launch and callback tests

**Deliverables:**
- Full OAuth PKCE flow
- Browser-based authentication
- DPoP token binding
- Enhanced security over app passwords

### Phase 3: Device Flow & Production Hardening (Week 5-6)

**Goals:** Device authorization flow, error recovery, monitoring

**Implementation:**
1. Device authorization grant flow
2. Comprehensive error handling:
   - Network failures with retry
   - Invalid credentials
   - Expired tokens
   - Rate limiting
3. Logging and debugging:
   - Structured logging (don't log secrets!)
   - Debug mode for troubleshooting
4. Configuration:
   - Config file support
   - Environment variables
   - CLI flags precedence
5. Documentation:
   - User guide for authentication
   - Troubleshooting guide
   - Security best practices

**Testing:**
- Device flow simulation
- Error scenario coverage
- Security audit
- Performance testing

**Deliverables:**
- Complete authentication system
- Production-ready error handling
- Comprehensive documentation
- Security review complete

## Security Considerations

### Token Storage
- **Never log tokens** - Implement sanitization in logging
- **Minimal exposure** - Load tokens only when needed
- **Secure permissions** - File fallback must be mode 0600 (user-only)
- **Encryption at rest** - For file storage, use AES-256-GCM with key derivation

### Network Security
- **TLS everywhere** - All authentication requests over HTTPS
- **Certificate validation** - No certificate pinning needed, but validate properly
- **Timeout configuration** - Reasonable timeouts to prevent hanging

### OAuth Security
- **PKCE required** - Even for confidential clients (defense in depth)
- **State parameter** - Prevent CSRF in OAuth flow
- **DPoP binding** - Prevents token theft/replay
- **Short-lived tokens** - Access tokens expire quickly (2 hours typical)

### Secrets Management
- **App passwords** - Recommend users create app-specific passwords
- **Never commit secrets** - .gitignore credential files
- **Memory safety** - Zero sensitive data when no longer needed (use zeroize crate in Rust)

## Deployment Models

### Desktop Application
- **Storage:** OS keyring (primary)
- **Auth Flow:** OAuth PKCE with browser
- **Fallback:** App password with encrypted file

### CI/CD / GitHub Actions
- **Storage:** Environment variables or GitHub Secrets
- **Auth Flow:** App password (simplest)
- **Alternative:** Pre-generated access/refresh tokens

### Server / Daemon
- **Storage:** Encrypted file or secrets manager
- **Auth Flow:** App password or device flow
- **Management:** CLI for initial login, then automatic refresh

### Container / Kubernetes
- **Storage:** Kubernetes Secrets or external secrets manager
- **Auth Flow:** App password with mounted secrets
- **Refresh:** Sidecar pattern for token refresh

## Testing Strategy

### Unit Tests
- Credential serialization/deserialization
- Token expiry checking
- PKCE challenge/verifier generation
- DPoP JWT creation

### Integration Tests
- createSession flow (with mock server)
- refreshSession flow
- Token refresh on expiry
- Multi-account switching

### End-to-End Tests
- Full OAuth flow (with mock OAuth server)
- CLI login command
- MCP tool invocation
- Cross-platform keyring access

### Security Tests
- Token not logged
- File permissions correct
- Encrypted storage works
- Memory doesn't leak tokens

## Migration from Current Implementation

The current JavaScript implementation uses `keytar` for storage and simple password authentication. Migration path:

1. **Phase 1:** Go/Rust servers implement app password auth (parity with JS)
2. **Phase 2:** Add OAuth support (enhancement)
3. **Phase 3:** JS implementation can optionally adopt OAuth using similar patterns

**No breaking changes** - App passwords continue to work indefinitely.

## Alternative Approaches Considered

### Why Not Username/Password Primary?
- BlueSky is deprecating basic auth in favor of OAuth
- App passwords are more secure (limited scope, revocable)
- OAuth provides better user experience for production apps

### Why Not JWT Libraries Instead of AT Protocol Libraries?
- AT Protocol OAuth has specific extensions (DPoP, PAR)
- Purpose-built libraries handle edge cases
- Better integration with AT Protocol services

### Why Not Just Store Tokens in Environment Variables?
- Environment variables can leak (process listings, logs)
- No cross-platform secret storage
- No automatic refresh capability

## Open Questions for User Feedback

1. **Client Registration:** Should we use dynamic client registration or require users to register their own OAuth client?
   - **Recommendation:** Start with dynamic registration, document manual registration for advanced users

2. **Default Auth Method:** What should be the default when user runs `autoreply login`?
   - **Recommendation:** App password (simpler), with clear instructions for OAuth

3. **MCP Tool vs Separate Service:** Should OAuth flows use MCP tools or separate HTTP service?
   - **Recommendation:** Start with MCP tool for app passwords, evaluate separate service for OAuth in Phase 2

4. **Token Refresh Strategy:** Refresh on-demand or proactively in background?
   - **Recommendation:** On-demand initially, consider background refresh in Phase 3 for long-running servers

## Success Criteria

- [ ] User can authenticate with app password via CLI
- [ ] User can authenticate with app password via MCP tool
- [ ] Credentials stored securely in OS keyring
- [ ] File fallback works when keyring unavailable
- [ ] Multiple accounts can be managed simultaneously
- [ ] Tokens refresh automatically before expiry
- [ ] OAuth PKCE flow works in interactive environments
- [ ] Device flow works in headless environments
- [ ] Clear error messages guide users through issues
- [ ] No tokens logged or exposed in process environment
- [ ] Documentation covers all authentication methods
- [ ] Both Go and Rust implementations feature-complete
- [ ] Cross-platform support (macOS, Linux, Windows)

## References

- [AT Protocol OAuth Specification](https://atproto.com/specs/oauth)
- [RFC 7636 - PKCE](https://datatracker.ietf.org/doc/html/rfc7636)
- [RFC 8628 - Device Authorization Grant](https://datatracker.ietf.org/doc/html/rfc8628)
- [RFC 9449 - DPoP](https://datatracker.ietf.org/doc/html/rfc9449)
- [BlueSky OAuth Client Examples](https://github.com/bluesky-social/cookbook/blob/main/oauth-client)
- [docs/11-login.md](./11-login.md) - Requirements wishlist
- [docs/7-detour-rust.md](./7-detour-rust.md) - Library ecosystem review
