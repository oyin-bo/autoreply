//! DID resolution functionality
//!
//! Handles resolving Bluesky handles to DIDs via XRPC

#![allow(clippy::items_after_test_module)]
#![allow(dead_code)]

use crate::error::AppError;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::Mutex;

/// DID resolution response from XRPC
#[derive(Debug, Deserialize)]
struct ResolveHandleResponse {
    did: String,
}

/// Check if a string looks like a valid handle
pub fn is_valid_handle(handle: &str) -> bool {
    // Basic validation - handle must have at least one dot and proper format
    if handle.is_empty() || !handle.contains('.') {
        return false;
    }

    let parts: Vec<&str> = handle.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // Check each part is non-empty and contains only valid characters
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }

    // Last part (TLD) should be at least 2 characters
    if let Some(tld) = parts.last() {
        if tld.len() < 2 {
            return false;
        }
    }

    true
}

/// Check if a string looks like a valid DID
pub fn is_valid_did(did: &str) -> bool {
    did.starts_with("did:") && did.len() > 4
}

/// Main DID resolver struct
pub struct DidResolver {
    client: Client,
    cache: std::sync::Arc<Mutex<HashMap<String, (String, Instant)>>>,
}

impl DidResolver {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Resolve handle to DID
    pub async fn resolve_handle(&self, handle: &str) -> Result<Option<String>, AppError> {
        // If it's already a DID, return it as-is
        if handle.starts_with("did:") {
            return Ok(Some(handle.to_string()));
        }

        // Basic handle validation
        if !is_valid_handle(handle) {
            return Ok(None);
        }

        // Check cache first
        if let Some(cached) = self.get_cached_resolution(handle) {
            return Ok(Some(cached));
        }

        // Try direct resolution
        let did = self.try_resolve_handle_direct(handle).await?;

        if let Some(ref did_str) = did {
            self.cache_resolution(handle, did_str);
        }

        Ok(did)
    }

    async fn try_resolve_handle_direct(&self, handle: &str) -> Result<Option<String>, AppError> {
        let url = format!(
            "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle={}",
            handle
        );

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<ResolveHandleResponse>().await {
                    Ok(resolve_response) => Ok(Some(resolve_response.did)),
                    Err(_) => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn get_cached_resolution(&self, _handle: &str) -> Option<String> {
        // For now, simplified without async locking
        None
    }

    fn cache_resolution(&self, _handle: &str, _did: &str) {
        // For now, simplified without async locking
    }

    /// Discover PDS endpoint for a DID
    pub async fn discover_pds(&self, _did: &str) -> Result<Option<String>, AppError> {
        // Simplified implementation
        Ok(None)
    }
}

impl Default for DidResolver {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for URL construction
fn construct_well_known_url(domain: &str) -> String {
    format!("https://{}/.well-known/atproto-did", domain)
}

fn construct_xrpc_resolve_url(domain: &str) -> String {
    format!("https://{}/xrpc/com.atproto.identity.resolveHandle", domain)
}

fn construct_plc_audit_url(did: &str) -> String {
    format!("https://plc.directory/{}/log/audit", did)
}

fn construct_pds_endpoint_url(did: &str) -> String {
    format!("https://plc.directory/{}", did)
}

fn did_web_to_did_document_url(did: &str) -> Option<String> {
    if !did.starts_with("did:web:") {
        return None;
    }

    let parts: Vec<&str> = did.split(':').collect();
    if parts.len() < 3 {
        return None;
    }

    let domain_and_path = &parts[2..];
    let domain = domain_and_path[0];

    if domain_and_path.len() == 1 {
        // Root domain case
        Some(format!("https://{}/.well-known/did.json", domain))
    } else {
        // Path case
        let path = domain_and_path[1..].join("/");
        Some(format!("https://{}/{}/did.json", domain, path))
    }
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
    fn test_did_web_to_did_document_url_invalid() {
        assert!(did_web_to_did_document_url("invalid:did").is_none());
        assert!(did_web_to_did_document_url("did:other:example.com").is_none());
    }

    #[test]
    fn test_handle_validation_logic() {
        assert!(is_valid_handle("alice.bsky.social"));
        assert!(is_valid_handle("user.example.com"));
        assert!(!is_valid_handle("not_a_handle"));
        assert!(!is_valid_handle(""));
        assert!(!is_valid_handle("handle.c")); // TLD too short
    }

    #[test]
    fn test_did_validation() {
        assert!(is_valid_did("did:plc:abcd1234efgh5678"));
        assert!(is_valid_did("did:web:example.com"));
        assert!(!is_valid_did("not-a-did"));
        assert!(!is_valid_did(""));
    }

    #[tokio::test]
    async fn test_did_resolver_creation() {
        let _resolver = DidResolver::new();
        // Just test that we can create it without panicking
    }

    #[tokio::test]
    async fn test_resolve_handle_plc_did_passthrough() {
        let resolver = DidResolver::new();
        let did = "did:plc:abcd1234efgh5678";
        let result = resolver.resolve_handle(did).await;
        assert!(matches!(result, Ok(Some(resolved_did)) if resolved_did == did));
    }
}
