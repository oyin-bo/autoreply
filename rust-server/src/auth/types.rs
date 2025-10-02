use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime};

/// Authentication errors
#[derive(Debug)]
#[allow(dead_code)] // Some variants for future use
pub enum AuthError {
    /// Account not found
    AccountNotFound(String),
    /// Invalid credentials
    InvalidCredentials(String),
    /// Keyring unavailable
    KeyringUnavailable(String),
    /// Configuration error
    ConfigError(String),
    /// Network error
    NetworkError(String),
    /// OAuth error
    OAuthError(String),
    /// Parse error
    ParseError(String),
    /// Authorization pending (device flow)
    AuthorizationPending,
    /// Slow down (polling too fast)
    SlowDown,
    /// Token expired
    ExpiredToken,
    /// Access denied by user
    AccessDenied,
    /// IO error
    IoError(std::io::Error),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::AccountNotFound(msg) => write!(f, "Account not found: {}", msg),
            AuthError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
            AuthError::KeyringUnavailable(msg) => write!(f, "Keyring unavailable: {}", msg),
            AuthError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            AuthError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AuthError::OAuthError(msg) => write!(f, "OAuth error: {}", msg),
            AuthError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AuthError::AuthorizationPending => write!(f, "Authorization pending"),
            AuthError::SlowDown => write!(f, "Slow down polling"),
            AuthError::ExpiredToken => write!(f, "Token expired"),
            AuthError::AccessDenied => write!(f, "Access denied"),
            AuthError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<std::io::Error> for AuthError {
    fn from(error: std::io::Error) -> Self {
        AuthError::IoError(error)
    }
}

impl From<keyring::Error> for AuthError {
    fn from(error: keyring::Error) -> Self {
        AuthError::KeyringUnavailable(error.to_string())
    }
}

/// Credentials represents stored authentication credentials for a BlueSky account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub dpop_key: String,
    pub expires_at: SystemTime,
}

impl Credentials {
    /// Check if credentials need to be refreshed
    #[allow(dead_code)] // Used in token lifecycle management
    pub fn needs_refresh(&self, threshold_minutes: u64) -> bool {
        let threshold = Duration::from_secs(threshold_minutes * 60);
        match self.expires_at.duration_since(SystemTime::now()) {
            Ok(remaining) => remaining < threshold,
            Err(_) => true, // Already expired
        }
    }
}

/// Account represents metadata for an authenticated account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub handle: String,
    pub did: String,
    pub pds: String,
    pub storage_ref: String, // "keyring", "encrypted", or "plaintext"
    pub created_at: SystemTime,
    pub last_used: SystemTime,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Configuration for authentication behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub auto_refresh: bool,
    pub refresh_threshold_minutes: u64,
    pub token_rotation_days: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            refresh_threshold_minutes: 5,
            token_rotation_days: 30,
        }
    }
}

/// Configuration for the authentication system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub accounts: Vec<Account>,
    pub default_account: Option<String>,
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            accounts: Vec::new(),
            default_account: None,
            settings: Settings::default(),
        }
    }
}

impl Config {
    /// Get an account by handle
    pub fn get_account(&self, handle: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.handle == handle)
    }

    /// Get a mutable reference to an account by handle
    pub fn get_account_mut(&mut self, handle: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.handle == handle)
    }

    /// Add or update an account
    pub fn add_account(&mut self, account: Account) {
        if let Some(existing) = self.get_account_mut(&account.handle) {
            *existing = account;
        } else {
            self.accounts.push(account);
        }
    }

    /// Remove an account by handle
    pub fn remove_account(&mut self, handle: &str) -> bool {
        let initial_len = self.accounts.len();
        self.accounts.retain(|a| a.handle != handle);
        self.accounts.len() < initial_len
    }

    /// Update the last used timestamp for an account
    pub fn update_last_used(&mut self, handle: &str) {
        if let Some(account) = self.get_account_mut(handle) {
            account.last_used = SystemTime::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_needs_refresh_expired() {
        let creds = Credentials {
            access_token: "test_token".to_string(),
            refresh_token: "test_refresh".to_string(),
            dpop_key: "test_key".to_string(),
            expires_at: SystemTime::now() - Duration::from_secs(300),
        };
        assert!(creds.needs_refresh(5));
    }

    #[test]
    fn test_credentials_needs_refresh_within_threshold() {
        let creds = Credentials {
            access_token: "test_token".to_string(),
            refresh_token: "test_refresh".to_string(),
            dpop_key: "test_key".to_string(),
            expires_at: SystemTime::now() + Duration::from_secs(180),
        };
        assert!(creds.needs_refresh(5));
    }

    #[test]
    fn test_credentials_needs_refresh_outside_threshold() {
        let creds = Credentials {
            access_token: "test_token".to_string(),
            refresh_token: "test_refresh".to_string(),
            dpop_key: "test_key".to_string(),
            expires_at: SystemTime::now() + Duration::from_secs(600),
        };
        assert!(!creds.needs_refresh(5));
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.version, "2.0");
        assert_eq!(config.accounts.len(), 0);
        assert!(config.default_account.is_none());
        assert!(config.settings.auto_refresh);
        assert_eq!(config.settings.refresh_threshold_minutes, 5);
    }

    #[test]
    fn test_config_get_account() {
        let mut config = Config::default();
        assert!(config.get_account("test.bsky.social").is_none());
        
        let account = Account {
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test123".to_string(),
            pds: "https://bsky.social".to_string(),
            storage_ref: "keyring".to_string(),
            created_at: SystemTime::now(),
            last_used: SystemTime::now(),
            metadata: HashMap::new(),
        };
        config.add_account(account);
        
        let found = config.get_account("test.bsky.social");
        assert!(found.is_some());
        assert_eq!(found.unwrap().handle, "test.bsky.social");
    }

    #[test]
    fn test_config_add_account() {
        let mut config = Config::default();
        
        let account = Account {
            handle: "alice.bsky.social".to_string(),
            did: "did:plc:alice123".to_string(),
            pds: "https://bsky.social".to_string(),
            storage_ref: "keyring".to_string(),
            created_at: SystemTime::now(),
            last_used: SystemTime::now(),
            metadata: HashMap::new(),
        };
        
        config.add_account(account.clone());
        assert_eq!(config.accounts.len(), 1);
        
        let mut updated = account.clone();
        updated.did = "did:plc:alice456".to_string();
        config.add_account(updated);
        
        assert_eq!(config.accounts.len(), 1);
        let found = config.get_account("alice.bsky.social");
        assert_eq!(found.unwrap().did, "did:plc:alice456");
    }

    #[test]
    fn test_config_remove_account() {
        let mut config = Config::default();
        
        let account = Account {
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test123".to_string(),
            pds: "https://bsky.social".to_string(),
            storage_ref: "keyring".to_string(),
            created_at: SystemTime::now(),
            last_used: SystemTime::now(),
            metadata: HashMap::new(),
        };
        config.add_account(account);
        
        assert!(config.remove_account("test.bsky.social"));
        assert_eq!(config.accounts.len(), 0);
        assert!(!config.remove_account("nonexistent.bsky.social"));
    }

    #[test]
    fn test_config_update_last_used() {
        let mut config = Config::default();
        
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        let account = Account {
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test123".to_string(),
            pds: "https://bsky.social".to_string(),
            storage_ref: "keyring".to_string(),
            created_at: old_time,
            last_used: old_time,
            metadata: HashMap::new(),
        };
        config.add_account(account);
        
        config.update_last_used("test.bsky.social");
        
        let updated = config.get_account("test.bsky.social").unwrap();
        assert!(updated.last_used > old_time);
    }
}
