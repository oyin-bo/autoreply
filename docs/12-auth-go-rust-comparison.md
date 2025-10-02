# Go vs Rust Authentication Implementation Comparison

This document provides a side-by-side comparison of authentication implementation approaches for Go and Rust, enabling parallel development while maintaining consistency.

---

## Library Equivalents

| Feature | Rust | Go | Notes |
|---------|------|-----|-------|
| **OS Keyring** | `keyring-rs` 2.3+ | `zalando/go-keyring` 0.2+ | Both use native OS APIs |
| **AT Protocol OAuth** | `atproto-oauth` | Copy from `haileyok/atproto-oauth-golang` | Rust has native crate, Go needs adaptation |
| **AT Protocol Client** | `atproto-client` | `bluesky-social/indigo` | Different API surfaces |
| **Encryption (fallback)** | `ring` | `crypto/aes` | For encrypted file storage |
| **Config Directories** | `dirs` crate | `os.UserConfigDir()` | Platform-specific paths |
| **JSON** | `serde_json` | `encoding/json` | Standard libraries |
| **HTTP Client** | `reqwest` | `net/http` | Both support TLS, proxies |

---

## Code Structure Comparison

### File Organization

**Rust:**
```
rust-server/src/
├── auth/
│   ├── mod.rs              # Public API
│   ├── credentials.rs      # Credential manager
│   ├── oauth.rs            # OAuth flows
│   ├── keyring.rs          # OS keyring wrapper
│   └── storage.rs          # File storage fallback
├── cli.rs                  # CLI commands (add auth)
└── mcp.rs                  # MCP tools (add auth)
```

**Go:**
```
go-server/internal/
├── auth/
│   ├── manager.go          # Credential manager
│   ├── oauth.go            # OAuth flows
│   ├── keyring.go          # OS keyring wrapper
│   └── storage.go          # File storage fallback
├── cli/                    # CLI commands (add auth)
│   └── auth.go
└── mcp/                    # MCP tools (add auth)
    └── auth_tools.go
```

---

## API Comparison

### Credential Manager

**Rust:**
```rust
pub struct CredentialManager {
    keyring_backend: KeyringBackend,
    file_backend: FileBackend,
    config: Config,
}

impl CredentialManager {
    pub fn new() -> Result<Self> { ... }
    
    pub async fn store_credentials(
        &mut self,
        handle: &str,
        access_token: &str,
        refresh_token: &str,
        dpop_key: &str,
    ) -> Result<()> { ... }
    
    pub async fn get_credentials(
        &self,
        handle: &str,
    ) -> Result<Credentials> { ... }
    
    pub async fn list_accounts(&self) -> Result<Vec<Account>> { ... }
    
    pub async fn delete_credentials(&mut self, handle: &str) -> Result<()> { ... }
}
```

**Go:**
```go
type CredentialManager struct {
    keyringBackend *KeyringBackend
    fileBackend    *FileBackend
    config         *Config
    mu             sync.RWMutex
}

func NewCredentialManager() (*CredentialManager, error) { ... }

func (cm *CredentialManager) StoreCredentials(
    ctx context.Context,
    handle string,
    accessToken string,
    refreshToken string,
    dpopKey string,
) error { ... }

func (cm *CredentialManager) GetCredentials(
    ctx context.Context,
    handle string,
) (*Credentials, error) { ... }

func (cm *CredentialManager) ListAccounts(
    ctx context.Context,
) ([]Account, error) { ... }

func (cm *CredentialManager) DeleteCredentials(
    ctx context.Context,
    handle string,
) error { ... }
```

---

## OAuth Flow Comparison

### PKCE Authorization Flow

**Rust:**
```rust
use atproto_oauth::{OAuthClient, Pkce};

pub struct OAuthFlow {
    client: OAuthClient,
}

impl OAuthFlow {
    pub async fn start_pkce_flow(&self, handle: &str) -> Result<PkceSession> {
        let pkce = Pkce::new();
        let auth_url = self.client.authorization_url(
            handle,
            &pkce.challenge,
            &pkce.method,
        ).await?;
        
        Ok(PkceSession {
            auth_url,
            verifier: pkce.verifier,
            state: generate_state(),
        })
    }
    
    pub async fn complete_pkce_flow(
        &self,
        code: &str,
        verifier: &str,
        state: &str,
    ) -> Result<Credentials> {
        let tokens = self.client.exchange_code(
            code,
            verifier,
        ).await?;
        
        Ok(Credentials::from_oauth_response(tokens))
    }
}
```

