# OAuth Implementation - Next Steps

## Current Status

After reviewing the official atproto OAuth documentation, I've identified all the issues with the current implementation. The good news is that **BlueSky fully supports OAuth** - our implementation just needs to be fixed to follow the atproto spec properly.

## What's Wrong (Summary)

The current implementation tried to use generic OAuth 2.0 patterns, but atproto has specific requirements:

1. **Discovery is mandatory** - Must resolve handle → DID → PDS → Authorization Server
2. **PAR is required** - Can't redirect directly to authorization endpoint
3. **DPoP with nonces** - More complex than basic DPoP
4. **No device flow** - atproto spec only defines authorization_code grant
5. **Client metadata** - Need proper client_id setup

## Implementation Approach

There are two options for fixing this:

### Option A: Use the Official SDK ✅ **RECOMMENDED**

The `atproto-oauth` crate we already have in dependencies provides ALL the necessary functionality:

```rust
// The atproto-oauth crate handles:
// - Identity resolution (handle → DID → PDS)
// - Authorization server discovery
// - PAR with PKCE and DPoP
// - Nonce management
// - Token exchange and refresh
// - DID verification

use atproto_oauth::client::OAuthClient;
use atproto_oauth::storage::StateStore;

// This is the right way to do it!
```

**Benefits:**
- Spec-compliant out of the box
- Handles all edge cases
- Maintained by BlueSky team
- Already in our dependencies

**Time:** 2-4 hours to integrate properly

### Option B: Manual Implementation

Implement everything from scratch following the spec:
- 200+ lines of discovery code
- 150+ lines of PAR logic
- 100+ lines of DPoP with nonces
- 50+ lines of token management
- Complex error handling

**Time:** 8-12 hours + debugging

## Recommendation

**Use Option A (Official SDK)**. The `atproto-oauth` crate is specifically designed for this and handles all the atproto-specific requirements.

## Implementation Steps (Option A)

### 1. Review atproto-oauth Documentation

```bash
# Check what the crate provides
cargo doc --package atproto-oauth --open
```

Key modules to understand:
- `client` - Main OAuth client
- `storage` - State and session storage
- `dpop` - DPoP implementation
- `resolver` - Identity resolution

### 2. Replace Current OAuth Module

Create new `src/auth/oauth_atproto.rs`:

```rust
use atproto_oauth::client::{OAuthClient, OAuthClientConfig};
use atproto_oauth::storage::MemoryStateStore; // Or FileStateStore
use url::Url;

pub struct AtprotoOAuthManager {
    client: OAuthClient<MemoryStateStore>,
}

impl AtprotoOAuthManager {
    pub async fn new() -> Result<Self, AppError> {
        let config = OAuthClientConfig {
            client_id: "https://autoreply.example.com/client-metadata.json".parse()?,
            redirect_uri: "http://localhost:0/callback".parse()?, // Dynamic port
            scopes: vec!["atproto".to_string(), "transition:generic".to_string()],
        };
        
        let store = MemoryStateStore::new();
        let client = OAuthClient::new(config, store)?;
        
        Ok(Self { client })
    }
    
    pub async fn browser_flow_login(&self, handle: &str) -> Result<Session, AppError> {
        // 1. Start authorization
        let auth_request = self.client.authorize(handle).await?;
        
        // 2. Open browser
        webbrowser::open(&auth_request.url)?;
        
        // 3. Start callback server
        let callback_result = self.wait_for_callback().await?;
        
        // 4. Exchange for tokens
        let token_response = self.client.token_exchange(
            &callback_result.code,
            &callback_result.state
        ).await?;
        
        // 5. Create session
        Ok(Session {
            handle: handle.to_string(),
            did: token_response.sub,
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            // ... etc
        })
    }
}
```

### 3. Update CLI Integration

Minimal changes needed in `main.rs`:

