# BlueSky MCP Authentication: Quick Reference

**For detailed implementation plan, see:** [12-auth-implementation-plan.md](./12-auth-implementation-plan.md)

---

## TL;DR - Key Decisions

### Authentication Methods (Priority Order)
1. **OAuth 2.0 with PKCE** - Primary method for interactive clients
2. **Device Authorization Grant** - For headless/CLI environments
3. **App Passwords** - Legacy fallback (maintain backward compatibility)

### Library Choices

| Feature | Rust | Go |
|---------|------|-----|
| **OAuth/AT Protocol** | `atproto-oauth` + `atproto-client` | `indigo` + adapted code from `haileyok/atproto-oauth-golang` |
| **Credential Storage** | `keyring-rs` (v2.3+) | `zalando/go-keyring` (v0.2+) |
| **Fallback Storage** | AES-256-GCM with `ring` | AES-GCM with `crypto/aes` |

### Multi-Account Support
- ✅ Multiple concurrent authenticated accounts
- ✅ Per-account credential isolation in OS keyring
- ✅ Default account selection with override capability
- ✅ Account-scoped operations via `account` parameter

### MCP Tools Added
1. `login(method, handle, ...)` - Initiate authentication
2. `auth_status(handle?)` - Check authentication status
3. `logout(handle)` - Remove credentials
4. `set_default_account(handle)` - Set default account

### CLI Commands Added
```bash
autoreply login [--method oauth|device|password]
autoreply accounts
autoreply use <handle>
autoreply logout <handle>
```

---

## Implementation Priority

### MVP (Minimum Viable Product)
1. App password authentication (maintain existing)
2. OS keyring storage with file fallback
3. Multi-account management
4. Basic CLI commands

### Phase 2
1. OAuth PKCE flow (browser-based)
2. Device authorization flow
3. Automatic token refresh
4. MCP tool integration

### Phase 3
1. Token lifecycle management
2. Security hardening
3. Migration from old format
4. Comprehensive testing

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   MCP Server                        │
│  ┌───────────────────────────────────────────────┐  │
│  │        Authentication Manager                 │  │
│  │  ┌─────────────┐  ┌─────────────────────────┐ │  │
│  │  │   OAuth     │  │   Credential Storage    │ │  │
│  │  │   Client    │  │  ┌──────────────────┐   │ │  │
│  │  │             │  │  │  OS Keyring      │   │ │  │
│  │  │  • PKCE     │  │  │  (Primary)       │   │ │  │
│  │  │  • Device   │←─┼─→│                  │   │ │  │
│  │  │  • DPoP     │  │  │  Encrypted File  │   │ │  │
│  │  │  • Refresh  │  │  │  (Fallback)      │   │ │  │
│  │  └─────────────┘  │  └──────────────────┘   │ │  │
│  │                   └─────────────────────────┘ │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  ┌───────────────────────────────────────────────┐  │
│  │              MCP Tools                        │  │
│  │  • login          • post   (with auth)        │  │
│  │  • auth_status    • search (with auth)        │  │
│  │  • logout         • ...                       │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

---

## Credential Storage Schema

### OS Keyring Layout
```
Service: autoreply-mcp

Keys:
  alice.bsky.social/access_token
  alice.bsky.social/refresh_token
  alice.bsky.social/dpop_key
  bob.bsky.social/access_token
  bob.bsky.social/refresh_token
  bob.bsky.social/dpop_key
  ...
```

### Config File (Non-sensitive metadata)
```json
{
  "version": "2.0",
  "accounts": [
    {
      "handle": "alice.bsky.social",
      "did": "did:plc:abc123",
      "pds": "https://pds.example.com",
      "storage_ref": "keyring",
      "created_at": "2025-01-01T09:00:00Z",
      "last_used": "2025-01-14T15:45:00Z"
    }
  ],
  "default_account": "alice.bsky.social"
}
```

---

## Security Checklist