**Go:**
```go
type OAuthFlow struct {
    client *oauth.Client
}

func (of *OAuthFlow) StartPKCEFlow(
    ctx context.Context,
    handle string,
) (*PkceSession, error) {
    pkce := oauth.NewPKCE()
    authURL, err := of.client.AuthorizationURL(
        ctx,
        handle,
        pkce.Challenge,
        pkce.Method,
    )
    if err != nil {
        return nil, err
    }
    
    return &PkceSession{
        AuthURL:  authURL,
        Verifier: pkce.Verifier,
        State:    generateState(),
    }, nil
}

func (of *OAuthFlow) CompletePKCEFlow(
    ctx context.Context,
    code string,
    verifier string,
    state string,
) (*Credentials, error) {
    tokens, err := of.client.ExchangeCode(
        ctx,
        code,
        verifier,
    )
    if err != nil {
        return nil, err
    }
    
    return credentialsFromOAuthResponse(tokens), nil
}
```

---

## Keyring Integration Comparison

### Storing Credentials

**Rust:**
```rust
use keyring::Entry;

fn store_in_keyring(
    service: &str,
    account: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    let full_key = format!("{}/{}", account, key);
    let entry = Entry::new(service, &full_key)?;
    entry.set_password(value)?;
    Ok(())
}

fn get_from_keyring(
    service: &str,
    account: &str,
    key: &str,
) -> Result<String> {
    let full_key = format!("{}/{}", account, key);
    let entry = Entry::new(service, &full_key)?;
    let value = entry.get_password()?;
    Ok(value)
}
```

**Go:**
```go
import "github.com/zalando/go-keyring"

func storeInKeyring(
    service string,
    account string,
    key string,
    value string,
) error {
    fullKey := fmt.Sprintf("%s/%s", account, key)
    return keyring.Set(service, fullKey, value)
}

func getFromKeyring(
    service string,
    account string,
    key string,
) (string, error) {
    fullKey := fmt.Sprintf("%s/%s", account, key)
    return keyring.Get(service, fullKey)
}
```

---

## Encrypted File Storage Comparison

### Encryption/Decryption

**Rust:**
```rust
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};

fn encrypt_data(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let rng = SystemRandom::new();
    
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)?;
    let mut sealing_key = SealingKey::new(unbound_key, NonceSequence);
    
    let mut nonce_bytes = vec![0u8; 12];
    rng.fill(&mut nonce_bytes)?;
    
    let mut ciphertext = plaintext.to_vec();
    sealing_key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut ciphertext,
    )?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce_bytes;
    result.extend(ciphertext);
    Ok(result)
}
```

**Go:**
```go
import (
    "crypto/aes"
    "crypto/cipher"
    "crypto/rand"
)

func encryptData(key []byte, plaintext []byte) ([]byte, error) {
    block, err := aes.NewCipher(key)
    if err != nil {
        return nil, err
    }
    
    gcm, err := cipher.NewGCM(block)
    if err != nil {
        return nil, err
    }
    
    nonce := make([]byte, gcm.NonceSize())
    if _, err := rand.Read(nonce); err != nil {
        return nil, err
    }
    
    ciphertext := gcm.Seal(nil, nonce, plaintext, nil)
    
    // Prepend nonce to ciphertext
    result := append(nonce, ciphertext...)
    return result, nil
}
```

---

## CLI Command Comparison

### Login Command

**Rust:**
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "autoreply")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Login {
        #[clap(long)]
        method: Option<String>, // oauth, device, password
        
        #[clap(long)]
        handle: Option<String>,
    },
    Accounts,
    Use { handle: String },
    Logout { handle: String },
}

async fn handle_login(method: Option<String>, handle: Option<String>) -> Result<()> {
    let mut cm = CredentialManager::new()?;
    let oauth = OAuthFlow::new()?;
    
    let method = method.unwrap_or_else(|| prompt_method());
    let handle = handle.unwrap_or_else(|| prompt_handle());
    
    match method.as_str() {
        "oauth" => {
            let session = oauth.start_pkce_flow(&handle).await?;
            println!("Opening browser for authentication...");
            open::that(&session.auth_url)?;
            // Wait for callback...
        }
        "device" => {
            let device = oauth.start_device_flow(&handle).await?;
            println!("Visit: {}", device.verification_uri);
            println!("Enter code: {}", device.user_code);
            // Poll for completion...
        }
        "password" => {
            let password = prompt_password();
            let creds = oauth.authenticate_with_password(&handle, &password).await?;
            cm.store_credentials(&handle, &creds).await?;
        }
        _ => return Err("Invalid method".into()),
    }
    
    Ok(())
}
```

**Go:**
```go
import "github.com/spf13/cobra"

