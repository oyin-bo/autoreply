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

/// Parse various account reference formats into a normalized form
/// Supports: handles, @handles, DIDs, partial DIDs (suffix only), and Bsky.app profile URLs
pub fn parse_account_reference(account: &str) -> String {
    let account = account.trim();

    // Already a full DID
    if account.starts_with("did:") {
        return account.to_string();
    }

    // Remove @ prefix for handles
    if let Some(stripped) = account.strip_prefix('@') {
        return stripped.to_string();
    }

    // Parse Bsky.app profile URLs: https://bsky.app/profile/{handle}
    if account.starts_with("https://bsky.app/profile/")
        || account.starts_with("http://bsky.app/profile/")
    {
        if let Some(profile_part) = account.split("/profile/").nth(1) {
            // Remove trailing slash and any path components
            let handle_or_did = profile_part.split('/').next().unwrap_or(profile_part);
            return handle_or_did.to_string();
        }
    }

    // Detect partial DID suffix (did:plc: is 8 chars, suffix is 24 base32 chars)
    // If we have exactly 24 base32 characters, assume it's a did:plc suffix
    if account.len() == 24 && account.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')) {
        return format!("did:plc:{}", account);
    }

    // Otherwise assume it's a handle
    account.to_string()
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
    /// Now supports multiple account reference formats via parse_account_reference
    pub async fn resolve_handle(&self, handle: &str) -> Result<Option<String>, AppError> {
        // Parse the account reference to normalize it
        let normalized = parse_account_reference(handle);

        // If it's already a DID, return it as-is
        if normalized.starts_with("did:") {
            return Ok(Some(normalized));
        }

        // Basic handle validation
        if !is_valid_handle(&normalized) {
            return Ok(None);
        }

        // Check cache first
        if let Some(cached) = self.get_cached_resolution(&normalized) {
            return Ok(Some(cached));
        }

        // Try direct resolution
        let did = self.try_resolve_handle_direct(&normalized).await?;

        if let Some(ref did_str) = did {
            self.cache_resolution(&normalized, did_str);
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
        // Implement PDS discovery for supported DID methods (did:plc and did:web)
        #[derive(Debug, serde::Deserialize)]
        struct ServiceEndpoint {
            #[serde(default)]
            id: Option<String>,
            #[serde(rename = "type", default)]
            type_field: Option<String>,
            #[serde(rename = "serviceEndpoint")]
            service_endpoint: String,
        }

        #[derive(Debug, serde::Deserialize)]
        struct DidDocument {
            #[serde(default)]
            service: Option<Vec<ServiceEndpoint>>,
        }

        // Helper to inspect service entries
        let extract_pds = |services: Option<Vec<ServiceEndpoint>>| -> Option<String> {
            if let Some(svcs) = services {
                for s in svcs.iter() {
                    if s.type_field.as_deref() == Some("AtprotoPersonalDataServer")
                        || s.id.as_deref() == Some("#atproto_pds")
                    {
                        return Some(s.service_endpoint.clone());
                    }
                }
            }
            None
        };

        // did:plc:<id> -> fetch https://plc.directory/{did}
        if _did.starts_with("did:plc:") {
            let url = construct_pds_endpoint_url(_did);
            let resp = self
                .client
                .get(&url)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|e| AppError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(AppError::DidResolveFailed(format!(
                    "DID document resolution failed with status {}",
                    resp.status()
                )));
            }

            let did_doc: DidDocument = resp.json().await.map_err(|e| {
                AppError::DidResolveFailed(format!("Failed to parse DID document: {}", e))
            })?;

            return Ok(extract_pds(did_doc.service));
        }

        // did:web:<host>[:path...] -> try well-known and did.json locations
        if _did.starts_with("did:web:") {
            let parts: Vec<&str> = _did.split(':').collect();
            if parts.len() < 3 {
                return Ok(None);
            }

            let host = parts[2];
            let mut candidates = Vec::new();
            if parts.len() == 3 {
                candidates.push(format!("https://{}/.well-known/did.json", host));
                candidates.push(format!("https://{}/did.json", host));
            } else {
                let path = parts[3..].join("/");
                candidates.push(format!("https://{}/{}/did.json", host, path));
            }

            let mut last_err: Option<AppError> = None;
            for url in candidates {
                let resp = match self
                    .client
                    .get(&url)
                    .header(
                        reqwest::header::ACCEPT,
                        "application/did+json, application/json",
                    )
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        last_err = Some(AppError::NetworkError(e.to_string()));
                        continue;
                    }
                };

                if !resp.status().is_success() {
                    last_err = Some(AppError::DidResolveFailed(format!(
                        "did:web document fetch failed with status {}",
                        resp.status()
                    )));
                    continue;
                }

                let did_doc: DidDocument = match resp.json().await {
                    Ok(d) => d,
                    Err(e) => {
                        last_err = Some(AppError::DidResolveFailed(format!(
                            "Failed to parse did:web document: {}",
                            e
                        )));
                        continue;
                    }
                };

                if let Some(endpoint) = extract_pds(did_doc.service) {
                    return Ok(Some(endpoint));
                }
            }

            // If we exhausted candidates, return last error if any, otherwise None
            if let Some(err) = last_err {
                return Err(err);
            }
            return Ok(None);
        }

        // Unsupported DID method for PDS discovery
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
    fn test_did_web_to_did_document_url_missing_domain() {
        // "did:web:" creates empty domain, which function doesn't validate
        // It will return Some("https:///.well-known/did.json") per implementation
        assert!(did_web_to_did_document_url("did:web:").is_some());
        assert!(did_web_to_did_document_url("did:web").is_none());
    }

    #[test]
    fn test_did_web_to_did_document_url_complex_path() {
        let url = did_web_to_did_document_url("did:web:example.com:users:profile:alice").unwrap();
        assert_eq!(url, "https://example.com/users/profile/alice/did.json");
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
    fn test_handle_validation_comprehensive() {
        // Valid handles
        assert!(is_valid_handle("alice.bsky.social"));
        assert!(is_valid_handle("bob.example.com"));
        assert!(is_valid_handle("user-name.test.io"));
        assert!(is_valid_handle("123.example.net"));
        assert!(is_valid_handle("a.b.c.d.example.org"));

        // Invalid handles
        assert!(!is_valid_handle(""));
        assert!(!is_valid_handle("nodot"));
        assert!(!is_valid_handle(".invalid"));
        assert!(!is_valid_handle("invalid."));
        assert!(!is_valid_handle("has space.com"));
        assert!(!is_valid_handle("has@symbol.com"));
        assert!(!is_valid_handle("x.y")); // TLD only 1 char
        assert!(!is_valid_handle("..double-dot.com"));
        assert!(!is_valid_handle("empty..part.com"));
    }

    #[test]
    fn test_handle_validation_edge_cases() {
        // Minimum valid handle
        assert!(is_valid_handle("a.bc"));

        // Long but valid
        let long_handle = format!("{}.example.com", "a".repeat(100));
        assert!(is_valid_handle(&long_handle));

        // Special characters
        assert!(is_valid_handle("user-name.example.com"));
        assert!(is_valid_handle("123-456.example.com"));
        assert!(!is_valid_handle("user_name.example.com")); // underscore not allowed
        assert!(!is_valid_handle("user.name!.com")); // exclamation not allowed
    }

    #[test]
    fn test_did_validation() {
        assert!(is_valid_did("did:plc:abcd1234efgh5678"));
        assert!(is_valid_did("did:web:example.com"));
        assert!(!is_valid_did("not-a-did"));
        assert!(!is_valid_did(""));
    }

    #[test]
    fn test_did_validation_comprehensive() {
        // Valid DIDs with different methods
        assert!(is_valid_did("did:plc:abcd1234efgh5678"));
        assert!(is_valid_did("did:web:example.com"));
        assert!(is_valid_did(
            "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"
        ));
        assert!(is_valid_did("did:method:suffix"));
        assert!(is_valid_did("did:x:y")); // Minimal valid
        assert!(is_valid_did("did:plc")); // Actually valid per implementation (> 4 chars)

        // Invalid DIDs
        assert!(!is_valid_did(""));
        assert!(!is_valid_did("did"));
        assert!(!is_valid_did("did:"));
        assert!(!is_valid_did("DID:plc:abcd")); // Case sensitive
        assert!(!is_valid_did("not-a-did"));
        assert!(!is_valid_did("notdid:plc:abc"));
    }

    #[test]
    fn test_parse_account_reference_did_passthrough() {
        let did = "did:plc:abcd1234efgh5678";
        assert_eq!(parse_account_reference(did), did);

        let did_web = "did:web:example.com";
        assert_eq!(parse_account_reference(did_web), did_web);
    }

    #[test]
    fn test_parse_account_reference_handle() {
        assert_eq!(
            parse_account_reference("alice.bsky.social"),
            "alice.bsky.social"
        );
        assert_eq!(
            parse_account_reference("user.example.com"),
            "user.example.com"
        );
    }

    #[test]
    fn test_parse_account_reference_at_prefix() {
        assert_eq!(
            parse_account_reference("@alice.bsky.social"),
            "alice.bsky.social"
        );
        assert_eq!(
            parse_account_reference("@user.example.com"),
            "user.example.com"
        );
    }

    #[test]
    fn test_parse_account_reference_bsky_url() {
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/alice.bsky.social"),
            "alice.bsky.social"
        );
        assert_eq!(
            parse_account_reference("http://bsky.app/profile/bob.example.com"),
            "bob.example.com"
        );

        // With DID in URL
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/did:plc:abc123"),
            "did:plc:abc123"
        );
    }

    #[test]
    fn test_parse_account_reference_bsky_url_with_path() {
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/alice.bsky.social/post/123"),
            "alice.bsky.social"
        );
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/user.com/feed/likes"),
            "user.com"
        );
    }

    #[test]
    fn test_parse_account_reference_partial_did() {
        // 24 base32 characters
        let partial = "abcdefg234567hijklmn2345";
        assert_eq!(
            parse_account_reference(partial),
            format!("did:plc:{}", partial)
        );

        // Not exactly 24 chars - treated as handle
        let not_partial = "abcdefg234567hijklmn234";
        assert_eq!(parse_account_reference(not_partial), not_partial);
    }

    #[test]
    fn test_parse_account_reference_partial_did_validation() {
        // Valid base32: only a-z and 2-7
        let valid_partial = "a2b3c4d5e6f7g2h3i4j5k6l7";
        assert_eq!(
            parse_account_reference(valid_partial),
            format!("did:plc:{}", valid_partial)
        );

        // Invalid characters (has 8, 9, uppercase)
        let invalid_partial1 = "abcdefg234567hijklmn8945";
        assert_eq!(parse_account_reference(invalid_partial1), invalid_partial1);

        let invalid_partial2 = "ABCDEFG234567HIJKLMN2345";
        assert_eq!(parse_account_reference(invalid_partial2), invalid_partial2);

        // Has special characters
        let invalid_partial3 = "abcdefg-34567hijklmn2345";
        assert_eq!(parse_account_reference(invalid_partial3), invalid_partial3);
    }

    #[test]
    fn test_parse_account_reference_whitespace() {
        assert_eq!(
            parse_account_reference("  alice.bsky.social  "),
            "alice.bsky.social"
        );
        assert_eq!(
            parse_account_reference(" did:plc:abc123 "),
            "did:plc:abc123"
        );
    }

    #[test]
    fn test_parse_account_reference_edge_cases() {
        // Empty string after trim
        assert_eq!(parse_account_reference(""), "");

        // Just @ - strip_prefix returns empty string
        assert_eq!(parse_account_reference("@"), "");

        // URL without profile part - doesn't match pattern, returned as-is
        assert_eq!(
            parse_account_reference("https://bsky.app/"),
            "https://bsky.app/"
        );

        // Malformed URL - split returns empty after "/profile/"
        // Actually the implementation uses split('/').next() which returns ""
        assert_eq!(parse_account_reference("https://bsky.app/profile/"), "");
    }

    #[test]
    fn test_construct_well_known_url() {
        assert_eq!(
            construct_well_known_url("example.com"),
            "https://example.com/.well-known/atproto-did"
        );
        assert_eq!(
            construct_well_known_url("alice.bsky.social"),
            "https://alice.bsky.social/.well-known/atproto-did"
        );
    }

    #[test]
    fn test_construct_xrpc_resolve_url() {
        assert_eq!(
            construct_xrpc_resolve_url("bsky.social"),
            "https://bsky.social/xrpc/com.atproto.identity.resolveHandle"
        );
    }

    #[test]
    fn test_construct_plc_audit_url() {
        assert_eq!(
            construct_plc_audit_url("did:plc:abc123"),
            "https://plc.directory/did:plc:abc123/log/audit"
        );
    }

    #[test]
    fn test_construct_pds_endpoint_url() {
        assert_eq!(
            construct_pds_endpoint_url("did:plc:abc123"),
            "https://plc.directory/did:plc:abc123"
        );
        assert_eq!(
            construct_pds_endpoint_url("did:web:example.com"),
            "https://plc.directory/did:web:example.com"
        );
    }

    #[tokio::test]
    async fn test_did_resolver_creation() {
        let _resolver = DidResolver::new();
        // Just test that we can create it without panicking
    }

    #[tokio::test]
    async fn test_did_resolver_default() {
        let resolver = DidResolver::default();
        // Test that default() works
        assert!(resolver.client.get("https://example.com").build().is_ok());
    }

    #[tokio::test]
    async fn test_resolve_handle_plc_did_passthrough() {
        let resolver = DidResolver::new();
        let did = "did:plc:abcd1234efgh5678";
        let result = resolver.resolve_handle(did).await;
        assert!(matches!(result, Ok(Some(resolved_did)) if resolved_did == did));
    }

    #[tokio::test]
    async fn test_resolve_handle_web_did_passthrough() {
        let resolver = DidResolver::new();
        let did = "did:web:example.com";
        let result = resolver.resolve_handle(did).await;
        assert!(matches!(result, Ok(Some(resolved_did)) if resolved_did == did));
    }

    #[tokio::test]
    async fn test_resolve_handle_invalid() {
        let resolver = DidResolver::new();

        // Invalid handles should return None
        let result = resolver.resolve_handle("not_a_handle").await;
        assert!(matches!(result, Ok(None)));

        let result = resolver.resolve_handle("").await;
        assert!(matches!(result, Ok(None)));

        let result = resolver.resolve_handle("x.y").await; // TLD too short
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn test_resolve_handle_with_at_prefix() {
        let resolver = DidResolver::new();
        // This will fail to resolve but shouldn't error
        let result = resolver
            .resolve_handle("@nonexistent.example.invalid")
            .await;
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn test_resolve_handle_partial_did() {
        let resolver = DidResolver::new();
        let partial = "a2b3c4d5e6f7g2h3i4j5k6l7";
        let result = resolver.resolve_handle(partial).await;
        // Should convert to full DID and return it
        assert!(matches!(result, Ok(Some(did)) if did.starts_with("did:plc:")));
    }

    #[tokio::test]
    async fn test_resolve_handle_bsky_url() {
        let resolver = DidResolver::new();
        let url = "https://bsky.app/profile/did:plc:abc123";
        let result = resolver.resolve_handle(url).await;
        assert!(matches!(result, Ok(Some(did)) if did == "did:plc:abc123"));
    }

    #[test]
    fn test_is_valid_handle_internationalized_domain() {
        // ASCII-only for now in this implementation
        assert!(is_valid_handle("user.example.com"));

        // Punycode would be valid but testing basic ASCII validation
        assert!(is_valid_handle("xn--user-xxa.example.com"));
    }

    #[test]
    fn test_is_valid_handle_multiple_subdomains() {
        assert!(is_valid_handle("alice.staging.bsky.social"));
        assert!(is_valid_handle("a.b.c.d.e.example.com"));
    }

    #[test]
    fn test_is_valid_handle_numeric() {
        assert!(is_valid_handle("123.456.example.com"));
        assert!(is_valid_handle("user123.example.com"));
        assert!(is_valid_handle("123user.example.com"));
    }

    #[test]
    fn test_is_valid_did_methods() {
        assert!(is_valid_did("did:plc:anything"));
        assert!(is_valid_did("did:web:example.com"));
        assert!(is_valid_did("did:key:base58key"));
        assert!(is_valid_did("did:peer:anything"));
        assert!(is_valid_did("did:ethr:0xaddress"));
        assert!(is_valid_did("did:ion:anything"));
    }

    #[test]
    fn test_parse_account_reference_case_sensitivity() {
        // DIDs are case-sensitive
        assert_eq!(parse_account_reference("did:plc:ABC123"), "did:plc:ABC123");

        // Handles are lowercase by convention but we preserve them
        assert_eq!(
            parse_account_reference("Alice.Example.COM"),
            "Alice.Example.COM"
        );
    }

    #[test]
    fn test_parse_account_reference_complex_urls() {
        // URL with query params - implementation doesn't strip query params
        // The split('/').next() gets "alice.com?param=value"
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/alice.com?param=value"),
            "alice.com?param=value"
        );

        // URL with hash - implementation doesn't strip hash
        assert_eq!(
            parse_account_reference("https://bsky.app/profile/alice.com#section"),
            "alice.com#section"
        );
    }
}
