//! Credential storage with keyring and file fallback

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use crate::auth::{AuthError, Credentials, Session};

const SERVICE_NAME: &str = "autoreply-bluesky";
const DEFAULT_ACCOUNT_KEY: &str = "default_account";

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// OS native keyring
    Keyring,
    /// JSON file in user config directory
    File,
}

/// Stored account data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    credentials: Credentials,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<Session>,
}

/// File-based credential storage format
#[derive(Debug, Default, Serialize, Deserialize)]
struct FileStorage {
    accounts: std::collections::HashMap<String, StoredAccount>,
    default_account: Option<String>,
}

/// Manages credential storage
pub struct CredentialStorage {
    backend: StorageBackend,
    file_path: Option<PathBuf>,
}

impl CredentialStorage {
    /// Create a new credential storage, preferring keyring
    pub fn new() -> Result<Self, AppError> {
        // Try keyring first
        if Self::test_keyring() {
            Ok(Self {
                backend: StorageBackend::Keyring,
                file_path: None,
            })
        } else {
            // Fall back to file storage
            let file_path = Self::get_storage_file_path()?;
            Ok(Self {
                backend: StorageBackend::File,
                file_path: Some(file_path),
            })
        }
    }
    
    /// Test if keyring is available
    fn test_keyring() -> bool {
        let entry = keyring::Entry::new(SERVICE_NAME, "test");
        entry.is_ok()
    }
    
