# AT Protocol OAuth Implementation Status

## IMPORTANT UPDATE: Implementation Now Follows AT Protocol Spec

As of the latest commit, the OAuth implementation has been corrected to follow the proper AT Protocol OAuth specification.

## What Changed

The implementation now correctly implements the AT Protocol OAuth discovery flow:

1. **Protected Resource Discovery** (Step 1)
   - Queries `/.well-known/oauth-protected-resource` on the PDS
   - Retrieves the list of authorization servers
   
2. **Authorization Server Discovery** (Step 2)
   - Queries `/.well-known/oauth-authorization-server` on the authorization server
   - Retrieves the OAuth server metadata

Previously, the implementation incorrectly tried to get metadata directly from the PDS at `/.well-known/oauth-authorization-server`, which is not correct per the AT Protocol spec.

## Current Implementation Status

### What IS Implemented ✅

1. **DPoP (Demonstrating Proof of Possession)** - Complete
   - ES256 key pair generation
   - DPoP JWT creation with all required claims (jti, htm, htu, iat, jwk)
   - JWK thumbprint calculation
   - Access token hash (ath) for authenticated requests
   - Key persistence (PEM format)

2. **PKCE (Proof Key for Code Exchange)** - Complete
   - Code verifier generation (43-character cryptographically random string)
   - S256 code challenge calculation
   - Secure verifier storage and retrieval

3. **OAuth Metadata Discovery** - Now Correct ✅
   - Protected resource metadata discovery from PDS
   - Authorization server discovery
   - Full metadata parsing
   - Proper timeout handling

4. **PAR (Pushed Authorization Request)** - Complete
   - PAR endpoint request with DPoP proof
   - Request URI reception
   - Parameter encoding per spec

5. **Authorization Code Flow** - Complete
   - Local callback server (port 8080)
   - Browser opening
   - Authorization code reception
   - State parameter validation

6. **Token Exchange** - Complete
   - Authorization code to token exchange
   - DPoP-bound token requests
   - DPoP nonce retry logic
   - Refresh token support

7. **Authenticated Requests** - Complete
   - DPoP-bound API requests
   - Access token hash (ath) inclusion
   - Proper Authorization header format

### What is NOT Implemented ❌

1. **Client Metadata Document** - Not Implemented
   - The implementation does NOT publish a client metadata JSON document on the public web
   - The `client_id` is currently hardcoded as "autoreply-mcp-client" instead of being an HTTPS URL
   - This is REQUIRED by the AT Protocol OAuth spec
   - **Impact**: The OAuth flow will fail because the authorization server cannot fetch client metadata

2. **Proper Client ID** - Not Implemented
   - Client ID must be a fully-qualified HTTPS URL pointing to the client metadata document
   - Current implementation uses a string identifier instead

3. **Scopes** - Incomplete
   - Implementation requests generic scopes but not specific AT Protocol scopes
   - Should request `atproto` and `transition:generic` scopes

## Why OAuth Doesn't Work Right Now

The OAuth implementation will FAIL because:

1. **No Client Metadata**: The authorization server expects to fetch client metadata from the `client_id` URL, but there's no publicly hosted metadata document

2. **Invalid Client ID**: Using "autoreply-mcp-client" instead of an HTTPS URL violates the spec

3. **PDS May Not Have OAuth**: Even with proper metadata, many PDS instances don't have OAuth enabled yet

## What Needs to Be Done

To make OAuth functional:

### Option 1: For End Users (Recommended)
Use app passwords - they work reliably:
```bash
./autoreply login --method password --handle your.handle.bsky.social
```

### Option 2: For Developers (To Make OAuth Work)

1. **Host Client Metadata**
   - Create a web server or use GitHub Pages to host `client-metadata.json`
   - Include all required fields per AT Protocol spec
   - Make it accessible via HTTPS

2. **Update Client ID**
   - Change `client_id` from "autoreply-mcp-client" to the full HTTPS URL of the metadata document
   - Example: `https://autoreply.example.com/oauth/client-metadata.json`

3. **Example Client Metadata** (what needs to be hosted):
```json
{
  "client_id": "https://autoreply.example.com/oauth/client-metadata.json",
  "application_type": "native",
  "client_name": "Autoreply MCP CLI",
  "dpop_bound_access_tokens": true,
  "grant_types": ["authorization_code", "refresh_token"],
  "redirect_uris": ["http://127.0.0.1:8080/callback"],
  "response_types": ["code"],
  "scope": "atproto transition:generic",
  "token_endpoint_auth_method": "none"
}
```

## Testing OAuth Support

To test if a PDS supports OAuth:

```bash
# Step 1: Check protected resource
curl https://YOUR_PDS_URL/.well-known/oauth-protected-resource

# Step 2: Get authorization server from the response
# Step 3: Check authorization server metadata
curl https://AUTH_SERVER_URL/.well-known/oauth-authorization-server
```

If both requests return valid JSON, OAuth should be available.

## Summary

| Component | Status | Notes |
|-----------|--------|-------|
| DPoP | ✅ Complete | Fully spec-compliant |
| PKCE | ✅ Complete | S256 implementation |
| PAR | ✅ Complete | With DPoP support |
| OAuth Discovery | ✅ Complete | Now follows AT Protocol spec |
| Token Exchange | ✅ Complete | With DPoP nonce retry |
| Token Refresh | ✅ Complete | DPoP-bound refreshes |
| Callback Server | ✅ Complete | HTTP server on port 8080 |
| **Client Metadata** | ❌ **Missing** | **No public metadata document** |
| **Client ID** | ❌ **Invalid** | **Not an HTTPS URL** |

**Bottom Line**: The OAuth **code** is implemented correctly and follows the AT Protocol specification for DPoP, PKCE, PAR, and the authorization flow. However, it cannot work without a publicly hosted client metadata document and proper client ID URL. Users should use app passwords until this infrastructure is set up.

