//! Repository provider for fetching and parsing ATProto repositories.

use crate::bluesky::did::DidResolver;
use crate::error::AppError;
use futures::StreamExt;
use reqwest::Client;
use std::path::PathBuf;
use std::time::Duration;
use tracing::debug;

/// Provides a parsed `Repo` object for a given DID.
///
/// This provider encapsulates the logic for:
/// 1. Resolving DIDs to get PDS endpoints.
/// 2. Fetching repository CAR files over HTTP, streaming directly to disk.
/// 3. Caching the CAR files locally with atomic operations.
/// 4. Parsing the CAR files into a `Repository` object using atrium-repo APIs.
pub struct RepositoryProvider {
    client: Client,
    cache_dir: PathBuf,
    did_resolver: DidResolver,
}

impl RepositoryProvider {
    /// Creates a new `RepositoryProvider`.
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            // No total timeout - let downloads complete as long as data flows
            .connect_timeout(Duration::from_secs(120)) // 2 minutes to establish connection (slow/flaky networks)
            .read_timeout(Duration::from_secs(120)) // 2 minutes between data chunks (detect true stalls)
            .user_agent("autoreply/0.3")
            .build()
            .map_err(|e| AppError::HttpClientInitialization(e.to_string()))?;

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("autoreply")
            .join("repos");

        std::fs::create_dir_all(&cache_dir).map_err(|e| {
            AppError::CacheError(format!("Failed to create cache directory: {}", e))
        })?;

        let did_resolver = DidResolver::new();
        Ok(Self {
            client,
            cache_dir,
            did_resolver,
        })
    }

    /// Fetches the repository CAR file for a DID.
    ///
    /// Streams the CAR file directly to disk with atomic operations as specified in PROCEED-FIX.md.
    /// Returns the path to the cached CAR file.
    pub async fn fetch_repo_car(&self, did: &str) -> Result<PathBuf, AppError> {
        // Resolve DID to PDS endpoint
        let pds_endpoint = self.did_resolver.discover_pds(did).await?.ok_or_else(|| {
            AppError::DidResolveFailed(format!("Could not determine PDS for DID {}", did))
        })?;
        let url = format!("{}/xrpc/com.atproto.sync.getRepo?did={}", pds_endpoint, did);
        debug!("Fetching repo from URL: {}", url);

        // Generate cache file paths
        let cache_filename = format!("{}.car", did.replace(':', "_"));
        let final_path = self.cache_dir.join(&cache_filename);

        // Check if cached file exists (no TTL or metadata per PROCEED-FIX.md spec)
        if final_path.exists() {
            debug!("Using cached repo for {}", did);
            return Ok(final_path);
        }

        // Generate temporary file path with randomized suffix to avoid collisions
        let temp_filename = format!("{}.tmp.{}", cache_filename, std::process::id());
        let temp_path = self.cache_dir.join(&temp_filename);

        // Fetch CAR file and stream directly to temp file
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.ipld.car")
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Failed to connect: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::NetworkError(format!(
                "Failed to fetch repo: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let content_length = response.content_length().unwrap_or(0);
        debug!("Downloading repo for {} ({} bytes)", did, content_length);

        // Stream bytes directly to temp file
        let mut temp_file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| AppError::CacheError(format!("Failed to create temp file: {}", e)))?;

        let mut stream = response.bytes_stream();
        let mut bytes_written = 0;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| {
                AppError::NetworkError(format!(
                    "Connection interrupted after {} bytes: {}",
                    bytes_written, e
                ))
            })?;

            bytes_written += chunk.len();
            tokio::io::AsyncWriteExt::write_all(&mut temp_file, &chunk)
                .await
                .map_err(|e| {
                    AppError::CacheError(format!("Failed to write to temp file: {}", e))
                })?;
        }

        // Flush and fsync as required
        tokio::io::AsyncWriteExt::flush(&mut temp_file)
            .await
            .map_err(|e| AppError::CacheError(format!("Failed to flush temp file: {}", e)))?;
        temp_file
            .sync_all()
            .await
            .map_err(|e| AppError::CacheError(format!("Failed to fsync temp file: {}", e)))?;

        // Drop the file handle before rename
        drop(temp_file);

        // Atomically rename temp file to final path
        std::fs::rename(&temp_path, &final_path).map_err(|e| {
            AppError::CacheError(format!("Failed to atomically rename temp file: {}", e))
        })?;

        debug!("Cached repo for {} ({} bytes)", did, bytes_written);
        Ok(final_path)
    }

    /// Get an iterator over AT Protocol records from a user's repository.
    /// Returns a streaming iterator that yields (record_type, cbor_data) tuples.
    /// This avoids loading all records into memory and supports early termination.
    pub async fn records(&self, did: &str) -> Result<crate::car::CarRecords, AppError> {
        let car_file_path = self.fetch_repo_car(did).await?;

        // Read entire file into memory (CAR files are typically 1-10MB)
        let car_bytes = tokio::fs::read(&car_file_path)
            .await
            .map_err(|e| AppError::CacheError(format!("Failed to read CAR file: {}", e)))?;

        // Create iterator from CAR file bytes
        crate::car::CarRecords::from_bytes(car_bytes)
            .map_err(|e| AppError::RepoParseFailed(format!("Failed to create CAR iterator: {}", e)))
    }
}

impl Default for RepositoryProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create default RepositoryProvider")
    }
}