### ✅ Must Have
- [x] Store tokens in OS keyring (not plaintext config)
- [x] Encrypted fallback storage with AES-256-GCM
- [x] File permissions 0600 for credential files
- [x] TLS for all OAuth/API calls
- [x] Redact tokens in logs (show only last 4 chars)
- [x] No tokens in environment variables
- [x] DPoP binding for token security

### 🔒 Best Practices
- [x] Automatic token refresh before expiry
- [x] Retry API calls once after token refresh
- [x] Clear error messages without exposing secrets
- [x] Graceful degradation when keyring unavailable
- [x] Prompt user consent for plaintext storage
- [x] Zero sensitive memory after use (best effort)

---

## Testing Requirements

### Platform Coverage
- macOS with Keychain ✓
- Windows with Credential Manager ✓
- Linux with libsecret ✓
- Linux without libsecret (fallback) ✓

### Flow Coverage
- OAuth PKCE (browser) ✓
- Device authorization ✓
- App password ✓
- Token refresh ✓
- Multi-account ✓
- Migration from old format ✓

### Error Scenarios
- Network failure during OAuth ✓
- Cancelled OAuth flow ✓
- Expired refresh token ✓
- Keyring unavailable ✓
- Invalid credentials ✓
- Concurrent access ✓

---

## Common Pitfalls to Avoid

### ❌ Don't
- Store tokens in plaintext config files (use keyring)
- Log full tokens (redact all but last 4 chars)
- Pass tokens via environment variables
- Skip TLS certificate validation
- Assume keyring is always available
- Share DPoP keys between accounts
- Forget to refresh tokens before expiry

### ✅ Do
- Use OS keyring as primary storage
- Implement encrypted fallback storage
- Auto-refresh tokens proactively (5 min before expiry)
- Handle concurrent token refresh with locks
- Validate file permissions (0600)
- Test on all supported platforms
- Provide clear migration path from old format

---

## Development Timeline

| Week | Phase | Deliverables |
|------|-------|-------------|
| 1-2 | Foundation | Credential storage, keyring integration |
| 3-4 | OAuth Client | PKCE, Device flow, DPoP support |
| 5 | CLI Integration | Login commands, account management |
| 6 | MCP Tools | Authentication tools in MCP protocol |
| 7 | Token Lifecycle | Automatic refresh, expiry handling |
| 8 | Testing | Integration tests, security audit |
| 9 | Documentation | Guides, tutorials, troubleshooting |

**Total:** 9 weeks for complete implementation

---

## Next Steps for Developers

### Starting Go Implementation
1. Review `docs/12-auth-implementation-plan.md` sections 2.2 and 7
2. Add `github.com/zalando/go-keyring` dependency
3. Copy OAuth helpers from `haileyok/atproto-oauth-golang`
4. Implement credential manager with keyring + file fallback
5. Add CLI login command with app password support
6. Test on all platforms (macOS, Windows, Linux)

### Starting Rust Implementation
1. Review `docs/12-auth-implementation-plan.md` sections 2.1 and 7
2. Add `keyring = "2.3"` and `atproto-oauth` to Cargo.toml
3. Create credential manager module with keyring + file fallback
4. Implement OAuth client using atproto-oauth crate
5. Add CLI login command with app password support
6. Test on all platforms (macOS, Windows, Linux)

### Parallel Work
- Both implementations can proceed independently
- Share config file format for cross-compatibility
- Coordinate on MCP tool schemas
- Test against same OAuth server for consistency

---

## Questions? Issues?

**For detailed information:**
- See [12-auth-implementation-plan.md](./12-auth-implementation-plan.md) for complete specification
- See [11-login.md](./11-login.md) for original requirements
- See [7-detour-rust.md](./7-detour-rust.md) for library research

**Key contacts:**
- Authentication design questions → Review implementation plan
- Platform-specific issues → Check Appendix C (error codes)
- Security concerns → Review Section 6 (security considerations)
