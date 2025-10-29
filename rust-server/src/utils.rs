//! Utility functions for URI parsing and manipulation

use crate::error::AppError;

/// Parse a post URI or URL and extract the DID and record key
/// Supports both at:// URIs and https://bsky.app/... URLs
#[derive(Debug, Clone, PartialEq)]
pub struct PostRef {
    pub did: String,
    pub rkey: String,
}

impl PostRef {
    /// Parse from at:// URI format: at://did:plc:xyz/app.bsky.feed.post/abc123
    /// or https://bsky.app/profile/handle.bsky.social/post/abc123
    pub fn parse(uri_or_url: &str) -> Result<Self, AppError> {
        // Try parsing as at:// URI first
        if let Some(at_uri) = uri_or_url.strip_prefix("at://") {
            let parts: Vec<&str> = at_uri.split('/').collect();
            if parts.len() >= 3 {
                let did = parts[0].to_string();
                let rkey = parts[2].to_string();
                
                // Validate DID format
                if !did.starts_with("did:") {
                    return Err(AppError::ParseError(format!(
                        "Invalid DID in URI: {}",
                        did
                    )));
                }
                
                return Ok(PostRef { did, rkey });
            }
            return Err(AppError::ParseError(format!(
                "Invalid at:// URI format: {}",
                uri_or_url
            )));
        }

        // Try parsing as https://bsky.app URL
        if let Some(rest) = uri_or_url.strip_prefix("https://bsky.app/profile/") {
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() >= 3 && parts[1] == "post" {
                // We have handle and rkey, but need to resolve handle to DID later
                // For now, return the handle as "did" - caller must resolve it
                let handle = parts[0].to_string();
                let rkey = parts[2].to_string();
                return Ok(PostRef {
                    did: handle,
                    rkey,
                });
            }
            return Err(AppError::ParseError(format!(
                "Invalid bsky.app URL format: {}",
                uri_or_url
            )));
        }

        Err(AppError::ParseError(format!(
            "URI must be at:// or https://bsky.app/... format: {}",
            uri_or_url
        )))
    }

    /// Check if the DID is actually a handle that needs resolution
    pub fn needs_did_resolution(&self) -> bool {
        !self.did.starts_with("did:")
    }
}

/// Construct an at:// URI from DID and record key
pub fn make_at_uri(did: &str, collection: &str, rkey: &str) -> String {
    format!("at://{}/{}/{}", did, collection, rkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_at_uri() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789";
        let result = PostRef::parse(uri).unwrap();
        assert_eq!(result.did, "did:plc:abc123");
        assert_eq!(result.rkey, "xyz789");
        assert!(!result.needs_did_resolution());
    }

    #[test]
    fn test_parse_bsky_url() {
        let url = "https://bsky.app/profile/alice.bsky.social/post/xyz789";
        let result = PostRef::parse(url).unwrap();
        assert_eq!(result.did, "alice.bsky.social");
        assert_eq!(result.rkey, "xyz789");
        assert!(result.needs_did_resolution());
    }

    #[test]
    fn test_parse_invalid_uri() {
        let uri = "https://example.com/post/123";
        assert!(PostRef::parse(uri).is_err());

        let uri = "at://invalid";
        assert!(PostRef::parse(uri).is_err());

        let uri = "at://not-a-did/collection/rkey";
        assert!(PostRef::parse(uri).is_err());
    }

    #[test]
    fn test_make_at_uri() {
        let uri = make_at_uri("did:plc:abc123", "app.bsky.feed.post", "xyz789");
        assert_eq!(uri, "at://did:plc:abc123/app.bsky.feed.post/xyz789");
    }
}
