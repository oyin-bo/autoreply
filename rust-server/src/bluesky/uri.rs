//! BlueSky URI parsing utilities

use crate::bluesky::did::DidResolver;
use crate::error::AppError;

/// Parsed post reference containing DID and record key
#[derive(Debug, Clone)]
pub struct PostRef {
    pub did: String,
    pub rkey: String,
}

/// Parse a post URI or URL into a PostRef
///
/// Supports both:
/// - at:// URIs: `at://{did}/app.bsky.feed.post/{rkey}`
/// - BlueSky URLs: `https://bsky.app/profile/{handle}/post/{rkey}`
///
/// For BlueSky URLs, the handle is resolved to a DID using the DID resolver.
pub async fn parse_post_uri(uri: &str) -> Result<PostRef, AppError> {
    if uri.starts_with("at://") {
        parse_at_uri(uri)
    } else if uri.contains("bsky.app/profile/") {
        parse_bsky_url(uri).await
    } else {
        Err(AppError::InvalidInput(format!(
            "Invalid post URI/URL format: {}. Expected at:// URI or https://bsky.app/... URL",
            uri
        )))
    }
}

/// Parse an at:// URI
fn parse_at_uri(uri: &str) -> Result<PostRef, AppError> {
    // Format: at://{did}/app.bsky.feed.post/{rkey}
    let parts: Vec<&str> = uri.trim_start_matches("at://").split('/').collect();
    
    if parts.len() < 3 {
        return Err(AppError::InvalidInput(format!(
            "Invalid at:// URI format: {}. Expected at://{{did}}/app.bsky.feed.post/{{rkey}}",
            uri
        )));
    }

    Ok(PostRef {
        did: parts[0].to_string(),
        rkey: parts[2].to_string(),
    })
}

/// Parse a BlueSky app URL
async fn parse_bsky_url(url: &str) -> Result<PostRef, AppError> {
    // Format: https://bsky.app/profile/{handle}/post/{rkey}
    let url_parts: Vec<&str> = url.split('/').collect();
    
    if url_parts.len() < 7 {
        return Err(AppError::InvalidInput(format!(
            "Invalid bsky.app URL format: {}. Expected https://bsky.app/profile/{{handle}}/post/{{rkey}}",
            url
        )));
    }

    let handle = url_parts[4];
    let rkey = url_parts[6];

    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver
        .resolve_handle(handle)
        .await?
        .ok_or_else(|| AppError::DidResolveFailed(format!("Could not resolve handle: {}", handle)))?;

    Ok(PostRef {
        did,
        rkey: rkey.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_at_uri_valid() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789";
        let result = parse_at_uri(uri).unwrap();
        assert_eq!(result.did, "did:plc:abc123");
        assert_eq!(result.rkey, "xyz789");
    }

    #[test]
    fn test_parse_at_uri_invalid() {
        let uri = "at://did:plc:abc123";
        let result = parse_at_uri(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_at_uri_with_extra_path() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789/extra";
        let result = parse_at_uri(uri).unwrap();
        assert_eq!(result.did, "did:plc:abc123");
        assert_eq!(result.rkey, "xyz789");
    }
}
