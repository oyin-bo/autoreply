use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

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

/// Config represents the authentication configuration
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
    
    /// Remove an account
    pub fn remove_account(&mut self, handle: &str) -> bool {
        if let Some(pos) = self.accounts.iter().position(|a| a.handle == handle) {
            self.accounts.remove(pos);
            true
        } else {
            false
        }
    }
    
    /// Update the last used timestamp for an account
    pub fn update_last_used(&mut self, handle: &str) {
        if let Some(account) = self.get_account_mut(handle) {
            account.last_used = SystemTime::now();
        }
    }
}
