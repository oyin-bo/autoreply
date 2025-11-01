//! Utility functions for tools

/// Convert AT URI to BlueSky web URL
/// at://did:plc:abc/app.bsky.feed.post/xyz -> https://bsky.app/profile/handle/post/xyz
/// Falls back to using DID in the URL if handle is empty
#[allow(dead_code)]
pub fn at_uri_to_bsky_url(at_uri: &str, handle: &str) -> String {
    // Parse AT URI: at://{did}/{collection}/{rkey}
    if !at_uri.starts_with("at://") {
        return at_uri.to_string();
    }

    let parts: Vec<&str> = at_uri.trim_start_matches("at://").split('/').collect();
    if parts.len() < 3 {
        return at_uri.to_string();
    }

    // parts[0] = DID
    // parts[1] = collection (e.g., app.bsky.feed.post)
    // parts[2] = rkey
    let did = parts[0];
    let rkey = parts[2];

    // Use handle if available, otherwise use DID as fallback
    let profile = if handle.is_empty() {
        did
    } else {
        handle
    };

    format!("https://bsky.app/profile/{}/post/{}", profile, rkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_at_uri_to_bsky_url_with_handle() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789";
        let handle = "alice.bsky.social";
        let result = at_uri_to_bsky_url(uri, handle);
        assert_eq!(result, "https://bsky.app/profile/alice.bsky.social/post/xyz789");
    }

    #[test]
    fn test_at_uri_to_bsky_url_without_handle() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post/xyz789";
        let handle = "";
        let result = at_uri_to_bsky_url(uri, handle);
        assert_eq!(result, "https://bsky.app/profile/did:plc:abc123/post/xyz789");
    }

    #[test]
    fn test_at_uri_to_bsky_url_invalid_uri() {
        let uri = "invalid://something";
        let handle = "alice.bsky.social";
        let result = at_uri_to_bsky_url(uri, handle);
        assert_eq!(result, uri);
    }

    #[test]
    fn test_at_uri_to_bsky_url_incomplete_uri() {
        let uri = "at://did:plc:abc123/app.bsky.feed.post";
        let handle = "alice.bsky.social";
        let result = at_uri_to_bsky_url(uri, handle);
        assert_eq!(result, uri);
    }
}