var loginCmd = &cobra.Command{
    Use:   "login",
    Short: "Authenticate with BlueSky",
    RunE:  runLogin,
}

func init() {
    loginCmd.Flags().String("method", "", "Authentication method (oauth, device, password)")
    loginCmd.Flags().String("handle", "", "BlueSky handle")
}

func runLogin(cmd *cobra.Command, args []string) error {
    ctx := cmd.Context()
    
    cm, err := NewCredentialManager()
    if err != nil {
        return err
    }
    
    oauth := NewOAuthFlow()
    
    method, _ := cmd.Flags().GetString("method")
    if method == "" {
        method = promptMethod()
    }
    
    handle, _ := cmd.Flags().GetString("handle")
    if handle == "" {
        handle = promptHandle()
    }
    
    switch method {
    case "oauth":
        session, err := oauth.StartPKCEFlow(ctx, handle)
        if err != nil {
            return err
        }
        fmt.Println("Opening browser for authentication...")
        browser.OpenURL(session.AuthURL)
        // Wait for callback...
        
    case "device":
        device, err := oauth.StartDeviceFlow(ctx, handle)
        if err != nil {
            return err
        }
        fmt.Printf("Visit: %s\n", device.VerificationURI)
        fmt.Printf("Enter code: %s\n", device.UserCode)
        // Poll for completion...
        
    case "password":
        password := promptPassword()
        creds, err := oauth.AuthenticateWithPassword(ctx, handle, password)
        if err != nil {
            return err
        }
        if err := cm.StoreCredentials(ctx, handle, creds); err != nil {
            return err
        }
        
    default:
        return fmt.Errorf("invalid method: %s", method)
    }
    
    return nil
}
```

---

## MCP Tool Comparison

### Login Tool

**Rust:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct LoginArgs {
    method: String,
    handle: String,
    password: Option<String>,
    callback_port: Option<u16>,
}

#[derive(Serialize)]
struct LoginResult {
    status: String,
    flow_type: Option<String>,
    auth_url: Option<String>,
    message: String,
}

async fn handle_login_tool(args: LoginArgs) -> Result<LoginResult> {
    let mut cm = CredentialManager::new()?;
    let oauth = OAuthFlow::new()?;
    
    match args.method.as_str() {
        "oauth" => {
            let session = oauth.start_pkce_flow(&args.handle).await?;
            Ok(LoginResult {
                status: "pending".to_string(),
                flow_type: Some("oauth".to_string()),
                auth_url: Some(session.auth_url),
                message: "Open this URL in your browser to complete authentication".to_string(),
            })
        }
        "password" => {
            let password = args.password.ok_or("password required")?;
            let creds = oauth.authenticate_with_password(&args.handle, &password).await?;
            cm.store_credentials(&args.handle, &creds).await?;
            Ok(LoginResult {
                status: "success".to_string(),
                flow_type: None,
                auth_url: None,
                message: format!("Successfully authenticated as @{}", args.handle),
            })
        }
        _ => Err("Invalid method".into()),
    }
}
```

**Go:**
```go
type LoginArgs struct {
    Method       string  `json:"method"`
    Handle       string  `json:"handle"`
    Password     *string `json:"password,omitempty"`
    CallbackPort *int    `json:"callback_port,omitempty"`
}

type LoginResult struct {
    Status    string  `json:"status"`
    FlowType  *string `json:"flow_type,omitempty"`
    AuthURL   *string `json:"auth_url,omitempty"`
    Message   string  `json:"message"`
}

func handleLoginTool(ctx context.Context, args LoginArgs) (*LoginResult, error) {
    cm, err := NewCredentialManager()
    if err != nil {
        return nil, err
    }
    
    oauth := NewOAuthFlow()
    
    switch args.Method {
    case "oauth":
        session, err := oauth.StartPKCEFlow(ctx, args.Handle)
        if err != nil {
            return nil, err
        }
        flowType := "oauth"
        return &LoginResult{
            Status:   "pending",
            FlowType: &flowType,
            AuthURL:  &session.AuthURL,
            Message:  "Open this URL in your browser to complete authentication",
        }, nil
        
    case "password":
        if args.Password == nil {
            return nil, errors.New("password required")
        }
        creds, err := oauth.AuthenticateWithPassword(ctx, args.Handle, *args.Password)
        if err != nil {
            return nil, err
        }
        if err := cm.StoreCredentials(ctx, args.Handle, creds); err != nil {
            return nil, err
        }
        return &LoginResult{
            Status:  "success",
            Message: fmt.Sprintf("Successfully authenticated as @%s", args.Handle),
        }, nil
        
    default:
        return nil, fmt.Errorf("invalid method: %s", args.Method)
    }
}
```

