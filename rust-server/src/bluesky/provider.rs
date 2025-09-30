//! Repository provider for fetching and parsing ATProto repositories.

use crate::bluesky::did::DidResolver;
use crate::cache::{CacheManager, CacheMetadata};
use crate::error::AppError;
use anyhow::Result;
use atrium_repo::{blockstore::CarStore, Repository};
use futures::StreamExt;
use reqwest::Client;
use std::io::Cursor;
use std::time::{Duration};
use tracing::{debug, info};

/// Provides a parsed `Repo` object for a given DID.
///
/// This provider encapsulates the logic for:
/// 1. Resolving DIDs to get PDS endpoints.
/// 2. Fetching repository CAR files over HTTP.
/// 3. Caching the CAR files locally.
/// 4. Parsing the CAR files into a `Repo` object.
pub struct RepositoryProvider {
    client: Client,
    cache: CacheManager,
    did_resolver: DidResolver,
}

impl RepositoryProvider {
    /// Creates a new `RepositoryProvider`.
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::HttpClientInitialization(e.to_string()))?;
        let cache = CacheManager::new()?;
        let did_resolver = DidResolver::new();
        Ok(Self {
            client,
            cache,
            did_resolver,
        })
    }

    /// Gets a parsed `Repo` for a given DID.
    ///
    /// This method handles fetching the repository from the network if it's not cached
    /// or if the cached version is stale. The parsing is done synchronously on a
    /// blocking thread to avoid stalling the async runtime.
    pub async fn get_repo(
        &self,
        did: &str,
    ) -> Result<Repository<CarStore<Cursor<Vec<u8>>>>, AppError> {
        let car_data = self.fetch_repo_car(did).await?;
        let cursor = Cursor::new(car_data);
        let store = CarStore::new(cursor)
            .map_err(|e| AppError::RepoParseFailed(format!("CarStore::new failed: {}", e)))?;
        let repo = Repository::new(store)
            .map_err(|e| AppError::RepoParseFailed(format!("Repository::new failed: {}", e)))?;
        Ok(repo)
            .await
            .map_err(|e| AppError::RepoParseFailed(format!("spawn_blocking join error: {}", e)))?
    }

    /// Fetches the repository CAR file for a DID.
    ///
    /// Handles PDS resolution, caching, and HTTP fetching.
    async fn fetch_repo_car(&self, did: &str) -> Result<Vec<u8>, AppError> {
        let pds_endpoint = self.did_resolver.discover_pds(did).await?.ok_or_else(|| {
            AppError::DidResolveFailed(format!("Could not determine PDS for DID {}", did))
        })?;
        let url = format!("{}/xrpc/com.atproto.sync.getRepo?did={}", pds_endpoint, did);
        debug!("Fetching repo from URL: {}", url);

        let cache_key = did.to_string();
        if let Ok(metadata) = self.cache.get_metadata(&cache_key) {
            if self.cache.is_cache_valid(&cache_key, metadata.ttl_hours) {
                if let Ok(cached_data) = self.cache.read_car(&cache_key) {
                    info!("Repo for {} is valid in cache. Loading from cache.", did);
                    return Ok(cached_data);
                }
            }
        }

        let mut request = self.client.get(&url);
        if let Ok(metadata) = self.cache.get_metadata(&cache_key) {
            if let Some(etag) = &metadata.etag {
                request = request.header("If-None-Match", etag);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::NetworkError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            info!("Repo for {} not modified. Loading from cache.", did);
            return self.cache.read_car(&cache_key);
        }

        if !response.status().is_success() {
            return Err(AppError::NetworkError(format!(
                "Failed to fetch repo: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let new_etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let content_length = response.content_length();

        info!(
            "Streaming repo for {} ({} bytes)",
            did,
            content_length.unwrap_or(0)
        );

        let mut car_data = Vec::with_capacity(content_length.unwrap_or(0) as usize);
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::NetworkError(e.to_string()))?;
            car_data.extend_from_slice(&chunk);
        }

        let mut new_metadata = CacheMetadata::new(did.to_string(), 24);
        new_metadata = new_metadata.with_headers(new_etag, None, content_length);
        self.cache.store_car(&cache_key, &car_data, new_metadata)?;

        Ok(car_data)
    }
}

impl Default for RepositoryProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create default RepositoryProvider")
    }
}
