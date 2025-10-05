# Authentication Module

This module provides comprehensive authentication support for the BlueSky AT Protocol.

## Features

- **OAuth Browser Flow**: Interactive OAuth with automatic browser redirect and PKCE (✅ fully implemented)
- **App Password Authentication**: Uses `com.atproto.server.createSession` XRPC endpoint (✅ fully implemented)
- **Credential Storage**: OS keyring (primary) with file fallback (✅ fully implemented)
- **Token Management**: Automatic token refresh and expiry checking (✅ fully implemented)
- **Multi-Account Support**: Store and manage multiple accounts with default selection (✅ fully implemented)

## Storage Backends

The module automatically selects the best available storage backend:

1. **OS Keyring** (preferred): Uses native secure storage
   - macOS: Keychain
   - Windows: Credential Manager
   - Linux: Secret Service API

2. **File Storage** (fallback): JSON file in user config directory
   - Location: `~/.config/autoreply/credentials.json` (Linux/macOS)
   - Permissions: Set to 0600 (user-only read/write)

## CLI Usage

### Login - OAuth Browser Flow (Default & Recommended)

For interactive OAuth with automatic browser opening:

```bash
# OAuth browser flow (default - fully functional!)
autoreply login --handle alice.bsky.social
```

The CLI will:
1. Start a local callback server on a random port
2. Open your default browser to the authorization page
3. Wait for you to approve (5-minute timeout)
4. Automatically receive the callback and exchange for tokens
5. Store tokens securely

**Example:**
```
Starting OAuth callback server on http://localhost:54321/callback
Opened browser for authorization. Waiting for callback...

[Browser opens to BlueSky authorization page]
[You click "Authorize"]
[Browser shows: "Authorization Successful! You can close this window."]

Received authorization code, exchanging for tokens
✓ Successfully authenticated as @alice.bsky.social
  DID: did:plc:abc123...
  Method: OAuth (browser)
  Storage: OS keyring
```

**Advantages:**
- Most user-friendly (one-click authorization)
- Most secure OAuth flow (PKCE + state validation)
- Works on desktop environments
- Tokens revocable per-application

**Security:**
- Uses PKCE (Proof Key for Code Exchange) S256 method
- State parameter for CSRF protection
- Localhost-only callback server
- 5-minute authorization timeout

### Login - App Password (Traditional)

Authenticate with your BlueSky account using an app password:

```bash
# Interactive login (prompts for credentials)
autoreply login

# Specify credentials via flags
autoreply login --handle alice.bsky.social --password app-password-here

# Use custom service URL
autoreply login --handle alice.bsky.social --service https://custom.pds.example
```

**Note**: Create app passwords in BlueSky Settings → App Passwords

### Account Management

List, switch, and delete authenticated accounts:

```bash
# List all authenticated accounts
autoreply login list

# Set default account
autoreply login default alice.bsky.social

# Delete specific account
autoreply login delete --handle alice.bsky.social

# Delete default account
autoreply login delete
```

## Programmatic Usage

### Basic Authentication

```rust
use autoreply::auth::{Credentials, SessionManager, CredentialStorage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create credentials
    let credentials = Credentials::new("alice.bsky.social", "app-password-here");
    
    // Authenticate
    let manager = SessionManager::new()?;
    let session = manager.login(&credentials).await?;
    
    println!("Authenticated as: {}", session.handle);
    println!("DID: {}", session.did);
    
    // Store credentials for later use
    let storage = CredentialStorage::new()?;
    storage.add_account(&session.handle, credentials)?;
    storage.store_session(&session.handle, session)?;
    
    Ok(())
}
```

### Token Refresh

```rust
use autoreply::auth::{SessionManager, CredentialStorage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = CredentialStorage::new()?;
    let manager = SessionManager::new()?;
    
    // Get stored session
    let session = storage.get_session("alice.bsky.social")?
        .ok_or("No session found")?;
    
    // Get valid session (auto-refreshes if needed)
    let valid_session = manager.get_valid_session(&session).await?;
    
    // Use the access token
    println!("Access token: {}", valid_session.access_jwt);
    
    Ok(())
}
```

### Making Authenticated Requests

```rust
use autoreply::auth::{SessionManager, Credentials};
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Authenticate
    let credentials = Credentials::new("alice.bsky.social", "password");
    let manager = SessionManager::new()?;
    let session = manager.login(&credentials).await?;
    
    // Make authenticated request
    let client = Client::new();
    let response = client
        .get("https://bsky.social/xrpc/app.bsky.actor.getProfile")
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .query(&[("actor", &session.handle)])
        .send()
        .await?;
    
    println!("Response: {:?}", response.text().await?);
    
    Ok(())
}
```

## Security Considerations

- **Never log tokens or passwords**: All token values are excluded from logs
- **File permissions**: File storage sets strict permissions (0600)
- **Token expiry**: Access tokens expire after 2 hours, use refresh tokens
- **Secure transport**: All API calls use HTTPS/TLS
- **App passwords**: Always use app-specific passwords, never your main account password

## Token Lifecycle

1. **Login**: Creates access token (2h lifetime) and refresh token (90d lifetime)
2. **Check Expiry**: Before use, check if token expires within 5 minutes
3. **Refresh**: If expiring soon, use refresh token to get new tokens
4. **Store**: Update stored session with new tokens

Example expiry check:

```rust
if session.is_expired() {
    // Token is expired or will expire within 5 minutes
    let refreshed = manager.refresh(&session).await?;
    storage.store_session(&handle, refreshed)?;
}
```

