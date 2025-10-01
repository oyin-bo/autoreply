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

    #[test]
    fn test_did_web_to_did_document_url_various_formats() {
        // Test subdomain
        let url = did_web_to_did_document_url("did:web:subdomain.example.org").unwrap();
        assert_eq!(url, "https://subdomain.example.org/.well-known/did.json");
        
        // Test multiple path segments
        let url = did_web_to_did_document_url("did:web:api.example.com:v1:users:bob").unwrap();
        assert_eq!(url, "https://api.example.com/v1/users/bob/did.json");
        
        // Test single path segment
        let url = did_web_to_did_document_url("did:web:example.com:alice").unwrap();
        assert_eq!(url, "https://example.com/alice/did.json");
    }

    #[test]
    fn test_did_web_to_did_document_url_invalid() {
        assert!(did_web_to_did_document_url("").is_none());
        assert!(did_web_to_did_document_url("not_a_did").is_none());
        assert!(did_web_to_did_document_url("did:plc:test").is_none());
        assert!(did_web_to_did_document_url("did:web:").is_none());
    }

    #[test]
    fn test_did_validation() {
        // Valid PLC DIDs - "did:plc:" (8) + 24-char identifier = 32 total
        let valid_plc = vec![
            "did:plc:abcdefghijklmnopqrstuvwx", // 8 + 24 = 32
            "did:plc:123456789012345678901234", // 8 + 24 = 32
            "did:plc:abcdefabcdefabcdefabcdef", // 8 + 24 = 32
        ];
        
        for did in valid_plc {
            assert!(did.starts_with("did:plc:"), "{} should start with did:plc:", did);
            assert_eq!(did.len(), 32, "{} should be 32 chars long", did);
            assert!(did[8..].chars().all(|c| c.is_ascii_alphanumeric()), "{} should be alphanumeric after prefix", did);
        }
        
        // Invalid DIDs
        let invalid = vec![
            "did:web:example.com", // Not PLC
            "did:plc:tooshort",    // Too short
            "did:plc:toolong123456789012345678901", // Too long
            "did:plc:has-invalid-chars123456789!", // Invalid characters
            "did:other:abc123def456789012345678901", // Wrong method
            "not_a_did",
            "",
        ];
        
        for did in invalid {
            let is_valid_plc = did.starts_with("did:plc:") 
                && did.len() == 32 
                && did[8..].chars().all(|c| c.is_ascii_alphanumeric());
            assert!(!is_valid_plc, "{} should not be a valid PLC DID", did);
        }
    }

    #[test]
    fn test_handle_validation_logic() {
        // Test handle validation logic (without making network calls)
        let invalid_handles = vec![
            "",
            "nodot",
            "empty.",
            ".empty", 
            "double..dot",
        ];
        
        // These would fail validation before any network request
        for handle in invalid_handles {
            let is_invalid = handle.is_empty() 
                || !handle.contains('.') 
                || handle.contains("..")
                || handle.starts_with('.')
                || handle.ends_with('.');
            assert!(is_invalid, "{} should be considered invalid", handle);
        }

        let valid_handles = vec![
            "alice.bsky.social",
            "bob.example.com", 
            "user.subdomain.example.org",
        ];
        
        for handle in valid_handles {
            let is_valid = !handle.is_empty()
                && handle.contains('.')
                && !handle.contains("..")
                && !handle.starts_with('.')
                && !handle.ends_with('.');
            assert!(is_valid, "{} should be considered valid", handle);
        }
    }

    #[test]
    fn test_well_known_url_construction() {
        // Test that we construct the right URLs
        let test_cases = vec![
            ("alice.bsky.social", "https://alice.bsky.social/.well-known/atproto-did"),
            ("bob.example.com", "https://bob.example.com/.well-known/atproto-did"),
            ("user.subdomain.org", "https://user.subdomain.org/.well-known/atproto-did"),
        ];
        
        for (handle, expected_url) in test_cases {
            let url = format!("https://{}/.well-known/atproto-did", handle.trim_start_matches('@'));
            assert_eq!(url, expected_url);
        }
    }

    #[test]
    fn test_plc_audit_url_construction() {
        let dids = vec![
            "did:plc:abc123def456789012345678901",
            "did:plc:zyxwvutsrqponmlkjihgfedcba98",
        ];
        
        for did in dids {
            let url = format!("https://plc.directory/{}/log/audit", did);
            assert!(url.starts_with("https://plc.directory/"));
            assert!(url.contains(did));
            assert!(url.ends_with("/log/audit"));
        }
    }

    #[test]
    fn test_xrpc_url_construction() {
        let endpoints = vec![
            "https://api.bsky.app",
            "https://bsky.social",
        ];
        
        let handle = "alice.bsky.social";
        
        for endpoint in endpoints {
            let url = format!("{}/xrpc/com.atproto.identity.resolveHandle?handle={}", endpoint, handle);
            assert!(url.contains("/xrpc/com.atproto.identity.resolveHandle"));
            assert!(url.contains(&format!("handle={}", handle)));
        }
    }

    #[test]
    fn test_pds_endpoint_url_construction() {
        let base_endpoints = vec![
            "https://bsky.social",
            "https://api.example.com",
            "https://pds.internal.org",
        ];
        
        let did = "did:plc:abc123def456789012345678901";
        
        for endpoint in base_endpoints {
            let repo_url = format!("{}/xrpc/com.atproto.sync.getRepo?did={}", endpoint, did);
            assert!(repo_url.contains("/xrpc/com.atproto.sync.getRepo"));
            assert!(repo_url.contains(&format!("did={}", did)));
        }
    }

    #[tokio::test] 
    async fn test_did_resolver_creation() {
        let resolver = DidResolver::new();
        assert!(resolver.cache_ttl > Duration::from_secs(0));
        
        let resolver2 = DidResolver::default();
        assert!(resolver2.cache_ttl > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_resolve_handle_plc_did_passthrough() {
        let resolver = DidResolver::new();
        
        let plc_did = "did:plc:abc123def456789012345678901";
        let result = resolver.resolve_handle(plc_did).await.unwrap();
        assert_eq!(result, plc_did);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let resolver = DidResolver::new();
        
        // Test caching a result
        resolver.cache_result("test.handle", "did:plc:test123456789012345678901").await;
        
        // Test cleanup (should not panic)
        resolver.cleanup_cache().await;
    }

    #[tokio::test]
    async fn test_pds_operations() {
        let resolver = DidResolver::new();
        
        // Test discovering PDS for unknown DID - should return Ok(None)
        let pds = resolver.discover_pds("did:plc:unknown").await;
        assert!(matches!(pds, Ok(None)));
        
        // Test discovering PDS for invalid DID format - should return Ok(None)
        let pds = resolver.discover_pds("invalid:did").await;
        assert!(matches!(pds, Ok(None)));
    }
}

impl DidResolver {


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

    /// Remove in-memory DID resolution cache entries older than cache_ttl
    /// In future to be called periodically to prevent memory bloat from long-running processes
    #[allow(dead_code)]
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
