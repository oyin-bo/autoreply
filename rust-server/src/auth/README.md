# Authentication Module

This module provides authentication support for the BlueSky AT Protocol.

## Features

- **App Password Authentication**: Uses `com.atproto.server.createSession` XRPC endpoint
- **Credential Storage**: OS keyring (primary) with file fallback
- **Token Management**: Automatic token refresh and expiry checking
- **Multi-Account Support**: Store and manage multiple accounts with default selection

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

### Login

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

### Logout

Remove stored credentials:

```bash
# Logout from default account
autoreply logout

# Logout from specific account
autoreply logout --handle alice.bsky.social
```

### Manage Accounts

List and manage authenticated accounts:

```bash
# List all authenticated accounts
autoreply accounts list

# Set default account
autoreply accounts default alice.bsky.social
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

## Future Enhancements

Planned features for future releases:

- OAuth 2.0 with DPoP and PKCE support
- Device Authorization Grant for headless environments
- Token rotation and automatic session management
- MCP tool for authentication in server mode
- Encrypted file storage option
