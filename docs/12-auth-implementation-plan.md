# BlueSky MCP Authentication Implementation Plan

**Status:** Research and specification phase  
**Target:** Go and Rust MCP server implementations  
**Date:** 2025-01-01

## Executive Summary

This document provides a comprehensive, practical, and well-researched plan for implementing authentication in the autoreply MCP server for both Go and Rust implementations. It covers BlueSky's AT Protocol OAuth mechanisms, credential storage strategies, library recommendations, and concrete implementation paths.

---

## 1. BlueSky/AT Protocol Authentication Overview

### 1.1 Authentication Mechanisms Supported by BlueSky

BlueSky's AT Protocol supports multiple authentication methods:

1. **OAuth 2.0 with PKCE and DPoP** (Preferred)
   - Modern, secure client authentication
   - PKCE (Proof Key for Code Exchange) prevents authorization code interception
   - DPoP (Demonstrating Proof of Possession) binds tokens to specific clients
   - Best for interactive applications with browser access

2. **OAuth 2.0 Device Authorization Grant** (Device Flow)
   - Designed for devices with limited input capabilities
   - User authenticates on a separate device (phone/computer)
   - Ideal for CLI tools, headless servers, CI/CD environments

3. **App Passwords** (Legacy/Fallback)
   - Username + app-specific password authentication
   - Simpler but less secure than OAuth
   - Useful as fallback when OAuth is unavailable
   - Currently supported by existing Node.js implementation

### 1.2 AT Protocol OAuth Flow Details

**Key Components:**
- **Authorization Server:** BlueSky's OAuth server (handles user consent)
- **Personal Data Server (PDS):** User's data host (varies per user)
- **Client:** The MCP server requesting access
- **DPoP:** Cryptographic binding of access tokens to the client

**OAuth Flow Steps:**
1. Client initiates authorization request (with PKCE challenge)
2. User is redirected to authorization server for consent
3. Authorization server returns code to client
4. Client exchanges code for access token (with PKCE verifier + DPoP proof)
5. Client uses access token + DPoP proof for API requests
6. Client refreshes tokens when expired (automatic)

**Token Types:**
- **Access Token:** Short-lived (minutes to hours), used for API calls
- **Refresh Token:** Long-lived (days to months), used to obtain new access tokens
- **DPoP Key Pair:** Cryptographic keys bound to the client instance

---

## 2. Library Research and Recommendations

### 2.1 Rust: OAuth and Credential Storage

#### OAuth/AT Protocol Libraries

**Primary Recommendation: `atproto-oauth` + `atproto-client`**

