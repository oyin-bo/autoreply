# Authentication Implementation: Planning Complete ✅

**Status:** Planning phase complete, ready for implementation  
**Date:** 2025-01-01

---

## What We've Delivered

This planning phase has produced a comprehensive, well-researched, and practical set of documents to guide the implementation of authentication support for the autoreply MCP server in both Go and Rust.

### 📚 Documentation Deliverables

1. **[11-login.md](./11-login.md)** - Original requirements (updated)
   - High-level wishlist and goals
   - Updated with references to implementation plan

2. **[12-auth-implementation-plan.md](./12-auth-implementation-plan.md)** - Complete technical specification (500+ lines)
   - AT Protocol OAuth mechanisms research
   - Library recommendations and evaluations
   - Multi-account architecture design
   - MCP tool specifications
   - CLI user experience flows
   - Security considerations
   - 9-week implementation roadmap
   - Testing and migration strategies

3. **[12-auth-quick-ref.md](./12-auth-quick-ref.md)** - Developer quick reference
   - TL;DR key decisions
   - Library choices summary
   - Architecture diagram
   - Security checklist
   - Common pitfalls
   - Getting started guides

4. **[12-auth-go-rust-comparison.md](./12-auth-go-rust-comparison.md)** - Side-by-side implementation guide
   - Library equivalents mapping
   - Code structure comparison
   - API design in both languages
   - OAuth flow implementations
   - Testing strategy alignment
   - Configuration compatibility

---

## Key Research Findings

### AT Protocol OAuth Support

**BlueSky supports multiple authentication methods:**
1. ✅ OAuth 2.0 with PKCE + DPoP (primary, most secure)
2. ✅ OAuth Device Authorization Grant (for headless/CLI)
3. ✅ App Passwords (legacy, maintain for backward compatibility)

**Critical OAuth Features:**
- **PKCE:** Prevents authorization code interception
- **DPoP:** Cryptographic binding of tokens to clients
- **Token Refresh:** Automatic renewal without user interaction
- **Multi-PDS:** Each user may have different Personal Data Server

### Library Recommendations

#### Rust Implementation
| Component | Library | Version | Notes |
|-----------|---------|---------|-------|
| OAuth/AT Protocol | `atproto-oauth` | 0.1+ | Complete AT Protocol OAuth implementation |
| OAuth Client | `atproto-client` | 0.1+ | HTTP client with DPoP support |
| OS Keyring | `keyring` | 2.3+ | Cross-platform credential storage |
| Encryption | `ring` | 0.17+ | For fallback file storage |
| Config Paths | `dirs` | 5.0+ | Platform-specific directories |

**Rust Advantages:**
- Native AT Protocol OAuth crates (maintained by community)
- Compact WASM output potential
- Strong type safety and zero-cost abstractions
- Excellent cargo ecosystem

#### Go Implementation
| Component | Library | Version | Notes |
|-----------|---------|---------|-------|
| AT Protocol | `bluesky-social/indigo` | latest | Official Go library (xrpc, identity, crypto) |
| OAuth Reference | `haileyok/atproto-oauth-golang` | archived | Reference implementation to adapt |
| OS Keyring | `zalando/go-keyring` | 0.2+ | Cross-platform credential storage |
| Encryption | `crypto/aes` | stdlib | For fallback file storage |
| Config Paths | `os.UserConfigDir()` | stdlib | Platform-specific directories |

**Go Advantages:**
- Official `indigo` library for AT Protocol primitives
- Mature ecosystem and tooling
- Fast compilation and single-binary deployment
- Straightforward concurrency with goroutines

### Cross-Platform Credential Storage

**Primary: OS Keyring**
- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service (libsecret)

**Fallback: Encrypted File**
- AES-256-GCM encryption
- User-scoped config directory
- File permissions: 0600 (user-only)

**Last Resort: Plaintext File**
- Only with explicit user consent
- Prominent security warning
- Document alternative setup methods

---

## Architecture Decisions

### Multi-Account Support ✅

**Design:**
- Support multiple authenticated BlueSky accounts simultaneously
- Per-account credential isolation in OS keyring
- Shared JSON config file for metadata (non-sensitive)
- Default account selection with per-operation override

**Storage Schema:**
```
Keyring:
  autoreply-mcp/alice.bsky.social/access_token
  autoreply-mcp/alice.bsky.social/refresh_token
  autoreply-mcp/alice.bsky.social/dpop_key
  autoreply-mcp/bob.bsky.social/...

Config File:
  {
    "version": "2.0",
    "accounts": [
      { "handle": "alice.bsky.social", "did": "...", "pds": "...", ... }
    ],
    "default_account": "alice.bsky.social"
  }
```

### MCP Server API ✅

**New MCP Tools:**
1. `login(method, handle, ...)` - Initiate authentication flow
2. `auth_status(handle?)` - Check authentication status
3. `logout(handle)` - Remove stored credentials
4. `set_default_account(handle)` - Set default account

