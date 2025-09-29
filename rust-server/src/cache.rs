//! Cache management for CAR files and metadata
//!
//! Implements the two-tier directory structure: {cache_dir}/{2-letter-prefix}/{full-did}/

use crate::error::AppError;
use anyhow::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Cache metadata stored alongside CAR files
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let prefix = if did.starts_with("did:plc:") && did.len() >= 10 {
            did[8..10].to_string()
        } else if did.starts_with("did:web:") {
            let rest = &did[8..];
            let sanitized: String = rest.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if sanitized.len() >= 2 {
                sanitized[0..2].to_string()
            } else {
                "xx".to_string()
            }
        } else {
            // Fallback for any other identifiers: derive from first two alnum chars
            let sanitized: String = did.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if sanitized.len() >= 2 { sanitized[0..2].to_string() } else { "xx".to_string() }
        };

        // Two-tier structure: {cache_dir}/{2-letter-prefix}/{id-without-scheme}/
        let dir_key = if did.starts_with("did:plc:") {
            &did[8..]
        } else if did.starts_with("did:web:") {
            &did[8..]
        } else {
            did
        };

        let cache_path = self.cache_dir.join(prefix).join(dir_key);
        Ok(cache_path)
    }

    /// Get paths for CAR file and metadata
    pub fn get_file_paths(&self, did: &str) -> Result<(PathBuf, PathBuf), AppError> {
        // Compute scheme-less cache path
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
        fs2::FileExt::unlock(&lock_file)?;
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

    /// Remove CAR cache directories where (current_time - cached_at) > ttl_hours
    /// In future to be called periodically by maintenance tasks or CLI commands to prevent disk bloat
    #[allow(dead_code)]
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
    // macOS: ~/Library/Caches/autoreply/did
    if cfg!(target_os = "macos") {
        if let Some(home) = dirs::home_dir() {
            return Ok(home.join("Library").join("Caches").join("autoreply").join("did"));
        }
    }

    // Windows: %LOCALAPPDATA%\autoreply\did (fallback to ~/AppData/Local)
    if cfg!(target_os = "windows") {
        if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local_appdata).join("autoreply").join("did"));
        }
        if let Some(home) = dirs::home_dir() {
            return Ok(home.join("AppData").join("Local").join("autoreply").join("did"));
        }
    }

    // Linux/Unix: $XDG_CACHE_HOME/autoreply/did or ~/.cache/autoreply/did
    if let Some(xdg_cache) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg_cache).join("autoreply").join("did"));
    }
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".cache").join("autoreply").join("did"));
    }

    // Fallback to relative .cache/autoreply/did
    Ok(PathBuf::from(".cache").join("autoreply").join("did"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use tempfile::{tempdir, TempDir};

    /// Helper to create a cache manager in a temporary directory
    fn create_test_cache_manager() -> (CacheManager, TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_manager = CacheManager {
            cache_dir: temp_dir.path().to_path_buf(),
        };
        (cache_manager, temp_dir)
    }

    #[test]
    fn test_cache_metadata_new() {
        let did = "did:plc:abc123def456".to_string();
        let ttl_hours = 24;
        
        let metadata = CacheMetadata::new(did.clone(), ttl_hours);
        
        assert_eq!(metadata.did, did);
        assert_eq!(metadata.ttl_hours, ttl_hours);
        assert!(metadata.etag.is_none());
        assert!(metadata.last_modified.is_none());
        assert!(metadata.content_length.is_none());
        assert!(metadata.cached_at > 0);
    }

    #[test]
    fn test_cache_metadata_with_headers() {
        let metadata = CacheMetadata::new("did:plc:test".to_string(), 24)
            .with_headers(
                Some("etag123".to_string()),
                Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
                Some(1024),
            );
        
        assert_eq!(metadata.etag, Some("etag123".to_string()));
        assert_eq!(metadata.last_modified, Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()));
        assert_eq!(metadata.content_length, Some(1024));
    }

    #[test]
    fn test_get_cache_path_plc() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let path = cache_manager.get_cache_path(did).unwrap();
        
        // Should use first 2 characters after "did:plc:" for prefix, then full id for directory
        let expected = cache_manager.cache_dir.join("ab").join("abc123def456789012345");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_get_cache_path_web() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:web:example.com:user:alice";
        let path = cache_manager.get_cache_path(did).unwrap();
        
        // Should use first 2 alphanumeric characters from domain
        let expected = cache_manager.cache_dir.join("ex").join("example.com:user:alice");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_get_cache_path_fallback() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        // Test with identifier that has fewer than 2 alphanumeric chars
        let did = "x";
        let path = cache_manager.get_cache_path(did).unwrap();
        
        let expected = cache_manager.cache_dir.join("xx").join("x");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_get_file_paths() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let (car_path, metadata_path) = cache_manager.get_file_paths(did).unwrap();
        
        let cache_path = cache_manager.get_cache_path(did).unwrap();
        assert_eq!(car_path, cache_path.join("repo.car"));
        assert_eq!(metadata_path, cache_path.join("metadata.json"));
    }

    #[test]
    fn test_store_and_read_car() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let car_data = b"test_car_data";
        let metadata = CacheMetadata::new(did.to_string(), 24)
            .with_headers(Some("etag123".to_string()), None, Some(car_data.len() as u64));
        
        // Store the CAR file
        cache_manager.store_car(did, car_data, metadata.clone()).unwrap();
        
        // Read it back
        let read_data = cache_manager.read_car(did).unwrap();
        assert_eq!(read_data, car_data);
        
        // Check metadata
        let read_metadata = cache_manager.get_metadata(did).unwrap();
        assert_eq!(read_metadata.did, metadata.did);
        assert_eq!(read_metadata.etag, metadata.etag);
        assert_eq!(read_metadata.ttl_hours, metadata.ttl_hours);
    }

    #[test]
    fn test_read_nonexistent_car() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:nonexistent";
        let result = cache_manager.read_car(did);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NotFound(_) => {} // Expected
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_get_nonexistent_metadata() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:nonexistent";
        let result = cache_manager.get_metadata(did);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NotFound(_) => {} // Expected
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_is_cache_valid_fresh() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let car_data = b"test_data";
        let metadata = CacheMetadata::new(did.to_string(), 24);
        
        cache_manager.store_car(did, car_data, metadata).unwrap();
        
        // Should be valid with 24 hour TTL
        assert!(cache_manager.is_cache_valid(did, 24));
    }

    #[test]
    fn test_is_cache_valid_expired() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let car_data = b"test_data";
        
        // Create metadata with old timestamp
        let old_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - 25 * 3600; // 25 hours ago
            
        let mut metadata = CacheMetadata::new(did.to_string(), 24);
        metadata.cached_at = old_timestamp;
        
        cache_manager.store_car(did, car_data, metadata).unwrap();
        
        // Should be expired with 24 hour TTL
        assert!(!cache_manager.is_cache_valid(did, 24));
    }

    #[test]
    fn test_is_cache_valid_nonexistent() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:nonexistent";
        
        // Should be false for non-existent cache
        assert!(!cache_manager.is_cache_valid(did, 24));
    }

    #[test]
    fn test_atomic_write_behavior() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        let did = "did:plc:abc123def456789012345";
        let car_data = b"test_data";
        let metadata = CacheMetadata::new(did.to_string(), 24);
        
        let (car_path, metadata_path) = cache_manager.get_file_paths(did).unwrap();
        let car_tmp_path = car_path.with_extension("car.tmp");
        let metadata_tmp_path = metadata_path.with_extension("json.tmp");
        
        // Ensure temp files don't exist initially
        assert!(!car_tmp_path.exists());
        assert!(!metadata_tmp_path.exists());
        
        // Store the file
        cache_manager.store_car(did, car_data, metadata).unwrap();
        
        // Final files should exist
        assert!(car_path.exists());
        assert!(metadata_path.exists());
        
        // Temp files should be cleaned up
        assert!(!car_tmp_path.exists());
        assert!(!metadata_tmp_path.exists());
    }

    #[test]
    fn test_cleanup_expired() {
        let (cache_manager, _temp_dir) = create_test_cache_manager();
        
        // Create a fresh cache entry
        let fresh_did = "did:plc:fresh12345678901234567";
        let fresh_metadata = CacheMetadata::new(fresh_did.to_string(), 24);
        cache_manager.store_car(fresh_did, b"fresh_data", fresh_metadata).unwrap();
        
        // Create an expired cache entry
        let expired_did = "did:plc:expired123456789012345";
        let old_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - 25 * 3600; // 25 hours ago
            
        let mut expired_metadata = CacheMetadata::new(expired_did.to_string(), 24);
        expired_metadata.cached_at = old_timestamp;
        cache_manager.store_car(expired_did, b"expired_data", expired_metadata).unwrap();
        
        // Verify both exist initially
        assert!(cache_manager.read_car(fresh_did).is_ok());
        assert!(cache_manager.read_car(expired_did).is_ok());
        
        // Run cleanup
        cache_manager.cleanup_expired().unwrap();
        
        // Fresh should still exist, expired should be gone
        assert!(cache_manager.read_car(fresh_did).is_ok());
        assert!(cache_manager.read_car(expired_did).is_err());
    }

    #[test]
    fn test_platform_specific_cache_dir() {
        // Test that get_cache_dir returns a reasonable path
        let cache_dir = get_cache_dir().unwrap();
        assert!(!cache_dir.as_os_str().is_empty());
        
        // Should contain "autoreply" and "did" in the path
        let path_str = cache_dir.to_string_lossy();
        assert!(path_str.contains("autoreply"));
        assert!(path_str.contains("did"));
    }

    #[test]
    fn test_cache_manager_new() {
        // This will create the cache manager using platform-specific directory
        let result = CacheManager::new();
        assert!(result.is_ok());
    }
}