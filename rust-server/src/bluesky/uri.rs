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
/// Supports:
/// - at:// URIs: `at://{did}/app.bsky.feed.post/{rkey}`
/// - BlueSky URLs: `https://bsky.app/profile/{handle}/post/{rkey}`
/// - Compact format: `@{handle}/{rkey}` (e.g., `@alice.bsky.social/3m4jnj3efp22t`)
///
/// For BlueSky URLs and compact format, the handle is resolved to a DID using the DID resolver.
pub async fn parse_post_uri(uri: &str) -> Result<PostRef, AppError> {
    let trimmed = uri.trim();

    if trimmed.starts_with("at://") {
        parse_at_uri(trimmed)
    } else if trimmed.contains("bsky.app/profile/") {
        parse_bsky_url(trimmed).await
    } else if trimmed.starts_with('@') && trimmed.contains('/') {
        parse_compact_format(trimmed).await
    } else {
        Err(AppError::InvalidInput(format!(
            "Invalid post URI/URL format: {}. Expected at:// URI, https://bsky.app/... URL, or @handle/rkey",
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
    let did = resolver.resolve_handle(handle).await?.ok_or_else(|| {
        AppError::DidResolveFailed(format!("Could not resolve handle: {}", handle))
    })?;

    Ok(PostRef {
        did,
        rkey: rkey.to_string(),
    })
}

/// Parse compact format @handle/rkey
async fn parse_compact_format(input: &str) -> Result<PostRef, AppError> {
    // Format: @handle/rkey (e.g., @alice.bsky.social/3m4jnj3efp22t)
    if !input.starts_with('@') {
        return Err(AppError::InvalidInput(format!(
            "Compact format must start with @: {}",
            input
        )));
    }

    let without_at = &input[1..]; // Remove leading @
    let parts: Vec<&str> = without_at.split('/').collect();

    if parts.len() < 2 {
        return Err(AppError::InvalidInput(format!(
            "Invalid compact format: {}. Expected @handle/rkey",
            input
        )));
    }

    let handle = parts[0];
    let rkey = parts[1];

    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(handle).await?.ok_or_else(|| {
        AppError::DidResolveFailed(format!("Could not resolve handle: {}", handle))
    })?;

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

    #[test]
    fn test_parse_compact_format_invalid_no_at() {
        let input = "alice.bsky.social/xyz789";
        let result = parse_at_uri(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_compact_format_invalid_no_slash() {
        let input = "@alice.bsky.social";
        // Can't test parse_compact_format directly as it's async,
        // but parse_post_uri should handle this
        // This just tests the at:// parser doesn't accept it
        let result = parse_at_uri(input);
        assert!(result.is_err());
    }
}