**Modified Existing Tools:**
- Add optional `account` parameter to all authenticated operations
- Use default account if not specified
- Error if no default and none specified

### CLI Commands ✅

**New Commands:**
```bash
autoreply login [--method oauth|device|password] [--handle <handle>]
autoreply accounts                    # List authenticated accounts
autoreply use <handle>                # Set default account
autoreply logout <handle>             # Remove credentials
```

**Modified Commands:**
```bash
autoreply post --account alice.bsky.social --text "Hello!"  # Use specific account
autoreply post --text "Hello!"                              # Use default account
```

### Token Lifecycle ✅

**Automatic Refresh Strategy:**
1. Check token expiry before each API call
2. Refresh proactively when < 5 minutes remaining
3. Use refresh token to obtain new access token
4. Store updated tokens in keyring
5. Retry API call with new token
6. Handle refresh failure gracefully (prompt re-authentication)

**Security:**
- Never log full tokens (redact to last 4 chars)
- No tokens in environment variables
- Use secure memory handling where possible
- TLS for all network calls
- Validate certificates (no insecure skip)

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
**Goal:** Credential storage with OS keyring + encrypted fallback

**Deliverables:**
- ✅ Credential manager structure
- ✅ OS keyring integration (via library)
- ✅ Encrypted file storage fallback
- ✅ Config file loading/saving
- ✅ Unit tests for storage operations

**Ready to Start:** Yes - libraries identified, APIs designed

### Phase 2: OAuth Client (Weeks 3-4)
**Goal:** Complete OAuth flows (PKCE, Device, App Password)

**Deliverables:**
- ✅ PKCE authorization code flow
- ✅ Device authorization flow
- ✅ DPoP JWT generation and signing
- ✅ Token refresh mechanism
- ✅ App password authentication (backward compatibility)
- ✅ Unit tests for OAuth components

**Ready to Start:** Yes - reference implementations available

### Phase 3: CLI Integration (Week 5)
**Goal:** User-facing authentication commands

**Deliverables:**
- ✅ `login` command with method selection
- ✅ `accounts` command to list accounts
- ✅ `logout` command to remove credentials
- ✅ `use` command to set default account
- ✅ Interactive prompts with good UX
- ✅ Help text and documentation

**Ready to Start:** Yes - UX flows designed, examples provided

### Phase 4: MCP Tool Integration (Week 6)
**Goal:** Authentication tools in MCP protocol

**Deliverables:**
- ✅ `login` MCP tool
- ✅ `auth_status` MCP tool
- ✅ `logout` MCP tool
- ✅ `set_default_account` MCP tool
- ✅ Modify existing tools for `account` parameter
- ✅ Update JSON schemas

**Ready to Start:** Yes - tool specifications complete

### Phase 5: Token Lifecycle (Week 7)
**Goal:** Automatic token refresh and expiry handling

**Deliverables:**
- ✅ Token expiry checking
- ✅ Automatic refresh before API calls
- ✅ Refresh failure handling
- ✅ Background refresh during idle
- ✅ Retry logic for expired tokens
- ✅ Logging and monitoring

**Ready to Start:** Yes - algorithms specified

### Phase 6: Testing and Hardening (Week 8)
**Goal:** Comprehensive testing and security audit

**Deliverables:**
- ✅ Integration tests with real OAuth
- ✅ Multi-account scenario tests
- ✅ Token refresh tests
- ✅ Platform-specific tests (macOS, Windows, Linux)
- ✅ Security audit (token handling, file permissions)
- ✅ Performance testing
- ✅ Error handling coverage

**Ready to Start:** Yes - test strategy documented

### Phase 7: Documentation and Polish (Week 9)
**Goal:** Complete documentation and migration guide

**Deliverables:**
- ✅ Update READMEs with authentication guide
- ✅ Authentication setup tutorial
- ✅ Troubleshooting guide
- ✅ Architecture diagrams
- ✅ Migration guide from app passwords
- ✅ Demo videos

**Ready to Start:** Yes - documentation structure planned

---

## Success Criteria

### Must Have (MVP)
- [x] Research complete and documented
- [ ] App password authentication works (maintain existing)
- [ ] OS keyring storage implemented
- [ ] Encrypted file fallback works
- [ ] Multi-account support functional
- [ ] Basic CLI commands (login, logout, accounts)
- [ ] Works on macOS, Windows, Linux

### Should Have (Phase 2)
- [ ] OAuth PKCE flow functional
- [ ] Device authorization flow works
- [ ] Automatic token refresh implemented
- [ ] MCP tools integrated
- [ ] Migration from old format works
- [ ] Comprehensive error handling

### Nice to Have (Future)
- [ ] Hardware security key support
- [ ] Biometric authentication
- [ ] Account activity monitoring
- [ ] Credential import/export
- [ ] SSO integration for enterprise

---

## Risk Assessment

