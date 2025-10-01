use super::config::{load_config, save_config};
use super::keyring::KeyringBackend;
use super::types::{Account, Config, Credentials};
use anyhow::{Context, Result};
use std::sync::RwLock;
use std::time::SystemTime;

/// CredentialManager manages authentication credentials for multiple accounts
pub struct CredentialManager {
    keyring: KeyringBackend,
    config: RwLock<Config>,
}

impl CredentialManager {
    /// Create a new credential manager
    pub fn new() -> Result<Self> {
        let config = load_config()
            .context("Failed to load config")?;
        
        let keyring = KeyringBackend::new();
        
        Ok(Self {
            keyring,
            config: RwLock::new(config),
        })
    }
    
    /// Store credentials for an account
    pub fn store_credentials(&self, handle: &str, creds: &Credentials) -> Result<()> {
        // Try to store in keyring
        let keyring_available = self.keyring.is_available();
        let storage_ref = if keyring_available {
            self.keyring.store_credentials(handle, creds)
                .context("Failed to store credentials in keyring")?;
            "keyring"
        } else {
            // TODO: Implement encrypted file storage fallback
            anyhow::bail!("Keyring not available and encrypted file storage not yet implemented");
        };
        
        // Update or add account metadata
        let mut config = self.config.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;
        
        if let Some(account) = config.get_account_mut(handle) {
            account.last_used = SystemTime::now();
        } else {
            let account = Account {
                handle: handle.to_string(),
                did: String::new(), // Will be filled in by OAuth flow
                pds: String::new(), // Will be filled in by OAuth flow
                storage_ref: storage_ref.to_string(),
                created_at: SystemTime::now(),
                last_used: SystemTime::now(),
                metadata: std::collections::HashMap::new(),
            };
            config.add_account(account);
        }
        
        save_config(&config)
            .context("Failed to save config")?;
        
        Ok(())
    }
    
    /// Get credentials for an account
    pub fn get_credentials(&self, handle: &str) -> Result<Credentials> {
        let config = self.config.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;
        
        let account = config.get_account(handle)
            .ok_or_else(|| anyhow::anyhow!("Account not found: {}", handle))?;
        
        // Try to get from keyring
        if account.storage_ref == "keyring" {
            let creds = self.keyring.get_credentials(handle)
                .context("Failed to get credentials from keyring")?;
            
            // Update last used timestamp in background
            drop(config); // Release read lock
            let _ = self.update_last_used(handle);
            
            return Ok(creds);
        }
        
        // TODO: Implement encrypted file storage fallback retrieval
        anyhow::bail!("Storage backend {} not yet implemented", account.storage_ref)
    }
    
    /// Delete credentials for an account
    pub fn delete_credentials(&self, handle: &str) -> Result<()> {
        let mut config = self.config.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;
        
        let account = config.get_account(handle)
            .ok_or_else(|| anyhow::anyhow!("Account not found: {}", handle))?;
        
        // Delete from keyring
        if account.storage_ref == "keyring" {
            self.keyring.delete_credentials(handle)
                .context("Failed to delete credentials from keyring")?;
        }
        
        // Remove from config
        config.remove_account(handle);
        
        // If this was the default account, clear it
        if config.default_account.as_deref() == Some(handle) {
            config.default_account = None;
        }
        
        save_config(&config)
            .context("Failed to save config")?;
        
        Ok(())
    }
    
    /// List all authenticated accounts
    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let config = self.config.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;
        
        Ok(config.accounts.clone())
    }
    
    /// Set the default account
    pub fn set_default_account(&self, handle: &str) -> Result<()> {
        let mut config = self.config.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;
        
        if config.get_account(handle).is_none() {
            anyhow::bail!("Account not found: {}", handle);
        }
        
        config.default_account = Some(handle.to_string());
        
        save_config(&config)
            .context("Failed to save config")?;
        
        Ok(())
    }
    
    /// Get the default account handle
    pub fn get_default_account(&self) -> Result<Option<String>> {
        let config = self.config.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire read lock: {}", e))?;
        
        Ok(config.default_account.clone())
    }
    
    /// Update the last used timestamp for an account
    fn update_last_used(&self, handle: &str) -> Result<()> {
        let mut config = self.config.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;
        
        config.update_last_used(handle);
        save_config(&config)
    }
}