    /// Get the file storage path
    fn get_storage_file_path() -> Result<PathBuf, AppError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::ConfigError("Could not find config directory".to_string()))?;
        
        let app_dir = config_dir.join("autoreply");
        fs::create_dir_all(&app_dir)
            .map_err(|e| AppError::ConfigError(format!("Failed to create config directory: {}", e)))?;
        
        Ok(app_dir.join("credentials.json"))
    }
    
    /// Read file storage
    fn read_file_storage(&self) -> Result<FileStorage, AppError> {
        let path = self.file_path.as_ref()
            .ok_or_else(|| AppError::ConfigError("No file path set".to_string()))?;
        
        if !path.exists() {
            return Ok(FileStorage::default());
        }
        
        let contents = fs::read_to_string(path)
            .map_err(|e| AppError::ConfigError(format!("Failed to read credentials file: {}", e)))?;
        
        serde_json::from_str(&contents)
            .map_err(|e| AppError::ConfigError(format!("Failed to parse credentials file: {}", e)))
    }
    
    /// Write file storage
    fn write_file_storage(&self, storage: &FileStorage) -> Result<(), AppError> {
        let path = self.file_path.as_ref()
            .ok_or_else(|| AppError::ConfigError("No file path set".to_string()))?;
        
        let contents = serde_json::to_string_pretty(storage)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize credentials: {}", e)))?;
        
        fs::write(path, contents)
            .map_err(|e| AppError::ConfigError(format!("Failed to write credentials file: {}", e)))?;
        
        // Set file permissions to user-only (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)
                .map_err(|e| AppError::ConfigError(format!("Failed to get file metadata: {}", e)))?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)
                .map_err(|e| AppError::ConfigError(format!("Failed to set file permissions: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Store credentials for an account
    pub fn store_credentials(&self, handle: &str, credentials: Credentials) -> Result<(), AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let entry = keyring::Entry::new(SERVICE_NAME, handle)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                let data = serde_json::to_string(&credentials)
                    .map_err(|e| AppError::ConfigError(format!("Failed to serialize credentials: {}", e)))?;
                
                entry.set_password(&data)
                    .map_err(|e| AppError::ConfigError(format!("Failed to store credentials: {}", e)))?;
                
                Ok(())
            }
            StorageBackend::File => {
                let mut storage = self.read_file_storage()?;
                storage.accounts.insert(
                    handle.to_string(),
                    StoredAccount {
                        credentials,
                        session: None,
                    }
                );
                self.write_file_storage(&storage)
            }
        }
    }
    
    /// Retrieve credentials for an account
    pub fn get_credentials(&self, handle: &str) -> Result<Credentials, AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let entry = keyring::Entry::new(SERVICE_NAME, handle)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                let data = entry.get_password()
                    .map_err(|_| AuthError::NoCredentials(handle.to_string()))?;
                
                serde_json::from_str(&data)
                    .map_err(|e| AppError::ConfigError(format!("Failed to parse credentials: {}", e)))
            }
            StorageBackend::File => {
                let storage = self.read_file_storage()?;
                storage.accounts
                    .get(handle)
                    .map(|account| account.credentials.clone())
                    .ok_or_else(|| AuthError::NoCredentials(handle.to_string()).into())
            }
        }
    }
    
    /// Store session for an account
    pub fn store_session(&self, handle: &str, session: Session) -> Result<(), AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                // For keyring, store session separately
                let session_key = format!("{}_session", handle);
                let entry = keyring::Entry::new(SERVICE_NAME, &session_key)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                let data = serde_json::to_string(&session)
                    .map_err(|e| AppError::ConfigError(format!("Failed to serialize session: {}", e)))?;
                
                entry.set_password(&data)
                    .map_err(|e| AppError::ConfigError(format!("Failed to store session: {}", e)))?;
                
                Ok(())
            }
            StorageBackend::File => {
                let mut storage = self.read_file_storage()?;
                if let Some(account) = storage.accounts.get_mut(handle) {
                    account.session = Some(session);
                    self.write_file_storage(&storage)
                } else {
                    Err(AuthError::NoCredentials(handle.to_string()).into())
                }
            }
        }
    }
    
    /// Retrieve session for an account
    /// Will be used for token refresh when OAuth is enabled
    #[allow(dead_code)]
    pub fn get_session(&self, handle: &str) -> Result<Option<Session>, AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let session_key = format!("{}_session", handle);
                let entry = keyring::Entry::new(SERVICE_NAME, &session_key)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                match entry.get_password() {
                    Ok(data) => {
                        let session = serde_json::from_str(&data)
                            .map_err(|e| AppError::ConfigError(format!("Failed to parse session: {}", e)))?;
                        Ok(Some(session))
                    }
                    Err(_) => Ok(None),
                }
            }
            StorageBackend::File => {
                let storage = self.read_file_storage()?;
                Ok(storage.accounts.get(handle).and_then(|account| account.session.clone()))
            }
        }
    }
    
    /// Delete credentials for an account
    pub fn delete_credentials(&self, handle: &str) -> Result<(), AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let entry = keyring::Entry::new(SERVICE_NAME, handle)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                let _ = entry.delete_password(); // Ignore errors if not found
                
                // Also delete session
                let session_key = format!("{}_session", handle);
                let session_entry = keyring::Entry::new(SERVICE_NAME, &session_key)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                let _ = session_entry.delete_password();
                
                Ok(())
            }
            StorageBackend::File => {
                let mut storage = self.read_file_storage()?;
                storage.accounts.remove(handle);
                if storage.default_account.as_ref() == Some(&handle.to_string()) {
                    storage.default_account = None;
                }
                self.write_file_storage(&storage)
            }
        }
    }
    
    /// List all stored account handles
    pub fn list_accounts(&self) -> Result<Vec<String>, AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                // For keyring, we need to store the list separately
                let list_entry = keyring::Entry::new(SERVICE_NAME, "account_list")
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                match list_entry.get_password() {
                    Ok(data) => {
                        serde_json::from_str(&data)
                            .map_err(|e| AppError::ConfigError(format!("Failed to parse account list: {}", e)))
                    }
                    Err(_) => Ok(vec![]),
                }
            }
            StorageBackend::File => {
                let storage = self.read_file_storage()?;
                Ok(storage.accounts.keys().cloned().collect())
            }
        }
    }
    
    /// Update the account list (for keyring backend)
    fn update_account_list(&self, handle: &str, add: bool) -> Result<(), AppError> {
        if self.backend != StorageBackend::Keyring {
            return Ok(());
        }
        
        let mut accounts = self.list_accounts()?;
        
        if add {
            if !accounts.contains(&handle.to_string()) {
                accounts.push(handle.to_string());
            }
        } else {
            accounts.retain(|h| h != handle);
        }
        
        let list_entry = keyring::Entry::new(SERVICE_NAME, "account_list")
            .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
        
        let data = serde_json::to_string(&accounts)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize account list: {}", e)))?;
        
        list_entry.set_password(&data)
            .map_err(|e| AppError::ConfigError(format!("Failed to store account list: {}", e)))?;
        
        Ok(())
    }
    
    /// Store credentials and update account list
    pub fn add_account(&self, handle: &str, credentials: Credentials) -> Result<(), AppError> {
        self.store_credentials(handle, credentials)?;
        self.update_account_list(handle, true)?;
        Ok(())
    }
    
    /// Delete credentials and update account list
    pub fn remove_account(&self, handle: &str) -> Result<(), AppError> {
        self.delete_credentials(handle)?;
        self.update_account_list(handle, false)?;
        Ok(())
    }
    
    /// Get default account handle
    pub fn get_default_account(&self) -> Result<Option<String>, AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let entry = keyring::Entry::new(SERVICE_NAME, DEFAULT_ACCOUNT_KEY)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                match entry.get_password() {
                    Ok(handle) => Ok(Some(handle)),
                    Err(_) => Ok(None),
                }
            }
            StorageBackend::File => {
                let storage = self.read_file_storage()?;
                Ok(storage.default_account)
            }
        }
    }
    
    /// Set default account handle
    pub fn set_default_account(&self, handle: &str) -> Result<(), AppError> {
        match self.backend {
            StorageBackend::Keyring => {
                let entry = keyring::Entry::new(SERVICE_NAME, DEFAULT_ACCOUNT_KEY)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create keyring entry: {}", e)))?;
                
                entry.set_password(handle)
                    .map_err(|e| AppError::ConfigError(format!("Failed to set default account: {}", e)))?;
                
                Ok(())
            }
            StorageBackend::File => {
                let mut storage = self.read_file_storage()?;
                storage.default_account = Some(handle.to_string());
                self.write_file_storage(&storage)
            }
        }
    }
    
    /// Get the storage backend type
    pub fn backend(&self) -> StorageBackend {
        self.backend
    }
}

impl Default for CredentialStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create CredentialStorage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_storage_backend() {
        let storage = CredentialStorage::new().unwrap();
        // Should be either Keyring or File
        assert!(matches!(storage.backend(), StorageBackend::Keyring | StorageBackend::File));
    }
    
    #[test]
    fn test_file_storage_path() {
        let path = CredentialStorage::get_storage_file_path().unwrap();
        assert!(path.to_string_lossy().contains("autoreply"));
        assert!(path.to_string_lossy().ends_with("credentials.json"));
    }
}
