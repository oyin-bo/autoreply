# Authentication in Go Server

The Go server implements app password authentication for BlueSky AT Protocol as described in `docs/12-auth-plan.md`.

## Overview

The authentication system provides:
- **App Password Authentication**: Uses `com.atproto.server.createSession` XRPC endpoint
- **Secure Credential Storage**: OS-native keychains with encrypted file fallback
- **Multi-Account Support**: Store and manage multiple authenticated accounts
- **MCP Integration**: Available as both MCP tools and CLI commands

## Architecture

### Components

1. **Credential Store** (`internal/auth/credentials.go`):
   - Manages secure storage using `99designs/keyring`
   - Supports OS keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service)
   - Falls back to encrypted file storage when keyring is unavailable
   - Stores credentials per handle with default handle management

2. **Session Manager** (`internal/auth/session.go`):
   - Handles AT Protocol authentication flows
   - Creates sessions using `com.atproto.server.createSession`
   - Supports token refresh using `com.atproto.server.refreshSession`

3. **MCP Tools**:
   - `LoginTool` (`internal/tools/login.go`): Authenticate with handle and app password
   - `LogoutTool` (`internal/tools/logout.go`): Remove stored credentials
   - `AccountsTool` (`internal/tools/accounts.go`): List accounts and manage default

## Usage

### CLI Commands

#### Login
```bash
autoreply login --handle alice.bsky.social --password <app-password>
```

App passwords can be generated in BlueSky Settings → App Passwords.

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

## Security

- **Token Storage**: Access and refresh tokens stored securely in OS keychain
- **No Password Storage**: Only tokens are stored, not the original password
- **Token Lifetime**: 
  - Access tokens: 2 hours
  - Refresh tokens: 90 days
- **Automatic Refresh**: Token refresh can be implemented when access token expires

## Testing

Run authentication tests:
```bash
cd go-server
go test ./internal/auth/... -v
```

## Implementation Details

### Credential Storage Backends

Priority order (first available is used):
1. **macOS**: Keychain
2. **Windows**: Credential Manager
3. **Linux**: Secret Service (via D-Bus)
4. **Fallback**: Encrypted file in `~/.autoreply/`

### Session Flow

1. User provides handle and app password
2. System calls `com.atproto.server.createSession` at `https://bsky.social`
3. Response includes access JWT, refresh JWT, DID
4. Credentials stored securely with handle as key
5. Handle set as default for subsequent operations

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
