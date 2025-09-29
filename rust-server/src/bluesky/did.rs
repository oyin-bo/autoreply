//! DID resolution functionality
//! 
//! Handles resolving Bluesky handles to DIDs via XRPC

use crate::error::AppError;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn, info};
use serde_json::Value;
use reqwest::header::ACCEPT;

/// DID resolution response from XRPC
#[derive(Debug, Deserialize)]
struct ResolveHandleResponse {
    did: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_web_to_did_document_url_root() {
        let url = did_web_to_did_document_url("did:web:example.com").unwrap();
        assert_eq!(url, "https://example.com/.well-known/did.json");
    }

    #[test]
    fn test_did_web_to_did_document_url_path() {
        let url = did_web_to_did_document_url("did:web:example.com:user:alice").unwrap();
        assert_eq!(url, "https://example.com/user/alice/did.json");
    }
}

impl DidResolver {
    /// Ensure PDS for a did:web is loaded; return it if found
    pub async fn ensure_did_web_pds(&self, did: &str) -> Result<Option<String>, AppError> {
        // If we already have it, return
        if let Some(pds) = self.get_pds_for(did).await { return Ok(Some(pds)); }
        // Attempt to fetch did:web document and cache endpoint
        self.resolve_did_web(did).await?;
        Ok(self.get_pds_for(did).await)
    }
    /// Get cached PDS endpoint for a DID (from did:web documents)
    pub async fn get_pds_for(&self, did: &str) -> Option<String> {
        let map = self.pds_map.lock().await;
        map.get(did).cloned()
    }

    /// Try resolving DID via .well-known on the handle domain.
    /// Returns Ok(Some(did)) on success, Ok(None) if not found or invalid, Err on parsing errors only.
    async fn try_well_known(&self, handle_domain: &str) -> Result<Option<String>, AppError> {
        // Only attempt if it looks like a domain
        if !handle_domain.contains('.') {
            return Ok(None);
        }

        let url = format!("https://{}/.well-known/atproto-did", handle_domain);
        debug!("Trying .well-known at {}", url);

        // Do not fail hard on network errors; just fall back to XRPC
        let resp = match self
            .client
            .get(&url)
            .header(ACCEPT, "text/plain, application/json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(".well-known request error for {}: {}", handle_domain, e);
                return Ok(None);
            }
        };

        if !resp.status().is_success() {
            debug!(".well-known HTTP {} for {}", resp.status(), handle_domain);
            return Ok(None);
        }

        let body = match resp.text().await {
            Ok(t) => t.trim().to_string(),
            Err(e) => {
                warn!(".well-known read error for {}: {}", handle_domain, e);
                return Ok(None);
            }
        };

        // Primary format is plain text DID
        if body.starts_with("did:plc:") && body.len() == 32 && body[8..].chars().all(|c| c.is_ascii_alphanumeric()) {
            return Ok(Some(body));
        }