### Low Risk ✅
- **Credential storage:** Well-established libraries for both Rust and Go
- **Platform support:** OS keyring APIs are stable and documented
- **Backward compatibility:** App passwords will continue to work

### Medium Risk ⚠️
- **OAuth complexity:** DPoP and PKCE are non-trivial but well-documented
- **Token refresh:** Edge cases around concurrent access need careful handling
- **Testing:** Requires real OAuth server access or good mocks

### Mitigation Strategies
1. **Use proven libraries:** Don't reinvent OAuth, use `atproto-oauth` (Rust) or adapt reference code (Go)
2. **Comprehensive testing:** Test all flows on all platforms early
3. **Fallback mechanisms:** Always have encrypted file storage as backup
4. **Incremental rollout:** Start with app passwords, add OAuth progressively
5. **Community support:** Leverage AT Protocol community for OAuth questions

---

## Next Steps

### Immediate Actions (This Week)

#### For Go Implementation Team:
1. ✅ Review `docs/12-auth-implementation-plan.md` (section 2.2)
2. ✅ Review `docs/12-auth-go-rust-comparison.md`
3. ⏭️ Create feature branch: `feature/auth-go`
4. ⏭️ Add dependencies: `go get github.com/zalando/go-keyring`
5. ⏭️ Implement credential storage module (Phase 1)
6. ⏭️ Write unit tests
7. ⏭️ Test on all platforms

#### For Rust Implementation Team:
1. ✅ Review `docs/12-auth-implementation-plan.md` (section 2.1)
2. ✅ Review `docs/12-auth-go-rust-comparison.md`
3. ⏭️ Create feature branch: `feature/auth-rust`
4. ⏭️ Add dependencies to `Cargo.toml`: `keyring = "2.3"`, `atproto-oauth = "0.1"`
5. ⏭️ Implement credential storage module (Phase 1)
6. ⏭️ Write unit tests
7. ⏭️ Test on all platforms

### Week 1 Milestones
- [ ] Both teams complete credential storage implementation
- [ ] Config file format finalized and documented
- [ ] Unit tests passing on all platforms
- [ ] Cross-team sync meeting to align on progress

### Week 2 Milestones
- [ ] Keyring integration complete with fallback
- [ ] Platform-specific testing complete
- [ ] Documentation for credential storage ready
- [ ] Begin Phase 2 (OAuth client implementation)

---

## Questions & Answers

### Q: Can we start implementation now?
**A:** Yes! All necessary research is complete. Library choices are made, APIs are designed, and code examples are provided.

### Q: Should we implement Go or Rust first?
**A:** Both can proceed in parallel. The documentation supports independent development with coordination points at each phase.

### Q: What if we need to change library choices during implementation?
**A:** The architecture is designed to be flexible. Swap implementations behind the interfaces without changing the overall design.

### Q: How do we handle platform-specific issues?
**A:** Document them in the relevant sections of the implementation plan. Share findings between teams to avoid duplicate effort.

### Q: What if OAuth proves too complex?
**A:** Start with app password support (already working). Add OAuth incrementally as a v2 feature. The design supports both.

### Q: How do we test OAuth flows without affecting production accounts?
**A:** Create dedicated test accounts on BlueSky. Document test credentials in a secure location (not in git). Consider mock server for unit tests.

---

## Resources

### Documentation
- [11-login.md](./11-login.md) - Original requirements
- [12-auth-implementation-plan.md](./12-auth-implementation-plan.md) - Complete specification
- [12-auth-quick-ref.md](./12-auth-quick-ref.md) - Developer quick reference
- [12-auth-go-rust-comparison.md](./12-auth-go-rust-comparison.md) - Side-by-side comparison
- [7-detour-rust.md](./7-detour-rust.md) - Original library research

### External References
- AT Protocol OAuth: https://atproto.com/specs/oauth
- DPoP RFC 9449: https://datatracker.ietf.org/doc/html/rfc9449
- PKCE RFC 7636: https://datatracker.ietf.org/doc/html/rfc7636
- Device Flow RFC 8628: https://datatracker.ietf.org/doc/html/rfc8628

### Libraries
- Rust: https://docs.rs/keyring, https://docs.rs/atproto-oauth
- Go: https://github.com/zalando/go-keyring, https://github.com/bluesky-social/indigo

---

## Conclusion

✅ **Planning Complete**

We have successfully:
- Researched AT Protocol OAuth mechanisms thoroughly
- Evaluated and selected appropriate libraries for Go and Rust
- Designed a comprehensive multi-account authentication architecture
- Specified MCP tools and CLI commands in detail
- Created implementation guides with code examples
- Developed a realistic 9-week roadmap
- Identified risks and mitigation strategies
- Provided testing and migration strategies

**The foundation is solid. Implementation can begin immediately.**

---

**Document Version:** 1.0  
**Last Updated:** 2025-01-01  
**Status:** ✅ Ready for Implementation