---

## Testing Strategy Comparison

### Unit Tests

**Rust:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_store_and_retrieve_credentials() {
        let mut cm = CredentialManager::new().unwrap();
        
        cm.store_credentials(
            "alice.bsky.social",
            "access_token_123",
            "refresh_token_456",
            "dpop_key_789",
        ).await.unwrap();
        
        let creds = cm.get_credentials("alice.bsky.social").await.unwrap();
        assert_eq!(creds.access_token, "access_token_123");
        assert_eq!(creds.refresh_token, "refresh_token_456");
    }
    
    #[tokio::test]
    async fn test_multiple_accounts() {
        let mut cm = CredentialManager::new().unwrap();
        
        cm.store_credentials("alice.bsky.social", "token1", "refresh1", "key1").await.unwrap();
        cm.store_credentials("bob.bsky.social", "token2", "refresh2", "key2").await.unwrap();
        
        let accounts = cm.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 2);
    }
}
```

**Go:**
```go
func TestStoreAndRetrieveCredentials(t *testing.T) {
    cm, err := NewCredentialManager()
    require.NoError(t, err)
    
    ctx := context.Background()
    
    err = cm.StoreCredentials(
        ctx,
        "alice.bsky.social",
        "access_token_123",
        "refresh_token_456",
        "dpop_key_789",
    )
    require.NoError(t, err)
    
    creds, err := cm.GetCredentials(ctx, "alice.bsky.social")
    require.NoError(t, err)
    assert.Equal(t, "access_token_123", creds.AccessToken)
    assert.Equal(t, "refresh_token_456", creds.RefreshToken)
}

func TestMultipleAccounts(t *testing.T) {
    cm, err := NewCredentialManager()
    require.NoError(t, err)
    
    ctx := context.Background()
    
    err = cm.StoreCredentials(ctx, "alice.bsky.social", "token1", "refresh1", "key1")
    require.NoError(t, err)
    
    err = cm.StoreCredentials(ctx, "bob.bsky.social", "token2", "refresh2", "key2")
    require.NoError(t, err)
    
    accounts, err := cm.ListAccounts(ctx)
    require.NoError(t, err)
    assert.Len(t, accounts, 2)
}
```

---

## Error Handling Comparison

**Rust:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("keyring unavailable: {0}")]
    KeyringUnavailable(String),
    
    #[error("invalid credentials")]
    InvalidCredentials,
    
    #[error("OAuth flow failed: {0}")]
    OAuthFailed(String),
    
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("storage error: {0}")]
    StorageError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AuthError>;
```

**Go:**
```go
type AuthError struct {
    Code    string
    Message string
    Err     error
}

func (e *AuthError) Error() string {
    if e.Err != nil {
        return fmt.Sprintf("%s: %s: %v", e.Code, e.Message, e.Err)
    }
    return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

func (e *AuthError) Unwrap() error {
    return e.Err
}

var (
    ErrKeyringUnavailable = &AuthError{Code: "keyring_unavailable", Message: "OS keyring is not available"}
    ErrInvalidCredentials = &AuthError{Code: "invalid_credentials", Message: "Invalid credentials provided"}
    ErrOAuthFailed        = &AuthError{Code: "oauth_failed", Message: "OAuth flow failed"}
    ErrNetworkError       = &AuthError{Code: "network_error", Message: "Network error occurred"}
    ErrStorageError       = &AuthError{Code: "storage_error", Message: "Storage operation failed"}
)
```

---

## Concurrency Patterns

**Rust:**
```rust
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct CredentialManager {
    keyring: Arc<RwLock<KeyringBackend>>,
    config: Arc<RwLock<Config>>,
}

impl CredentialManager {
    pub async fn refresh_token(&self, handle: &str) -> Result<()> {
        // Read lock for checking
        let config = self.config.read().await;
        let account = config.get_account(handle)?;
        
        if !account.needs_refresh() {
            return Ok(());
        }
        drop(config);
        
        // Write lock for updating
        let mut config = self.config.write().await;
        let mut keyring = self.keyring.write().await;
        
        // Double-check after acquiring write lock
        let account = config.get_account(handle)?;
        if !account.needs_refresh() {
            return Ok(());
        }
        
        // Perform refresh
        let new_tokens = self.oauth.refresh_access_token(&account.refresh_token).await?;
        keyring.store(handle, "access_token", &new_tokens.access_token)?;
        config.update_expiry(handle, new_tokens.expires_at)?;
        
        Ok(())
    }
}
```