```rust
use crate::auth::oauth_atproto::AtprotoOAuthManager;

async fn execute_login_cli(args: cli::LoginArgs) -> Result<String, AppError> {
    if args.oauth {
        let oauth = AtprotoOAuthManager::new().await?;
        let session = oauth.browser_flow_login(&args.handle).await?;
        // Store session...
    } else {
        // App password flow (unchanged)
    }
}
```

### 4. Remove Device Flow

Delete all device flow code:
- `DeviceAuthResponse` struct
- `start_device_flow()` method
- `poll_device_token()` method  
- `device_flow_login()` method
- CLI `--device` flag

Update docs to explain device flow isn't in atproto spec.

### 5. Client Metadata Decision

**For CLI app, we need to either:**

**A. Host metadata somewhere:**
```json
// https://autoreply.example.com/client-metadata.json
{
  "client_id": "https://autoreply.example.com/client-metadata.json",
  "application_type": "native",
  "client_name": "autoreply CLI",
  "dpop_bound_access_tokens": true,
  "grant_types": ["authorization_code", "refresh_token"],
  "redirect_uris": ["http://localhost/callback"],
  "response_types": ["code"],
  "scope": "atproto transition:generic",
  "token_endpoint_auth_method": "none"
}
```

**B. Use loopback redirect (RFC 8252):**
- Most CLI apps use `http://localhost:{random_port}/callback`
- atproto spec allows this for native apps
- No hosted metadata needed

Recommend **B** for simplicity.

### 6. Testing

```bash
# Build
cargo build --release

# Test login
./target/release/autoreply login --oauth --handle yourhandle.bsky.social

# Should:
# 1. Open browser to BlueSky authorization page
# 2. You authorize the app
# 3. Browser redirects to localhost callback
# 4. CLI receives tokens and stores session
# 5. Success message displayed
```

## File Changes Needed

### New Files
- `src/auth/oauth_atproto.rs` - New SDK-based implementation (200 lines)

### Modified Files
- `src/auth/mod.rs` - Export new module, remove device flow
- `src/auth/oauth.rs` - **DELETE** (replace with oauth_atproto.rs)
- `src/cli.rs` - Remove `--device` flag
- `src/main.rs` - Update to use new OAuth module

### Documentation Updates
- Update all OAuth docs to reflect atproto-specific implementation
- Remove device flow references
- Add examples using the SDK

## Timeline Estimate

- SDK integration: 2 hours
- Remove device flow: 30 minutes
- Testing and debugging: 1 hour
- Documentation updates: 1 hour
- **Total: ~4.5 hours**

Much better than 8-12 hours for manual implementation!

## Questions to Resolve

1. **Where to host client metadata?**
   - Option: Use GitHub Pages (`https://oyin-bo.github.io/autoreply/client-metadata.json`)
   - Option: Use loopback (no hosting needed)
   - **Decision needed before implementing**

2. **Storage backend?**
   - MemoryStateStore (sessions lost on restart)
   - FileStateStore (persist to disk)
   - Custom (integrate with existing keyring storage)
   - **Recommendation**: FileStateStore for now

3. **Session lifetime?**
   - atproto spec: 2 weeks for public clients
   - Need to implement token refresh
   - Should handle refresh automatically

## Benefits After Fix

✅ OAuth will actually work with BlueSky
✅ Proper spec compliance
✅ Better security (DPoP, PAR, PKCE all correct)
✅ Maintained by official SDK
✅ Future-proof for spec changes

## Current PR Status

The PR currently has:
- ✅ App password authentication (working)
- ❌ OAuth browser flow (broken - needs SDK integration)
- ❌ OAuth device flow (needs removal - not in spec)

After fixes:
- ✅ App password authentication (unchanged)
- ✅ OAuth browser flow (working via SDK)
- ❌ Device flow (removed - doesn't exist in atproto)

## References

- [atproto-oauth crate docs](https://docs.rs/atproto-oauth/0.12.0/atproto_oauth/)
- [atproto OAuth spec](https://atproto.com/specs/auth)
- [OAuth client guide](docs/oauth-client.md)

---

**Next Action**: Decide on client metadata hosting approach, then begin SDK integration.