- **Package:** [`atproto-oauth`](https://docs.rs/atproto-oauth)
  - **Modules:** `dpop`, `pkce`, `jwk`, `jwt`, `workflow`, `storage`, `resources`
  - **Coverage:** Complete AT Protocol OAuth implementation
  - **DPoP Support:** Built-in DPoP JWT creation and verification
  - **PKCE Support:** S256 code challenge/verifier helpers
  - **PAR Support:** Pushed Authorization Request helpers
  - **Status:** Actively maintained, documented on docs.rs
  - **WASM:** Compatible with wasm32 targets (audit crypto dependencies)

- **Package:** [`atproto-client`](https://docs.rs/atproto-client)
  - **Features:** HTTP client with DPoP authentication
  - **CLI Helpers:** Modules for CLI authentication flows
  - **Integration:** Works seamlessly with `atproto-oauth`

**Supporting Libraries:**
- `atproto-identity` - DID resolution and key utilities
- `atproto-oauth-aip` - Provider-side workflows (optional)

**Evaluation:**
- ✅ **Pros:** Complete, native AT Protocol support; compact WASM output; strong type safety
- ⚠️ **Cons:** Relatively new ecosystem; verify crypto backend compatibility for deployment targets

#### Credential Storage Libraries

**Primary Recommendation: `keyring-rs`**

- **Package:** [`keyring`](https://crates.io/crates/keyring)
  - **Version:** 2.x (stable)
  - **Platforms:** macOS (Keychain), Windows (Credential Manager), Linux (Secret Service/libsecret)
  - **API:** Simple key-value store: `set_password()`, `get_password()`, `delete_password()`
  - **Example:**
    ```rust
    use keyring::Entry;
    
    let entry = Entry::new("autoreply-mcp", "user@bsky.social")?;
    entry.set_password("token_data_json")?;
    let token = entry.get_password()?;
    ```
  - **Status:** Mature, widely used, actively maintained
  - **Fallback:** Requires manual implementation for unsupported platforms

**Alternative: Manual file-based storage with encryption**

- **Encryption:** Use `ring` or `sodiumoxide` for AES-256-GCM encryption
- **Location:** User-specific config directory (via `dirs` crate)
- **Permissions:** Set file permissions to 0600 (user-only read/write)
- **Format:** JSON with encrypted token fields

**Fallback Strategy:**
1. Try OS keyring first
2. If unavailable, check for encrypted file storage
3. As last resort (with explicit user consent), use plaintext file with strict permissions

### 2.2 Go: OAuth and Credential Storage

#### OAuth/AT Protocol Libraries

**Primary Recommendation: Community implementations + `indigo` primitives**

- **Package:** [haileyok/atproto-oauth-golang](https://github.com/haileyok/atproto-oauth-golang)
  - **Status:** Archived (read-only) as of Sep 2025 - reference implementation only
  - **Coverage:** Complete OAuth flow (PAR, PKCE, DPoP, token exchange)
  - **Components:** `PdsDpopJwt`, `AuthServerDpopJwt`, `SendParAuthRequest`, `ClientAssertionJwt`
  - **Integration:** XRPC client integration for authenticated requests
  - **Recommendation:** Use as reference, copy/adapt code into project (MIT license)

- **Package:** [bluesky-social/indigo](https://github.com/bluesky-social/indigo)
  - **Status:** Actively maintained official Go library (~1.2k stars)
  - **Modules:** `xrpc`, `identity`, `crypto`, `atproto`
  - **Coverage:** Core AT Protocol primitives (not complete OAuth client)
  - **Usage:** Build OAuth client on top of indigo primitives
  - **Benefit:** Official support, long-term maintenance

**Alternative: Fork/maintain community implementation**

- Consider forking `streamplace/atproto-oauth-golang` if active maintenance is needed
- Evaluate maturity and test coverage before adopting

**Implementation Strategy:**
1. Use `indigo` for core XRPC, DID resolution, and crypto
2. Adapt OAuth flow implementation from `haileyok/atproto-oauth-golang`
3. Implement custom OAuth client combining both resources
4. Write comprehensive tests to ensure correctness

#### Credential Storage Libraries

**Primary Recommendation: `go-keyring`**

- **Package:** [zalando/go-keyring](https://github.com/zalando/go-keyring)
  - **Version:** 0.2.x (stable)
  - **Platforms:** macOS, Windows, Linux
  - **API:** Simple functions: `Set()`, `Get()`, `Delete()`
  - **Example:**
    ```go
    import "github.com/zalando/go-keyring"
    
    err := keyring.Set("autoreply-mcp", "user@bsky.social", tokenDataJSON)
    token, err := keyring.Get("autoreply-mcp", "user@bsky.social")
    ```
  - **Status:** Mature, widely used, maintained by Zalando
  - **Backend:** Uses native OS APIs (same as keyring-rs)

**Alternative: `github.com/99designs/keyring`**

- More features (multiple backends, testing support)
- Slightly more complex API
- Good for advanced use cases

**Fallback Strategy (same as Rust):**
1. Try OS keyring first
2. Encrypted file storage with standard library `crypto/aes`
3. Plaintext with user consent and strict permissions (0600)

**File Storage Implementation:**
- Location: Use `os.UserConfigDir()` + `/autoreply-mcp/`
- Encryption: `crypto/aes` with GCM mode
- Key derivation: `crypto/scrypt` from user-provided password or machine-specific salt

---

## 3. Authentication Architecture

### 3.1 Multiple Concurrent Logins

**Requirements:**
- MCP server must support multiple authenticated BlueSky accounts simultaneously
- Each account identified by handle or DID
- Credential storage must support multiple entries
- API must allow selecting which account to use for operations

**Storage Schema:**

```json
{
  "accounts": [
    {
      "handle": "alice.bsky.social",
      "did": "did:plc:abc123xyz789",
      "pds": "https://pds.example.com",
      "access_token": "encrypted_or_keyring_ref",
      "refresh_token": "encrypted_or_keyring_ref",
      "dpop_private_key": "encrypted_or_keyring_ref",
      "expires_at": "2025-01-15T10:30:00Z",
      "created_at": "2025-01-01T09:00:00Z",
      "last_used": "2025-01-14T15:45:00Z"
    },
    {
      "handle": "bob.bsky.social",
      "did": "did:plc:xyz789abc123",
      ...
    }
  ],
  "default_account": "alice.bsky.social"
}
```

**Keyring Organization (OS-native storage):**
- Service name: `autoreply-mcp`
- Account key: `{handle}/access_token`, `{handle}/refresh_token`, `{handle}/dpop_key`
- Metadata stored in plaintext config file (non-sensitive data)
- Sensitive tokens stored in OS keyring

### 3.2 Deployment Model Support

**1. Interactive Desktop (Full OAuth with Browser)**
- Use PKCE authorization code flow
- Open system browser for user consent
- Local HTTP server on `localhost:8080` for callback
- Works for: Desktop MCP clients, interactive terminals

**2. Headless CLI / CI/CD (Device Flow)**
- Use OAuth device authorization grant
- Display device code and verification URL to user
- User completes auth on separate device
- Poll for token while user authenticates
- Works for: Remote SSH sessions, GitHub Actions, automated agents

**3. HTTP MCP Server (Long-running Daemon)**
- Support both OAuth flows depending on initial setup
- Store credentials persistently for reuse
- Automatic token refresh in background
- Health check endpoint to verify auth status

**4. CLI Login Command**
- Standalone `autoreply login` command
- Performs OAuth flow and stores credentials
- Used for initial setup or when OAuth inside MCP fails
- Can use app password as fallback

### 3.3 Token Lifecycle Management

**Automatic Refresh Strategy:**

```
Client Request → Check Token Expiry → Valid? → Use Token → Make API Call
                        ↓
                   Expired?
                        ↓
         Use Refresh Token → Get New Access Token → Update Storage → Retry Request
                        ↓
              Refresh Failed?
                        ↓
         Notify User → Trigger Re-authentication
```

**Implementation Details:**
- Check expiry 5 minutes before actual expiration
- Refresh tokens proactively during idle time
- Lock credential access during refresh to prevent race conditions
- Retry API calls once after token refresh
- Fail with clear "re-authentication required" error if refresh fails

**Token Storage Security:**
- Never log tokens (redact in all log output)
- Never pass tokens via environment variables
- Use secure memory handling (zero memory after use if possible)
- Rotate DPoP keys periodically (optional enhancement)

---

## 4. MCP Server API Contract

### 4.1 Should Login Be a Separate MCP Tool?

**Recommendation: Yes, with caveats**

**Rationale:**
- ✅ **Consistency:** Keeps authentication within MCP protocol
- ✅ **Discoverability:** Clients can discover login capability via `tools/list`
- ✅ **Programmatic:** Allows automated flows and testing
- ⚠️ **Complexity:** OAuth flows with browser redirects are challenging in pure MCP
- ⚠️ **UX:** Device flow requires displaying codes/URLs to user

**Hybrid Approach (Recommended):**
1. **MCP Tool:** `login` - Initiates login flow, returns status/instructions
2. **CLI Command:** `autoreply login` - Standalone command for problematic cases
3. **Environment Variables:** Support pre-configured credentials for automation

### 4.2 Proposed MCP Authentication Tools

#### Tool: `login`

**Purpose:** Initiate authentication flow for a BlueSky account

**Parameters:**
```json
{
  "method": "oauth" | "device" | "password",
  "handle": "alice.bsky.social",
  "password": "app-password-xxxx",  // Only for password method
  "callback_port": 8080              // Only for oauth method
}
```

**Returns (OAuth/Device Flow):**
```json
{
  "status": "pending",
  "flow_type": "oauth",
  "auth_url": "https://bsky.app/oauth/authorize?...",
  "message": "Open this URL in your browser to complete authentication"
}
```

**Returns (Device Flow):**
```json
{
  "status": "pending",
  "flow_type": "device",
  "device_code": "ABCD-EFGH",
  "verification_uri": "https://bsky.app/device",
  "user_code": "WXYZ-1234",
  "message": "Visit https://bsky.app/device and enter code: WXYZ-1234",
  "poll_interval": 5
}
```

**Returns (Password Method):**
```json
{
  "status": "success",
  "handle": "alice.bsky.social",
  "did": "did:plc:abc123",
  "message": "Successfully authenticated as @alice.bsky.social"
}
```

#### Tool: `auth_status`

**Purpose:** Check status of pending authentication or list active accounts

**Parameters:**
```json
{
  "handle": "alice.bsky.social"  // Optional, omit to list all accounts
}
```

**Returns:**
```json
{
  "accounts": [
    {
      "handle": "alice.bsky.social",
      "did": "did:plc:abc123",
      "authenticated": true,
      "expires_at": "2025-01-15T10:30:00Z",
      "default": true
    }
  ],
  "default_account": "alice.bsky.social"
}
```

#### Tool: `logout`

**Purpose:** Remove stored credentials for an account

**Parameters:**
```json
{
  "handle": "alice.bsky.social"
}
```

**Returns:**
```json
{
  "status": "success",
  "message": "Logged out from @alice.bsky.social"
}
```

#### Tool: `set_default_account`

**Purpose:** Set which account is used by default for operations

**Parameters:**
```json
{
  "handle": "bob.bsky.social"
}
```

**Returns:**
```json
{
  "status": "success",
  "default_account": "bob.bsky.social"
}
```

### 4.3 Integration with Existing Tools

**Modified Tool Parameters:**

Existing tools (profile, search, post, etc.) should accept an optional `account` parameter:

```json
{
  "name": "post",
  "arguments": {
    "account": "alice.bsky.social",  // Optional, uses default if omitted
    "text": "Hello from MCP!",
    ...
  }
}
```

**Behavior:**
- If `account` specified, use that account's credentials
- If omitted, use default account
- Error if no default account and none specified
- Error if specified account not authenticated

---

## 5. CLI User Experience

### 5.1 Login Command Flow

**Interactive OAuth (Browser Available):**
```bash
$ autoreply login
Choose authentication method:
  1) OAuth (browser-based) [Recommended]
  2) Device code (for remote/headless)
  3) App password (legacy)

Selection: 1

Enter your BlueSky handle: alice.bsky.social

Opening browser for authentication...
Waiting for authorization...

✓ Successfully authenticated as @alice.bsky.social
  DID: did:plc:abc123xyz789
  PDS: https://pds.example.com

Credentials stored securely in system keyring.
```

**Device Flow (Headless):**
```bash
$ autoreply login --method device

Enter your BlueSky handle: bob.bsky.social

Please visit: https://bsky.app/device
And enter this code: WXYZ-1234

Waiting for authorization... (press Ctrl+C to cancel)

✓ Successfully authenticated as @bob.bsky.social
  DID: did:plc:xyz789abc123

Credentials stored securely in system keyring.
```

**App Password (Legacy):**
```bash
$ autoreply login --method password

Enter your BlueSky handle: charlie.bsky.social
Enter app password: ****-****-****-****

✓ Successfully authenticated as @charlie.bsky.social
  DID: did:plc:xyz789abc123

Credentials stored securely in system keyring.
```

### 5.2 Account Management Commands

**List Accounts:**
```bash
$ autoreply accounts

Authenticated Accounts:
  ✓ alice.bsky.social (default)
    DID: did:plc:abc123xyz789
    Expires: 2025-01-15 10:30:00

  ✓ bob.bsky.social
    DID: did:plc:xyz789abc123
    Expires: 2025-01-20 14:45:00
```

**Switch Default Account:**
```bash
$ autoreply use bob.bsky.social
✓ Default account set to @bob.bsky.social
```

**Logout:**
```bash
$ autoreply logout alice.bsky.social
✓ Logged out from @alice.bsky.social
  Credentials removed from system keyring.
```

**Using Specific Account for Operations:**
```bash
$ autoreply post --account alice.bsky.social --text "Hello!"
✓ Posted as @alice.bsky.social
  URI: at://alice.bsky.social/app.bsky.feed.post/abc123
```

### 5.3 Error Messages and Guidance

**Examples:**

```
Error: No authenticated accounts found.
→ Run 'autoreply login' to authenticate.

Error: Token expired and refresh failed.
→ Please re-authenticate: autoreply login alice.bsky.social

Error: System keyring not available.
→ Using fallback encrypted file storage.
→ Create a master password for encryption: ****

Error: Network error during OAuth flow.
→ Check your internet connection and try again.
→ If using a proxy, set HTTP_PROXY environment variable.

Warning: Storing credentials in plaintext.
→ This is not secure. Install system keyring support:
  - macOS: Built-in (Keychain)
  - Linux: Install libsecret (apt install libsecret-1-0)
  - Windows: Built-in (Credential Manager)
```

---

## 6. Security Considerations

### 6.1 Token Storage Security

**OS Keyring (Preferred):**
- Encrypted by OS with user's login credentials
- Isolated from other applications
- Survives system reboots
- Protected by OS-level access controls

**Encrypted File Storage (Fallback):**
- AES-256-GCM encryption
- Key derived from user password via scrypt/pbkdf2
- Integrity verification with HMAC
- File permissions: 0600 (user-only read/write)
- Location: User-specific config directory

**Plaintext File Storage (Last Resort):**
- Only with explicit user consent
- Display prominent warning
- File permissions: 0600
- Log file path for user awareness
- Recommend alternative setup methods

### 6.2 Network Security

**TLS:**
- All OAuth and API calls over HTTPS
- Certificate validation enabled (no insecure skip)
- Use OS certificate store for trust

**Proxy Support:**
- Honor HTTP_PROXY, HTTPS_PROXY environment variables
- Support proxy authentication if needed
- No credential leakage through proxy logs

**DPoP Security:**
- Bind tokens to specific client instances
- Use unique DPoP key pair per account
- Rotate keys on security events (optional)

### 6.3 Process Security

**Memory Handling:**
- Zero sensitive data in memory after use (best effort)
- Avoid string immutability issues with tokens
- No swap to disk for credential pages (OS-dependent)

**Logging:**
- Never log full tokens (redact to last 4 chars: "xxx...abc1")
- Log authentication events (login, logout, refresh)
- Sanitize error messages to avoid token leakage

**Environment Variables:**
- Don't pass tokens via env vars (use files or keyring)
- Support env vars for non-sensitive config only
- Clear sensitive env vars after reading

---

## 7. Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

**Go Implementation:**
- [ ] Research and evaluate go-keyring integration
- [ ] Design credential storage schema
- [ ] Implement basic keyring wrapper with fallback
- [ ] Create credential manager struct
- [ ] Unit tests for credential storage

**Rust Implementation:**
- [ ] Research and evaluate keyring-rs integration
- [ ] Design credential storage schema (same as Go)
- [ ] Implement keyring wrapper with fallback
- [ ] Create credential manager module
- [ ] Unit tests for credential storage

**Common:**
- [ ] Document storage format and migration paths
- [ ] Create test fixtures and mock credentials

### Phase 2: OAuth Client (Week 3-4)

**Go Implementation:**
- [ ] Adapt OAuth code from haileyok/atproto-oauth-golang
- [ ] Integrate with indigo for XRPC calls
- [ ] Implement PKCE flow with browser callback
- [ ] Implement device authorization flow
- [ ] Add DPoP JWT generation and signing
- [ ] Unit tests for OAuth components

**Rust Implementation:**
- [ ] Integrate atproto-oauth crate
- [ ] Implement PKCE flow with local HTTP server
- [ ] Implement device authorization flow
- [ ] Configure DPoP support from atproto-client
- [ ] Unit tests for OAuth components

**Common:**
- [ ] Test OAuth flows against real BlueSky OAuth server
- [ ] Handle edge cases (timeouts, cancellation, errors)

### Phase 3: CLI Integration (Week 5)

**Both Implementations:**
- [ ] Add `login` command with method selection
- [ ] Add `accounts` command to list authenticated accounts
- [ ] Add `logout` command to remove credentials
- [ ] Add `use` command to set default account
- [ ] Implement interactive prompts with proper UX
- [ ] Add help text and documentation

### Phase 4: MCP Tool Integration (Week 6)

**Both Implementations:**
- [ ] Add `login` MCP tool
- [ ] Add `auth_status` MCP tool
- [ ] Add `logout` MCP tool
- [ ] Add `set_default_account` MCP tool
- [ ] Modify existing tools to accept `account` parameter
- [ ] Update tool schemas and documentation

### Phase 5: Token Lifecycle (Week 7)

**Both Implementations:**
- [ ] Implement automatic token refresh
- [ ] Add token expiry checking before API calls
- [ ] Handle refresh failures gracefully
- [ ] Add background refresh for idle periods
- [ ] Implement retry logic for expired tokens
- [ ] Add token refresh logging and metrics

### Phase 6: Testing and Hardening (Week 8)

**Both Implementations:**
- [ ] Integration tests with real OAuth flows
- [ ] Test multi-account scenarios
- [ ] Test token refresh and expiry handling
- [ ] Test fallback storage mechanisms
- [ ] Security audit (token handling, file permissions)
- [ ] Performance testing with multiple accounts
- [ ] Error handling and edge case coverage

### Phase 7: Documentation and Polish (Week 9)

**Both Implementations:**
- [ ] Update README with authentication guide
- [ ] Create authentication setup tutorial
- [ ] Document troubleshooting common issues
- [ ] Add architecture diagrams
- [ ] Create migration guide from app passwords
- [ ] Record demo video of authentication flows

---

## 8. Migration Strategy

### 8.1 Backward Compatibility

**Support Existing App Passwords:**
- Detect legacy app password storage format
- Migrate to new format on first use
- Maintain support for app password method as fallback

**Migration Path:**
```
Old Format (keytar):
  Service: autoreply
  Account: user@bsky.social
  Password: app-password-value

New Format (keyring):
  Service: autoreply-mcp
  Accounts: user@bsky.social/access_token, user@bsky.social/refresh_token, ...
```

**Migration Code:**
```rust
// Check for old format
if let Ok(old_password) = keyring.get("autoreply", handle) {
    // Migrate to new format
    // 1. Authenticate with app password
    // 2. Obtain OAuth tokens
    // 3. Store in new format
    // 4. Delete old entry
    // 5. Log migration success
}
```

### 8.2 Version Detection and Upgrade

**Config Version Field:**
```json
{
  "version": "2.0",
  "migration_date": "2025-01-15T10:30:00Z",
  "accounts": [...]
}
```

**Upgrade Process:**
- Detect old version on startup
- Prompt user for migration consent
- Backup old credentials before migration
- Perform migration with rollback on failure
- Log migration events

---

## 9. Testing Strategy

### 9.1 Unit Tests

**Credential Storage:**
- Store and retrieve single account
- Store and retrieve multiple accounts
- Update existing account
- Delete account
- Handle missing keyring (fallback)
- Encryption/decryption (file storage)
- File permissions verification

**OAuth Flows:**
- PKCE code challenge generation
- DPoP JWT creation and signing
- Token exchange with mock server
- Token refresh with mock server
- Error handling (network, invalid response)

### 9.2 Integration Tests

**OAuth with Real Server:**
- Complete PKCE flow (manual testing)
- Complete device flow (manual testing)
- App password authentication
- Token refresh after expiry
- Multi-account management

**MCP Protocol:**
- Login tool calls
- Auth status queries
- Logout operations
- Using specific account for operations

### 9.3 Manual Testing Checklist

**Platforms:**
- [ ] macOS with Keychain
- [ ] Windows with Credential Manager
- [ ] Linux with libsecret
- [ ] Linux without libsecret (fallback)

**Scenarios:**
- [ ] Fresh install, first login
- [ ] Second account login
- [ ] Switch default account
- [ ] Token refresh after expiry
- [ ] Network failure during OAuth
- [ ] Cancel OAuth flow mid-process
- [ ] Logout and re-login
- [ ] Migration from old format

---

## 10. Open Questions and Future Enhancements

### 10.1 Open Questions

1. **Token Rotation Frequency:** How often should we proactively refresh tokens?
   - Recommendation: Refresh when < 5 minutes remaining

2. **Session Persistence:** Should sessions survive app restarts?
   - Recommendation: Yes, store refresh tokens persistently

3. **Key Rotation:** Should DPoP keys rotate periodically?
   - Recommendation: Optional enhancement, not MVP requirement

4. **Multi-PDS Support:** How to handle users with multiple PDS hosts?
   - Recommendation: Discover PDS per account, store in metadata

5. **Credential Sharing:** Should credentials be shareable across MCP instances?
   - Recommendation: Yes, use shared keyring/file location

### 10.2 Future Enhancements

**Phase 2 Features:**
- OAuth with custom redirect URLs (for web-based MCP clients)
- SSO integration for enterprise deployments
- Hardware security key support (WebAuthn)
- Credential import/export (encrypted format)
- Account activity monitoring and anomaly detection
- Rate limit handling with backoff

**Advanced Security:**
- Hardware-backed keyring (TPM, Secure Enclave)
- Biometric authentication for credential access
- Per-operation consent prompts (high-risk actions)
- Audit logging for compliance

**Developer Experience:**
- Mock authentication mode for testing
- Credential validation tool
- Authentication flow debugging mode
- GraphQL-style account selection

---

## 11. Conclusion

This implementation plan provides a comprehensive, practical roadmap for adding authentication support to the autoreply MCP server. Key highlights:

✅ **Well-Researched:** Based on actual library capabilities and AT Protocol specs  
✅ **Practical:** Concrete code examples and library recommendations  
✅ **Secure:** Multiple layers of credential protection with sensible fallbacks  
✅ **User-Friendly:** Clear CLI flows and helpful error messages  
✅ **Future-Proof:** Support for OAuth while maintaining backward compatibility  
✅ **Cross-Platform:** Works on macOS, Windows, and Linux  

**Next Steps:**
1. Review and approve this plan
2. Begin Phase 1 implementation (credential storage)
3. Develop proof-of-concept for OAuth flows
4. Iterate based on testing and feedback

**Timeline:** 9 weeks for complete implementation with testing and documentation

---

## Appendices

### Appendix A: Library Version Requirements

**Rust:**
```toml
[dependencies]
keyring = "2.3"
atproto-oauth = "0.1"
atproto-client = "0.1"
atproto-identity = "0.1"
ring = "0.17"  # for encryption
dirs = "5.0"   # for config paths
```

**Go:**
```go
require (
    github.com/zalando/go-keyring v0.2.3
    github.com/bluesky-social/indigo v0.0.0-20250101000000
    golang.org/x/crypto v0.17.0
    golang.org/x/oauth2 v0.15.0
)
```

### Appendix B: Configuration File Format

**Location:**
- Linux/macOS: `~/.config/autoreply-mcp/config.json`
- Windows: `%APPDATA%\autoreply-mcp\config.json`

**Format:**
```json
{
  "version": "2.0",
  "storage_backend": "keyring",
  "accounts": [
    {
      "handle": "alice.bsky.social",
      "did": "did:plc:abc123xyz789",
      "pds": "https://pds.example.com",
      "storage_ref": "keyring",
      "created_at": "2025-01-01T09:00:00Z",
      "last_used": "2025-01-14T15:45:00Z",
      "metadata": {
        "display_name": "Alice",
        "avatar_url": "https://..."
      }
    }
  ],
  "default_account": "alice.bsky.social",
  "settings": {
    "auto_refresh": true,
    "refresh_threshold_minutes": 5,
    "token_rotation_days": 30
  }
}
```

### Appendix C: Error Code Reference

| Code | Description | User Action |
|------|-------------|-------------|
| `auth_required` | No authenticated account | Run `autoreply login` |
| `auth_expired` | Token expired, refresh failed | Re-authenticate with `autoreply login` |
| `auth_invalid` | Invalid credentials | Check username/password |
| `oauth_cancelled` | User cancelled OAuth flow | Restart login process |
| `oauth_timeout` | OAuth flow timed out | Check network and retry |
| `keyring_unavailable` | OS keyring not accessible | Install keyring support or use fallback |
| `permission_denied` | Cannot write credential file | Check file permissions |
| `network_error` | Network failure during auth | Check connection and retry |

### Appendix D: References

**AT Protocol Documentation:**
- OAuth Specification: https://atproto.com/specs/oauth
- XRPC Documentation: https://atproto.com/specs/xrpc
- DID Methods: https://atproto.com/specs/did

**Library Documentation:**
- keyring-rs: https://docs.rs/keyring
- atproto-oauth: https://docs.rs/atproto-oauth
- go-keyring: https://github.com/zalando/go-keyring
- indigo: https://github.com/bluesky-social/indigo

**OAuth Standards:**
- RFC 6749: OAuth 2.0 Framework
- RFC 7636: PKCE
- RFC 9449: DPoP
- RFC 8628: Device Authorization Grant
