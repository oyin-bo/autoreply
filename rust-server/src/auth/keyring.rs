use super::types::Credentials;
use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "autoreply-mcp";

/// KeyringBackend provides OS keyring storage for credentials
#[allow(dead_code)] // Used in external modules
pub struct KeyringBackend {
    service_name: String,
}

#[allow(dead_code)] // Methods used in external modules
impl KeyringBackend {
    /// Create a new keyring backend
    pub fn new() -> Self {
        Self {
            service_name: SERVICE_NAME.to_string(),
        }
    }
    
    /// Store a value in the keyring
    pub fn store(&self, account: &str, key: &str, value: &str) -> Result<()> {
        let full_key = format!("{}/{}", account, key);
        let entry = Entry::new(&self.service_name, &full_key)?;
        entry.set_password(value)
            .context("Failed to set password in keyring")?;
        Ok(())
    }
    
    /// Get a value from the keyring
    pub fn get(&self, account: &str, key: &str) -> Result<String> {
        let full_key = format!("{}/{}", account, key);
        let entry = Entry::new(&self.service_name, &full_key)?;
        entry.get_password()
            .context("Failed to get password from keyring")
    }
    
    /// Delete a value from the keyring
    pub fn delete(&self, account: &str, key: &str) -> Result<()> {
        let full_key = format!("{}/{}", account, key);
        let entry = Entry::new(&self.service_name, &full_key)?;
        entry.delete_password()
            .context("Failed to delete password from keyring")
    }
    
    /// Store all credentials for an account in the keyring
    pub fn store_credentials(&self, account: &str, creds: &Credentials) -> Result<()> {
        self.store(account, "access_token", &creds.access_token)
            .context("Failed to store access token")?;
        
        self.store(account, "refresh_token", &creds.refresh_token)
            .context("Failed to store refresh token")?;
        
        self.store(account, "dpop_key", &creds.dpop_key)
            .context("Failed to store DPoP key")?;
        
        Ok(())
    }
    
    /// Get all credentials for an account from the keyring
    pub fn get_credentials(&self, account: &str) -> Result<Credentials> {
        let access_token = self.get(account, "access_token")
            .context("Failed to get access token")?;
        
        let refresh_token = self.get(account, "refresh_token")
            .context("Failed to get refresh token")?;
        
        let dpop_key = self.get(account, "dpop_key")
            .context("Failed to get DPoP key")?;
        
        // Note: We don't store expires_at in keyring, it's in the config file
        // For now, use a placeholder - this will be properly handled when we integrate with config
        let expires_at = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        
        Ok(Credentials {
            access_token,
            refresh_token,
            dpop_key,
            expires_at,
        })
    }
    
    /// Delete all credentials for an account from the keyring
    pub fn delete_credentials(&self, account: &str) -> Result<()> {
        // Try to delete all keys, collecting errors
        let mut errors = Vec::new();
        
        if let Err(e) = self.delete(account, "access_token") {
            errors.push(format!("access_token: {}", e));
        }
        
        if let Err(e) = self.delete(account, "refresh_token") {
            errors.push(format!("refresh_token: {}", e));
        }
        
        if let Err(e) = self.delete(account, "dpop_key") {
            errors.push(format!("dpop_key: {}", e));
        }
        
        if !errors.is_empty() {
            anyhow::bail!("Failed to delete some credentials: {}", errors.join(", "));
        }
        
        Ok(())
    }
    
    /// Check if the OS keyring is available
    pub fn is_available(&self) -> bool {
        let test_key = "_test_availability";
        let test_value = "test";
        
        // Try to perform a test operation
        let entry = match Entry::new(&self.service_name, test_key) {
            Ok(e) => e,
            Err(_) => return false,
        };
        
        if entry.set_password(test_value).is_err() {
            return false;
        }
        
        // Clean up test entry
        let _ = entry.delete_password();
        true
    }
}

impl Default for KeyringBackend {
    fn default() -> Self {
        Self::new()
    }
}