**Go:**
```go
type CredentialManager struct {
    keyring *KeyringBackend
    config  *Config
    mu      sync.RWMutex
}

func (cm *CredentialManager) RefreshToken(ctx context.Context, handle string) error {
    // Read lock for checking
    cm.mu.RLock()
    account := cm.config.GetAccount(handle)
    needsRefresh := account.NeedsRefresh()
    cm.mu.RUnlock()
    
    if !needsRefresh {
        return nil
    }
    
    // Write lock for updating
    cm.mu.Lock()
    defer cm.mu.Unlock()
    
    // Double-check after acquiring write lock
    account = cm.config.GetAccount(handle)
    if !account.NeedsRefresh() {
        return nil
    }
    
    // Perform refresh
    newTokens, err := cm.oauth.RefreshAccessToken(ctx, account.RefreshToken)
    if err != nil {
        return err
    }
    
    if err := cm.keyring.Store(handle, "access_token", newTokens.AccessToken); err != nil {
        return err
    }
    
    if err := cm.config.UpdateExpiry(handle, newTokens.ExpiresAt); err != nil {
        return err
    }
    
    return nil
}
```

---

## Configuration Loading

**Rust:**
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub accounts: Vec<Account>,
    pub default_account: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        
        if !path.exists() {
            return Ok(Self::default());
        }
        
        let data = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&data)?;
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
    
    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or("Cannot determine config directory")?;
        Ok(config_dir.join("autoreply-mcp").join("config.json"))
    }
}
```

**Go:**
```go
type Config struct {
    Version        string    `json:"version"`
    Accounts       []Account `json:"accounts"`
    DefaultAccount *string   `json:"default_account,omitempty"`
}

func LoadConfig() (*Config, error) {
    path, err := configPath()
    if err != nil {
        return nil, err
    }
    
    if _, err := os.Stat(path); os.IsNotExist(err) {
        return defaultConfig(), nil
    }
    
    data, err := os.ReadFile(path)
    if err != nil {
        return nil, err
    }
    
    var config Config
    if err := json.Unmarshal(data, &config); err != nil {
        return nil, err
    }
    
    return &config, nil
}

func (c *Config) Save() error {
    path, err := configPath()
    if err != nil {
        return err
    }
    
    dir := filepath.Dir(path)
    if err := os.MkdirAll(dir, 0700); err != nil {
        return err
    }
    
    data, err := json.MarshalIndent(c, "", "  ")
    if err != nil {
        return err
    }
    
    return os.WriteFile(path, data, 0600)
}

func configPath() (string, error) {
    configDir, err := os.UserConfigDir()
    if err != nil {
        return "", err
    }
    return filepath.Join(configDir, "autoreply-mcp", "config.json"), nil
}
```

---

## Key Takeaways for Parallel Development

### Maintain Consistency
1. **Config file format:** Use identical JSON structure
2. **Keyring organization:** Same service name and key patterns
3. **MCP tool schemas:** Identical parameter and return types
4. **Error codes:** Use same error code strings
5. **File locations:** Same config directory structure

### Platform Compatibility
- Both implementations must work on macOS, Windows, and Linux
- Test keyring availability and fallback on all platforms
- Verify file permissions (0600) are set correctly

### Testing Coordination
- Share test credentials and fixtures
- Use same OAuth mock server for integration tests
- Coordinate on error message formats

### Documentation
- Keep CLI help text consistent between implementations
- Use same examples in both READMEs
- Cross-reference between implementation guides

---

## Development Workflow

### Initial Setup (Weeks 1-2)
1. Both teams implement credential storage independently
2. Share config file format specification
3. Test keyring integration on target platforms

### OAuth Implementation (Weeks 3-4)
1. Rust uses `atproto-oauth` crate directly
2. Go adapts code from reference implementation
3. Both teams test against same OAuth server
4. Share findings on DPoP and PKCE quirks

### CLI Commands (Week 5)
1. Implement same command structure
2. Test interoperability (Go CLI with Rust MCP, vice versa)
3. Ensure consistent user experience

### MCP Tools (Week 6)
1. Implement identical tool schemas
2. Test cross-implementation compatibility
3. Verify error handling matches

### Testing (Weeks 7-8)
1. Run same test suite on both implementations
2. Compare results and fix discrepancies
3. Document platform-specific issues

### Polish (Week 9)
1. Align documentation
2. Create unified migration guide
3. Record demo videos showing both implementations