## API Reference

### Credentials

```rust
pub struct Credentials {
    pub identifier: String,  // Handle or DID
    pub password: String,     // App password
    pub service: String,      // Service URL (default: https://bsky.social)
}

impl Credentials {
    pub fn new(identifier: impl Into<String>, password: impl Into<String>) -> Self;
    pub fn with_service(identifier: impl Into<String>, password: impl Into<String>, service: impl Into<String>) -> Self;
}
```

### Session

```rust
pub struct Session {
    pub access_jwt: String,   // Access token
    pub refresh_jwt: String,  // Refresh token
    pub handle: String,       // User handle
    pub did: String,          // User DID
    pub service: String,      // Service URL
    pub expires_at: Option<DateTime<Utc>>,
}

impl Session {
    pub fn is_expired(&self) -> bool;
}
```

### SessionManager

```rust
pub struct SessionManager { /* ... */ }

impl SessionManager {
    pub fn new() -> Result<Self, AppError>;
    pub async fn login(&self, credentials: &Credentials) -> Result<Session, AppError>;
    pub async fn refresh(&self, session: &Session) -> Result<Session, AppError>;
    pub async fn get_valid_session(&self, session: &Session) -> Result<Session, AppError>;
}
```

### CredentialStorage

```rust
pub struct CredentialStorage { /* ... */ }

impl CredentialStorage {
    pub fn new() -> Result<Self, AppError>;
    pub fn store_credentials(&self, handle: &str, credentials: Credentials) -> Result<(), AppError>;
    pub fn get_credentials(&self, handle: &str) -> Result<Credentials, AppError>;
    pub fn store_session(&self, handle: &str, session: Session) -> Result<(), AppError>;
    pub fn get_session(&self, handle: &str) -> Result<Option<Session>, AppError>;
    pub fn delete_credentials(&self, handle: &str) -> Result<(), AppError>;
    pub fn list_accounts(&self) -> Result<Vec<String>, AppError>;
    pub fn add_account(&self, handle: &str, credentials: Credentials) -> Result<(), AppError>;
    pub fn remove_account(&self, handle: &str) -> Result<(), AppError>;
    pub fn get_default_account(&self) -> Result<Option<String>, AppError>;
    pub fn set_default_account(&self, handle: &str) -> Result<(), AppError>;
    pub fn backend(&self) -> StorageBackend;
}
```

## Testing

The module includes comprehensive unit tests:

```bash
# Run all auth tests
cargo test auth::

# Run specific test module
cargo test auth::credentials::
cargo test auth::session::
cargo test auth::storage::
```

## Implementation Status

### ✅ Fully Implemented
- App password authentication
- OAuth Browser Flow with PKCE and callback server
- Secure credential storage (OS keyring + file fallback)
- Multi-account management
- Token management and refresh
- PKCE S256 code challenge generation
- Local HTTP callback server (Axum-based)
- State parameter validation (CSRF protection)
- Automatic browser opening
- User-friendly authorization pages
- **MCP elicitation support** for interactive credential prompts (v0.3.2+)

### 📋 Planned for Future Releases
- DPoP token binding (advanced security feature)
- Token rotation and automatic session management

## MCP Integration (v0.3.2+)

### Elicitation Support

The authentication module integrates with the MCP server's elicitation capability to provide interactive credential collection when running in MCP mode.

**How it works:**

1. **Client Capability Detection**: During MCP initialization, the server detects if the client supports the `elicitation` capability
2. **Interactive Prompts**: When credentials are missing, the server sends `elicitation/create` requests to the client
3. **User Response**: The client presents the prompt to the user and returns their input
4. **Seamless Flow**: The authentication proceeds with the provided credentials

**Example Flow:**

```
MCP Client → Server: tools/call login (no handle provided)
Server → Client: elicitation/create { message: "Please provide your BlueSky handle", schema: {...} }
Client → User: [Interactive prompt]
User → Client: "alice.bsky.social"
Client → Server: elicitation response { action: "accept", content: { handle: "alice.bsky.social" } }
Server: Continues authentication with handle
```

**Supported Prompts:**

- **Handle prompt**: When `--handle` is not provided
  - Message: "Please provide your BlueSky handle"
  - Schema: `{ type: "object", properties: { handle: { type: "string" } } }`

- **Password prompt**: When `--password` is not provided (and OAuth is not available)
  - Message: Detailed instructions with link to create app password + OAuth suggestion
  - Schema: `{ type: "object", properties: { password: { type: "string" } } }`
  - User can decline to switch to OAuth method

**Fallback Behavior:**

If the MCP client doesn't support elicitation, the `login` tool returns a detailed error message with manual instructions:

```markdown
# Login requires handle - but **ClientName does not support interactive prompts** (MCP elicitation)

To complete login, please:
1. Create an app password at: https://bsky.app/settings/app-passwords
2. Retry with: login(handle="your.handle", password="app-password")

Or use OAuth in CLI mode:
autoreply login --handle your.handle
```

This ensures a good experience whether or not the client supports elicitation.

### Supported MCP Clients

Known clients with elicitation support:
- Gemini CLI (recent versions)
- Claude Desktop (with MCP SDK updates)
- Custom MCP clients implementing the elicitation protocol

### Testing Elicitation

The module includes comprehensive tests for elicitation behavior:

```bash
# Run MCP-specific tests
cargo test mcp::tests::

# Test elicitation handling
cargo test test_request_elicitation
cargo test test_elicitation_response
```
- MCP tool for authentication in server mode
- Encrypted file storage option
