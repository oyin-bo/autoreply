//! DID resolution functionality
//! 
//! Handles resolving Bluesky handles to DIDs via XRPC

use crate::error::AppError;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// DID resolution response from XRPC
#[derive(Debug, Deserialize)]
struct ResolveHandleResponse {
    did: String,
}

/// Simple in-memory cache for DID resolution
pub struct DidResolver {
    client: Client,
    cache: Mutex<HashMap<String, (String, Instant)>>,
    cache_ttl: Duration,
}

impl DidResolver {
    /// Create new DID resolver
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10)) // As specified in docs
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            cache: Mutex::new(HashMap::new()),
            cache_ttl: Duration::from_secs(3600), // 1 hour cache
        }
    }

    /// Resolve handle to DID
    pub async fn resolve_handle(&self, account: &str) -> Result<String, AppError> {
        // If it's already a DID, return it
        if account.starts_with("did:plc:") {
            return Ok(account.to_string());
        }

        // Check cache first
        {
            let mut cache = self.cache.lock().await;
            if let Some((did, cached_at)) = cache.get(account) {
                if cached_at.elapsed() < self.cache_ttl {
                    debug!("DID cache hit for handle: {}", account);
                    return Ok(did.clone());
                } else {
                    // Remove expired entry
                    cache.remove(account);
                }
            }
        }

        // Clean handle (remove @ prefix if present)
        let clean_handle = account.strip_prefix('@').unwrap_or(account);
        
        // Extract hostname for resolution endpoint
        let hostname = if clean_handle.contains('.') {
            let parts: Vec<&str> = clean_handle.split('.').collect();
            if parts.len() >= 2 {
                format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
            } else {
                "bsky.social".to_string()
            }
        } else {
            "bsky.social".to_string()
        };

        // Resolve via XRPC as specified in docs
        let url = format!(
            "https://{}/xrpc/com.atproto.identity.resolveHandle?handle={}",
            hostname, clean_handle
        );

        debug!("Resolving handle {} via {}", clean_handle, url);

        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AppError::DidResolveFailed(format!(
                "HTTP {} from {}: {}",
                response.status(),
                hostname,
                response.text().await.unwrap_or_default()
            )));
        }

        let resolve_response: ResolveHandleResponse = response.json().await?;
        let did = resolve_response.did;

        // Validate DID format
        if !did.starts_with("did:plc:") || did.len() != 32 {
            return Err(AppError::DidResolveFailed(format!(
                "Invalid DID format returned: {}",
                did
            )));
        }

        // Cache the result
        {
            let mut cache = self.cache.lock().await;
            cache.insert(account.to_string(), (did.clone(), Instant::now()));
        }

        debug!("Resolved handle {} to DID {}", account, did);
        Ok(did)
    }

    /// Clear expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.cache.lock().await;
        let now = Instant::now();
        
        cache.retain(|_, (_, cached_at)| now.duration_since(*cached_at) < self.cache_ttl);
    }
}

impl Default for DidResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate DID format
pub fn is_valid_did(did: &str) -> bool {
    did.starts_with("did:plc:") 
        && did.len() == 32 
        && did[8..].chars().all(|c| c.is_ascii_alphanumeric())
}

/// Validate handle format
pub fn is_valid_handle(handle: &str) -> bool {
    let clean_handle = handle.strip_prefix('@').unwrap_or(handle);
    
    // Must contain at least one dot
    if !clean_handle.contains('.') {
        return false;
    }

    // Split into parts and validate
    let parts: Vec<&str> = clean_handle.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // Each part must not be empty and contain valid characters
    parts.iter().all(|part| {
        !part.is_empty() 
            && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !part.starts_with('-')
            && !part.ends_with('-')
    })
}