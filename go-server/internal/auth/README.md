# Authentication in Go Server

The Go server implements all three authentication methods for BlueSky AT Protocol as described in `docs/12-auth-plan.md`.

## Overview

The authentication system provides:
- **App Password Authentication**: Simple username/password flow using `com.atproto.server.createSession`
- **OAuth 2.0 with DPoP and PKCE**: Most secure method for interactive clients with browser access
- **Device Authorization Grant**: Best for headless/remote environments without browser
- **Secure Credential Storage**: OS-native keychains with encrypted file fallback
- **Multi-Account Support**: Store and manage multiple authenticated accounts
- **MCP Integration**: Available as both MCP tools and CLI commands

## Architecture

### Components

1. **Credential Store** (`credentials.go`):
   - Manages secure storage using `99designs/keyring`
   - Supports OS keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service)
   - Falls back to encrypted file storage when keyring is unavailable
   - Stores credentials per handle with default handle management

2. **Session Manager** (`session.go`):
   - Handles app password authentication flows
   - Creates sessions using `com.atproto.server.createSession`
   - Supports token refresh using `com.atproto.server.refreshSession`

3. **OAuth Flow** (`oauth.go`):
   - Implements OAuth 2.0 with PKCE (RFC 7636) and DPoP (RFC 9449)
   - Generates authorization URLs for browser-based login
   - Exchanges authorization codes for tokens
   - Supports token refresh with DPoP proofs

4. **Device Authorization Flow** (`device.go`):
   - Implements RFC 8628 Device Authorization Grant
   - Requests device codes and displays user codes
   - Polls for authorization completion
   - Handles slow-down and expiration scenarios

5. **DPoP Support** (`dpop.go`):
   - Generates ECDSA P-256 key pairs
   - Creates DPoP proof JWTs for token binding
   - Calculates JWK thumbprints

6. **PKCE Support** (`pkce.go`):
   - Generates code verifiers and challenges
   - Uses SHA-256 for challenge derivation
   - Base64URL encoding without padding

7. **Callback Server** (`callback.go`):
   - Local HTTP server for OAuth callbacks
   - Captures authorization codes
   - Provides user-friendly success/error pages

8. **MCP Tools**:
   - `LoginTool` (`tools/login.go`): App password authentication
   - `OAuthLoginTool` (`tools/oauth_login.go`): OAuth with PKCE and DPoP
   - `DeviceLoginTool` (`tools/device_login.go`): Device authorization
   - `LogoutTool` (`tools/logout.go`): Remove stored credentials
   - `AccountsTool` (`tools/accounts.go`): List accounts and manage default

## Usage

### CLI Commands

#### App Password Login (Simple)
```bash
autoreply login --handle alice.bsky.social --password <app-password>
```

App passwords can be generated in BlueSky Settings → App Passwords.

#### OAuth Login (Most Secure)
```bash
autoreply oauth-login --port 8080
```

Opens browser for authorization, then automatically completes login.

#### Device Authorization (Headless)
```bash
autoreply device-login
```

Displays verification URL and code for authorization on another device.

#### List Accounts
```bash
autoreply accounts
```

#### Set Default Account
```bash
autoreply accounts --action set-default --handle alice.bsky.social
```

#### Logout
```bash
# Logout default account
autoreply logout

# Logout specific account
autoreply logout --handle alice.bsky.social
```

### MCP Tools

All authentication commands are available as MCP tools:

**App Password:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "login",
    "arguments": {
      "handle": "alice.bsky.social",
      "password": "app-password-here"
    }
  }
}
```

**OAuth:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "oauth-login",
    "arguments": {
      "port": 8080
    }
  }
}
```

**Device Authorization:**
```json
{
  "method": "tools/call",
  "params": {
    "name": "device-login",
    "arguments": {}
  }
}
```

## Security

- **Token Storage**: Access and refresh tokens stored securely in OS keychain
- **DPoP Token Binding**: OAuth tokens bound to cryptographic keys, preventing token theft
- **PKCE**: Protects authorization code exchange from interception
- **No Password Storage**: Only tokens are stored (for app password method, password is not stored)
- **Token Lifetime**: 
  - Access tokens: 2 hours
  - Refresh tokens: 90 days
- **Automatic Refresh**: Token refresh supported for all authentication methods

## Testing

Run authentication tests:
```bash
cd go-server
go test ./internal/auth/... -v
```

Tests cover:
- Credential storage and retrieval
- PKCE challenge generation
- DPoP key generation and proof creation
- Token format validation

## Implementation Details

### Credential Storage Backends

Priority order (first available is used):
1. **macOS**: Keychain
2. **Windows**: Credential Manager
3. **Linux**: Secret Service (via D-Bus)
4. **Fallback**: Encrypted file in `~/.autoreply/`

### App Password Flow

1. User provides handle and app password
2. System calls `com.atproto.server.createSession` at `https://bsky.social`
3. Response includes access JWT, refresh JWT, DID
4. Credentials stored securely with handle as key
5. Handle set as default for subsequent operations

### OAuth Flow

1. Generate PKCE code verifier and challenge
2. Generate DPoP key pair
3. Construct authorization URL with challenge
4. Start local callback server
5. User authorizes in browser
6. Receive authorization code via callback
7. Exchange code for tokens with DPoP proof
8. Store credentials securely

### Device Authorization Flow

1. Generate DPoP key pair
2. Request device code from authorization server
3. Display verification URL and user code
4. Poll token endpoint for authorization
5. Handle authorization_pending and slow_down
6. Receive tokens once user authorizes
7. Store credentials securely

### Token Format

Stored credentials include:
- `handle`: User's BlueSky handle
- `access_token`: JWT for authenticated requests (2h lifetime)
- `refresh_token`: JWT for refreshing session (90d lifetime)
- `did`: User's decentralized identifier

## Future Enhancements

As outlined in `docs/12-auth-plan.md`:

1. **OAuth with PKCE**: More secure for interactive clients
2. **Device Authorization Grant**: Better for headless environments
3. **PDS Resolution**: Resolve user's actual PDS instead of assuming bsky.social
4. **Automatic Token Refresh**: Transparent refresh of expired tokens
5. **Token Revocation**: Server-side logout support

## References

- [AT Protocol Authentication](https://atproto.com/specs/xrpc)
- [BlueSky App Passwords](https://bsky.app/settings/app-passwords)
- [99designs/keyring](https://github.com/99designs/keyring)
