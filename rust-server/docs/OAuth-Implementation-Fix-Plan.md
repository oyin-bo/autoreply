# OAuth Implementation Fix Plan

## Summary

After reviewing the official atproto OAuth documentation, it's clear that **BlueSky DOES support OAuth**, but our implementation was using incorrect endpoints and missing critical flows. This document outlines what needs to be fixed.

## Current Problems

### 1. No Identity Resolution
- **Current**: Directly tries to hit `https://bsky.social/oauth/authorize`
- **Required**: Must resolve handle → DID → PDS → Authorization Server

### 2. Missing PAR (Pushed Authorization Requests)
- **Current**: Tries to redirect directly to authorization endpoint
- **Required**: Must use PAR endpoint first to get `request_uri`, then redirect

### 3. Incomplete DPoP Implementation
- **Current**: Basic DPoP structure exists but missing nonce handling
- **Required**: Full DPoP with nonce discovery, retry logic, and proper JWT fields

### 4. Device Flow Not Supported
- **Current**: Tries to use `/oauth/device/code` endpoint
- **Reality**: AT Protocol OAuth spec only defines authorization_code grant, not device flow
- **Fix**: Remove device flow entirely (it's not part of the spec)

### 5. Incorrect Client Metadata
- **Current**: Using string `client_id = "autoreply-cli"`
- **Required**: `client_id` must be a URL to a published client metadata JSON

## Implementation Steps

### Phase 1: Fix Discovery Flow ✅

1. **Handle Resolution** (handle →  DID)
   - Use `atproto-identity` crate for handle/DID resolution
   - Support both DNS and HTTP resolution methods
   - Bidirectional verification (DID document must claim handle)

2. **DID → PDS Discovery**
   - Parse DID document to find PDS endpoint
   - Extract `service` entries of type `AtprotoPersonalDataServer`

3. **Authorization Server Discovery**
   - Fetch `/.well-known/oauth-protected-resource` from PDS
   - Get `authorization_servers` array
   - Fetch `/.well-known/oauth-authorization-server` from auth server
   - Parse `pushed_authorization_request_endpoint`, `authorization_endpoint`, `token_endpoint`

### Phase 2: Implement PAR (Pushed Authorization Requests) ✅

1. **Generate PKCE**
   - Create random verifier (32-96 bytes)
   - Compute S256 challenge: SHA256(verifier) | base64url

2. **Generate DPoP Keypair**
   - Use `ES256` (NIST P-256) curve
   - Generate new keypair for each session
   - Never export or reuse keys

3. **Initial PAR Request**
   - POST to `pushed_authorization_request_endpoint`
   - Form-encoded body with: `client_id`, `redirect_uri`, `code_challenge`, `code_challenge_method=S256`, `scope`, `state`, `login_hint`
   - Include DPoP header (will fail with nonce error)

4. **Nonce Discovery**
   - Expect 401 with `use_dpop_nonce` error
   - Extract nonce from `DPoP-Nonce` header
   - Retry PAR with nonce included

5. **Get request_uri**
   - Parse response JSON for `request_uri`
   - Store for authorization redirect

### Phase 3: Browser Authorization Flow ✅

1. **Redirect to Authorization Endpoint**
   - Build URL: `{authorization_endpoint}?client_id={client_id}&request_uri={request_uri}`
   - Open browser or display URL
   - Wait for callback

2. **Handle Callback**
   - Receive: `code`, `state`, `iss`
   - Validate `state` matches
   - Validate `iss` matches authorization server

3. **Token Exchange**
   - POST to `token_endpoint`
   - Form-encoded: `grant_type=authorization_code`, `code`, `code_verifier`, `redirect_uri`, `client_id`
   - Include DPoP header with nonce (may need retry for nonce refresh)
   - Parse response: `access_token`, `refresh_token`, `sub` (DID), `scope`

4. **Verify DID**
   - **Critical**: Verify `sub` DID matches the expected account
   - Compare against original handle's DID

### Phase 4: Client Metadata ✅

Since we're a "native" CLI app (not hosted), we have two options:

**Option A: Public Client (Recommended for CLI)**
- No client metadata URL needed
- Use `client_id` as URL to docs or project page
- Set `token_endpoint_auth_method: "none"`
- This is what mobile/desktop apps typically use

**Option B: Hosted Metadata (If we want confidential client)**
- Host `client-metadata.json` at public URL
- Use that URL as `client_id`
- Include JWKs for client assertion
- Required for web services

For CLI, we'll use Option A.

### Phase 5: Remove Device Flow

Device flow is NOT in the atproto OAuth spec. Remove all device flow code:
- Remove `start_device_flow()` method
- Remove `poll_device_token()` method
- Remove `device_flow_login()` method
- Remove `DeviceAuthResponse` struct
- Update documentation to reflect only browser flow

## Testing Plan

1. **Unit Tests**
   - PKCE generation (verifier + S256 challenge)
   - DPoP JWT creation
   - State/nonce validation
   - DID verification

2. **Integration Tests**
   - Full flow with test account
   - Handle resolution
   - PAR request/response
   - Token exchange
   - Refresh token flow

3. **Manual Testing**
   - `autoreply login --oauth --handle yourhandle.bsky.social`
   - Verify browser opens
   - Complete authorization
   - Verify tokens stored
   - Make authenticated request

## Migration Notes

### For Users

**Before**: OAuth didn't work at all (404s and invalid redirect_uri)
**After**: OAuth browser flow works correctly

**Breaking Changes**: None for app password users (still works)

### For Developers

OAuth flow now follows atproto spec exactly:
1. Handle → DID → PDS discovery
2. PAR with DPoP and PKCE
3. Browser authorization
4. Token exchange with DID verification
5. Refresh with DPoP nonce handling

Device flow removed (not in spec).

## Timeline

- Phase 1-3: Core OAuth implementation (2-3 hours)
- Phase 4: Client metadata decision (30 min)
- Phase 5: Remove device flow (30 min)
- Testing: (1 hour)
- Documentation updates: (1 hour)

**Total**: ~5-6 hours for complete implementation

## References

- [atproto OAuth Specification](https://atproto.com/specs/auth)
- [OAuth Client Implementation Guide](docs/oauth-client.md)
- [OAuth for AT Protocol Blog](docs/2024-09-25-oauth-atproto.md)
- [OAuth Improvements Blog](docs/2025-06-12-oauth-improvements.md)
