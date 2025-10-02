# OAuth Implementation Plan

## Scope

Implementing two additional authentication flows as requested:
1. **OAuth with Page Redirect** - Browser-based OAuth with PKCE + DPoP
2. **Device Authorization Grant** - For headless/CLI environments

## Current State

✅ App password authentication implemented (770 lines)
- Session management
- Token refresh
- Secure storage (keyring + file fallback)
- Multi-account support

## Requirements

### OAuth with Page Redirect

**Dependencies:**
- `atproto-oauth` v0.13.0 - OAuth workflow with PKCE, DPoP
- `atproto-client` v0.13.0 - HTTP client with OAuth support
- `atproto-identity` v0.13.0 - DID resolution for OAuth
- `axum` - Local HTTP server for callback
- `webbrowser` - Open browser for authorization
- `tower` - HTTP service utilities

**Flow:**
1. Generate PKCE code verifier and challenge
2. Create DPoP key pair
3. Start local HTTP server on random port (e.g., `http://localhost:PORT/callback`)
4. Build authorization URL with AT Protocol OAuth parameters
5. Open browser to authorization URL
6. User authorizes on their PDS
7. Callback receives authorization code
8. Exchange code for tokens using DPoP
9. Store tokens and DPoP key securely

**CLI Command:**
```bash
autoreply login --oauth [--handle HANDLE]
```

### Device Authorization Grant

**Dependencies:**
- Same as OAuth redirect

**Flow:**
1. Request device code from authorization server
2. Display verification URL and user code to user
3. Poll token endpoint until:
   - User authorizes (success)
   - Timeout (failure)
   - User denies (failure)
4. Receive tokens on success
5. Store tokens securely

**CLI Command:**
```bash
autoreply login --device [--handle HANDLE]
```

## Implementation Strategy

Given the complexity of implementing OAuth from scratch with DPoP and PKCE, I'll use the `atproto-oauth` crate which provides these primitives. This is a substantial implementation.

### Phase 1: Dependencies & Infrastructure ✅
- Add required crates to Cargo.toml
- Create OAuth module structure
- Basic error handling for OAuth flows

### Phase 2: OAuth Page Redirect
- Implement using `atproto-oauth` client
- Create local HTTP server for callback handling
- Integrate browser opening
- Token exchange and storage

### Phase 3: Device Authorization Grant
- Implement device code flow using `atproto-oauth`
- Display user instructions
- Poll for authorization
- Token storage

### Phase 4: CLI Integration
- Add `--oauth` and `--device` flags to login command
- Update help text and documentation
- Add examples

### Phase 5: Testing & Documentation
- Unit tests for OAuth flows
- Integration tests
- Update all documentation

## Estimated Effort

**Total:** 10-15 hours for complete, production-ready implementation

This is a significant feature that requires careful implementation for security and reliability.

## Next Steps

Starting with Phase 1: Adding dependencies and creating module structure.
