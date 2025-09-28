//! Cache management for CAR files and metadata
//!
//! Implements the two-tier directory structure: {cache_dir}/{2-letter-prefix}/{full-did}/

use crate::error::AppError;
use anyhow::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Cache metadata stored alongside CAR files
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub did: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_length: Option<u64>,
    pub cached_at: u64,
    pub ttl_hours: u64,
}

/// Cache management struct
pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    /// Create new cache manager with platform-specific cache directory
    pub fn new() -> Result<Self> {
        let cache_dir = get_cache_dir()?;
        
        // Ensure cache directory exists
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
            info!("Created cache directory: {}", cache_dir.display());
        }

        Ok(Self { cache_dir })
    }

    /// Get the cache path for a DID using two-tier structure
    pub fn get_cache_path(&self, did: &str) -> Result<PathBuf, AppError> {
        if !did.starts_with("did:plc:") || did.len() < 10 {
            return Err(AppError::InvalidInput("Invalid DID format".to_string()));
        }

        // Extract 2-letter prefix from DID (first two chars after "did:plc:")
        let prefix = &did[8..10];
        
        // Two-tier structure: {cache_dir}/{2-letter-prefix}/{full-did}/
        let cache_path = self.cache_dir.join(prefix).join(did);
        
        Ok(cache_path)
    }

    /// Get paths for CAR file and metadata
    pub fn get_file_paths(&self, did: &str) -> Result<(PathBuf, PathBuf), AppError> {
        let cache_path = self.get_cache_path(did)?;
        let car_path = cache_path.join("repo.car");
        let metadata_path = cache_path.join("metadata.json");
        
        Ok((car_path, metadata_path))
    }

    /// Check if cached data is valid and not expired
    pub fn is_cache_valid(&self, did: &str, ttl_hours: u64) -> bool {
        match self.get_metadata(did) {
            Ok(metadata) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                let cache_age = now - metadata.cached_at;
                let ttl_seconds = ttl_hours * 3600;
                
                debug!(
                    "Cache for DID {} is {} seconds old (TTL: {} seconds)",
                    did, cache_age, ttl_seconds
                );
                
                cache_age < ttl_seconds
            }
            Err(_) => false,
        }
    }

    /// Get cached metadata
    pub fn get_metadata(&self, did: &str) -> Result<CacheMetadata, AppError> {
        let (_, metadata_path) = self.get_file_paths(did)?;
        
        if !metadata_path.exists() {
            return Err(AppError::NotFound("Metadata not found".to_string()));
        }

        let metadata_str = fs::read_to_string(&metadata_path)?;
        let metadata: CacheMetadata = serde_json::from_str(&metadata_str)?;
        
        Ok(metadata)
    }

    /// Store CAR file and metadata atomically
    pub fn store_car(&self, did: &str, car_data: &[u8], metadata: CacheMetadata) -> Result<(), AppError> {
        let (car_path, metadata_path) = self.get_file_paths(did)?;
        
        // Ensure directory exists
        if let Some(parent) = car_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Use .tmp suffix for atomic writes as specified
        let car_tmp_path = car_path.with_extension("car.tmp");
        let metadata_tmp_path = metadata_path.with_extension("json.tmp");

        // Lock the directory during write
        let lock_path = car_path.with_extension("lock");
        let lock_file = fs::File::create(&lock_path)?;
        lock_file.lock_exclusive()?;

        // Write files atomically
        fs::write(&car_tmp_path, car_data)?;
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_tmp_path, metadata_json)?;

        // Atomic rename
        fs::rename(&car_tmp_path, &car_path)?;
        fs::rename(&metadata_tmp_path, &metadata_path)?;

        // Release lock
        lock_file.unlock()?;
        let _ = fs::remove_file(lock_path); // Best effort cleanup

        info!("Cached CAR file for DID: {}", did);
        Ok(())
    }

    /// Read cached CAR file
    pub fn read_car(&self, did: &str) -> Result<Vec<u8>, AppError> {
        let (car_path, _) = self.get_file_paths(did)?;
        
        if !car_path.exists() {
            return Err(AppError::NotFound("CAR file not found".to_string()));
        }

        let car_data = fs::read(&car_path)?;
        debug!("Read CAR file: {} bytes", car_data.len());
        
        Ok(car_data)
    }

    /// Clean up expired cache entries
    pub fn cleanup_expired(&self) -> Result<(), AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cleaned_count = 0;
        
        // Walk through all 2-letter prefix directories
        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let prefix_dir = entry.path();
            
            // Walk through DID directories
            for did_entry in fs::read_dir(&prefix_dir)? {
                let did_entry = did_entry?;
                if !did_entry.file_type()?.is_dir() {
                    continue;
                }

                let did_dir = did_entry.path();
                let metadata_path = did_dir.join("metadata.json");
                
                if let Ok(metadata_str) = fs::read_to_string(&metadata_path) {
                    if let Ok(metadata) = serde_json::from_str::<CacheMetadata>(&metadata_str) {
                        let cache_age = now - metadata.cached_at;
                        let ttl_seconds = metadata.ttl_hours * 3600;
                        
                        if cache_age > ttl_seconds {
                            if let Err(e) = fs::remove_dir_all(&did_dir) {
                                warn!("Failed to remove expired cache dir {}: {}", did_dir.display(), e);
                            } else {
                                cleaned_count += 1;
                                debug!("Removed expired cache for: {}", metadata.did);
                            }
                        }
                    }
                }
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned up {} expired cache entries", cleaned_count);
        }

        Ok(())
    }
}

/// Get platform-specific cache directory
fn get_cache_dir() -> Result<PathBuf> {
    if let Some(xdg_cache) = std::env::var_os("XDG_CACHE_HOME") {
        Ok(PathBuf::from(xdg_cache).join("bluesky-mcp"))
    } else if let Some(home) = dirs::home_dir() {
        if cfg!(target_os = "windows") {
            if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
                Ok(PathBuf::from(local_appdata).join("bluesky-mcp"))
            } else {
                Ok(home.join("AppData").join("Local").join("bluesky-mcp"))
            }
        } else {
            Ok(home.join(".cache").join("bluesky-mcp"))
        }
    } else {
        // Fallback to current directory
        Ok(PathBuf::from(".cache").join("bluesky-mcp"))
    }
}

impl CacheMetadata {
    /// Create new metadata
    pub fn new(did: String, ttl_hours: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            did,
            etag: None,
            last_modified: None,
            content_length: None,
            cached_at: now,
            ttl_hours,
        }
    }

    /// Set HTTP headers
    pub fn with_headers(mut self, etag: Option<String>, last_modified: Option<String>, content_length: Option<u64>) -> Self {
        self.etag = etag;
        self.last_modified = last_modified;
        self.content_length = content_length;
        self
    }
}