        // Some deployments might return JSON { "did": "..." }
        if body.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(d) = v.get("did").and_then(|x| x.as_str()) {
                    if d.starts_with("did:plc:") && d.len() == 32 && d[8..].chars().all(|c| c.is_ascii_alphanumeric()) {
                        return Ok(Some(d.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Cache the resolution result
    async fn cache_result(&self, account: &str, did: &str) {
        let mut cache = self.cache.lock().await;
        cache.insert(account.to_string(), (did.to_string(), Instant::now()));
    }

    /// Resolve a did:web DID by fetching its DID document and extracting the PDS endpoint if present
    async fn resolve_did_web(&self, did: &str) -> Result<(), AppError> {
        // Convert did:web to URL of did.json
        let url = did_web_to_did_document_url(did).ok_or_else(|| {
            AppError::DidResolveFailed("Invalid did:web format".to_string())
        })?;

        debug!("Fetching did:web document at {}", url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::DidResolveFailed(format!(
                "did:web document fetch failed with status {}",
                resp.status()
            )));
        }

        let text = resp.text().await.unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| AppError::DidResolveFailed(format!("Invalid did:web JSON: {}", e)))?;

        // Extract service endpoint for AtprotoPersonalDataServer
        if let Some(services) = v.get("service").and_then(|s| s.as_array()) {
            for svc in services {
                let type_ok = svc.get("type").and_then(|t| t.as_str()).map(|t| t == "AtprotoPersonalDataServer").unwrap_or(false);
                let id_ok = svc.get("id").and_then(|t| t.as_str()).map(|t| t.contains("atproto") || t.contains("pds")).unwrap_or(false);
                if type_ok || id_ok {
                    if let Some(ep) = svc.get("serviceEndpoint").and_then(|e| e.as_str()) {
                        let endpoint = if ep.starts_with("http") { ep.to_string() } else { format!("https://{}", ep) };
                        let mut map = self.pds_map.lock().await;
                        map.insert(did.to_string(), endpoint);
                        return Ok(());
                    }
                }
            }
        }

        // Some did:web docs might use properties under verificationMethod or elsewhere; ignore for now
        Ok(())
    }
}

/// Convert did:web identifier to the URL of its did.json
fn did_web_to_did_document_url(did: &str) -> Option<String> {
    if !did.starts_with("did:web:") { return None; }
    let rest = &did[8..];
    if rest.is_empty() { return None; }
    // did:web:example.com          => https://example.com/.well-known/did.json
    // did:web:example.com:foo:bar  => https://example.com/foo/bar/did.json
    let parts: Vec<&str> = rest.split(':').collect();
    let host = parts.get(0)?.to_string();
    if parts.len() == 1 {
        Some(format!("https://{}/.well-known/did.json", host))
    } else {
        let path = parts[1..].join("/");
        Some(format!("https://{}/{}/did.json", host, path))
    }
}

/// Simple in-memory cache for DID resolution
pub struct DidResolver {
    client: Client,
    cache: Mutex<HashMap<String, (String, Instant)>>,
    cache_ttl: Duration,
    pds_map: Mutex<HashMap<String, String>>, // did -> pds endpoint
}

impl DidResolver {
    /// Create new DID resolver
    pub fn new() -> Self {
        let client = crate::http::client_with_timeout(Duration::from_secs(10));

        Self {
            client,
            cache: Mutex::new(HashMap::new()),
            cache_ttl: Duration::from_secs(3600), // 1 hour cache
            pds_map: Mutex::new(HashMap::new()),
        }
    }

    /// Discover the PDS endpoint for a DID by inspecting the PLC audit log
    /// Returns Ok(Some(pds_base_url)) on success, Ok(None) if not found, or Err on network/parse errors
    pub async fn discover_pds(&self, did: &str) -> Result<Option<String>, AppError> {
        // Simple validation
        if !did.starts_with("did:plc:") {
            return Ok(None);
        }

        // PLC audit log URL (public directory)
        let url = format!("https://plc.directory/{}/log/audit", did);
        debug!("Querying PLC audit log for DID {}: {}", did, url);

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            warn!("PLC audit log HTTP {} for {}", resp.status(), did);
            return Ok(None);
        }

        let text = resp.text().await?;
        let v: Value = serde_json::from_str(&text)?;

        // The audit log is typically an array of entries; iterate in reverse to find the most recent PDS endpoint
        if let Some(entries) = v.as_array() {
            for entry in entries.iter().rev() {
                if let Some(op) = entry.get("operation") {
                    if let Some(services) = op.get("services") {
                        if let Some(atp) = services.get("atproto_pds") {
                            if let Some(endpoint) = atp.get("endpoint") {
                                if let Some(endpoint_str) = endpoint.as_str() {
                                    // Ensure scheme
                                    let pds = if endpoint_str.starts_with("http") {
                                        endpoint_str.to_string()
                                    } else {
                                        format!("https://{}", endpoint_str)
                                    };
                                    debug!("Discovered PDS for {}: {}", did, pds);
                                    return Ok(Some(pds));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Resolve handle to DID
    pub async fn resolve_handle(&self, account: &str) -> Result<String, AppError> {
        // If it's already a DID, return it
        if account.starts_with("did:plc:") {
            return Ok(account.to_string());
        }
        if account.starts_with("did:web:") {
            // Attempt to fetch did:web document and cache PDS endpoint if present
            let did = account.to_string();
            if let Err(e) = self.resolve_did_web(&did).await {
                return Err(AppError::DidResolveFailed(format!(
                    "did:web resolution failed: {}",
                    e.message()
                )));
            }
            return Ok(did);
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
        
        // First try the well-known endpoint on the handle domain: https://<handle>/.well-known/atproto-did
        if let Some(did) = self.try_well_known(clean_handle).await? {
            info!("Resolved via .well-known for {} -> {}", clean_handle, did);
            self.cache_result(account, &did).await;
            debug!("Resolved handle {} to DID {}", account, did);
            return Ok(did);
        }

        // Fallback: resolve via XRPC, trying api.bsky.app then bsky.social
        let endpoints = vec![
            "https://api.bsky.app/xrpc/com.atproto.identity.resolveHandle",
            "https://bsky.social/xrpc/com.atproto.identity.resolveHandle",
        ];
        let mut did: Option<String> = None;
        let mut last_err: Option<AppError> = None;
        for base in endpoints.into_iter() {
            let url = format!("{}?handle={}", base, clean_handle);
            debug!("Resolving handle {} via {}", clean_handle, url);
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<ResolveHandleResponse>().await {
                            Ok(res) => { did = Some(res.did); break; },
                            Err(e) => { last_err = Some(AppError::DidResolveFailed(e.to_string())); }
                        }
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        last_err = Some(AppError::DidResolveFailed(format!("HTTP {} from {}: {}", status, base, body)));
                    }
                }
                Err(e) => { last_err = Some(AppError::DidResolveFailed(e.to_string())); }
            }
        }
        let did = did.ok_or_else(|| last_err.unwrap_or(AppError::DidResolveFailed("Unknown handle resolution error".to_string())))?;

        // Validate DID format
        if !did.starts_with("did:plc:") || did.len() != 32 {
            return Err(AppError::DidResolveFailed(format!(
                "Invalid DID format returned: {}",
                did
            )));
        }

        // Cache the result
        self.cache_result(account, &did).await;

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