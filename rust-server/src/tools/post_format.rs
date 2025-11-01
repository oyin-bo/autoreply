//! Post formatting utilities for consistent Markdown output
//!
//! Implements the formatting spec from docs/16-mcp-schemas.md:
//! - Blockquoted user content (prefix with `> `)
//! - Compact post IDs (@handle/rkey or @h/…last4)
//! - Emoji stats (👍 likes  ♻️ reshares  💬 replies)
//! - Threading indicators (└─ with indentation)
//! - ISO timestamps without milliseconds

use std::collections::HashMap;

/// Compact a post ID for display
/// - First mention: @handle/rkey
/// - Subsequent mentions in thread: @firstletter/…last4
pub fn compact_post_id(handle: &str, rkey: &str, seen_posts: &HashMap<String, String>) -> String {
    let full_id = format!("{}/{}", handle, rkey);
    
    // Check if we've seen this post before
    if seen_posts.contains_key(&full_id) {
        ultra_compact_id(handle, rkey)
    } else {
        // First mention - use full format
        format!("@{}/{}", handle, rkey)
    }
}

/// Ultra-compact format for reply-to references
/// @firstletter/…last4
pub fn ultra_compact_id(handle: &str, rkey: &str) -> String {
    let first_letter = handle.chars().next().unwrap_or('?');
    let last_four = if rkey.len() > 4 {
        &rkey[rkey.len()-4..]
    } else {
        rkey
    };
    format!("@{}/…{}", first_letter, last_four)
}

/// Blockquote user content - prefix every line with `> `
pub fn blockquote_content(text: &str) -> String {
    if text.is_empty() {
        return "> \n".to_string();
    }
    
    text.lines()
        .map(|line| format!("> {}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format stats with emojis
/// ♻️ combines reposts + quotes
/// Only shows non-zero stats
pub fn format_stats(likes: i32, reposts: i32, quotes: i32, replies: i32) -> String {
    let mut parts = Vec::new();
    
    if likes > 0 {
        parts.push(format!("👍 {}", likes));
    }
    
    // Combine reposts and quotes into ♻️
    let reshares = reposts + quotes;
    if reshares > 0 {
        parts.push(format!("♻️ {}", reshares));
    }
    
    if replies > 0 {
        parts.push(format!("💬 {}", replies));
    }
    
    parts.join("  ")
}

/// Format timestamp - ISO 8601 without milliseconds, with Z suffix
pub fn format_timestamp(timestamp: &str) -> String {
    // Remove milliseconds if present and ensure Z suffix
    if let Some(dot_pos) = timestamp.find('.') {
        let before_dot = &timestamp[..dot_pos];
        // Timestamp format: 2024-10-06T10:15:33.123Z -> 2024-10-06T10:15:33Z
        format!("{}Z", before_dot)
    } else if timestamp.ends_with('Z') {
        timestamp.to_string()
    } else {
        format!("{}Z", timestamp.trim_end_matches('+').split('+').next().unwrap_or(timestamp))
    }
}

/// Extract rkey from at:// URI
/// at://did:plc:abc123/app.bsky.feed.post/3m4jnj3efp22t -> 3m4jnj3efp22t
pub fn extract_rkey(uri: &str) -> &str {
    if uri.is_empty() {
        return "unknown";
    }
    uri.split('/').last().unwrap_or("unknown")
}

/// Build threading indicator with indentation
/// depth=0: no prefix (root post)
/// depth=1: "└─"
/// depth=2: "  └─"
/// depth=3: "    └─"
pub fn threading_indicator(depth: usize, reply_to_compact: &str, author_id: &str) -> String {
    if depth == 0 {
        // Root post - no indicator, just the author ID
        author_id.to_string()
    } else {
        let indent = "  ".repeat(depth - 1);
        format!("{}└─{} → {}", indent, reply_to_compact, author_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_post_id_first_mention() {
        let seen = HashMap::new();
        let result = compact_post_id("alice.bsky.social", "3kq8a3f1", &seen);
        assert_eq!(result, "@alice.bsky.social/3kq8a3f1");
    }

    #[test]
    fn test_compact_post_id_subsequent() {
        let mut seen = HashMap::new();
        seen.insert("alice.bsky.social/3kq8a3f1".to_string(), "1".to_string());
        let result = compact_post_id("alice.bsky.social", "3kq8a3f1", &seen);
        assert_eq!(result, "@a/…a3f1");
    }

    #[test]
    fn test_ultra_compact_id() {
        assert_eq!(ultra_compact_id("alice.bsky.social", "3kq8a3f1"), "@a/…a3f1");
        assert_eq!(ultra_compact_id("bob", "3kq8b2e4"), "@b/…b2e4");
        assert_eq!(ultra_compact_id("carol-long-handle", "abc"), "@c/…abc");
    }

    #[test]
    fn test_blockquote_content_simple() {
        assert_eq!(blockquote_content("Hello world"), "> Hello world");
    }

    #[test]
    fn test_blockquote_content_multiline() {
        let input = "Line 1\nLine 2\nLine 3";
        let expected = "> Line 1\n> Line 2\n> Line 3";
        assert_eq!(blockquote_content(input), expected);
    }

    #[test]
    fn test_blockquote_content_with_markdown() {
        let input = "# Header\n## Subheader\n- Item 1\n- Item 2";
        let expected = "> # Header\n> ## Subheader\n> - Item 1\n> - Item 2";
        assert_eq!(blockquote_content(input), expected);
    }

    #[test]
    fn test_blockquote_content_empty() {
        assert_eq!(blockquote_content(""), "> \n");
    }

    #[test]
    fn test_format_stats_all() {
        assert_eq!(format_stats(234, 50, 39, 45), "👍 234  ♻️ 89  💬 45");
    }

    #[test]
    fn test_format_stats_only_likes() {
        assert_eq!(format_stats(12, 0, 0, 0), "👍 12");
    }

    #[test]
    fn test_format_stats_no_reposts() {
        assert_eq!(format_stats(33, 0, 1, 1), "👍 33  ♻️ 1  💬 1");
    }

    #[test]
    fn test_format_stats_all_zero() {
        assert_eq!(format_stats(0, 0, 0, 0), "");
    }

    #[test]
    fn test_format_stats_reshares_combined() {
        assert_eq!(format_stats(10, 5, 3, 2), "👍 10  ♻️ 8  💬 2");
    }

    #[test]
    fn test_format_timestamp_with_millis() {
        assert_eq!(
            format_timestamp("2025-10-31T23:38:49.569Z"),
            "2025-10-31T23:38:49Z"
        );
    }

    #[test]
    fn test_format_timestamp_without_millis() {
        assert_eq!(
            format_timestamp("2024-10-06T10:15:33Z"),
            "2024-10-06T10:15:33Z"
        );
    }

    #[test]
    fn test_format_timestamp_no_z_suffix() {
        assert_eq!(
            format_timestamp("2024-10-06T10:15:33"),
            "2024-10-06T10:15:33Z"
        );
    }

    #[test]
    fn test_extract_rkey() {
        assert_eq!(
            extract_rkey("at://did:plc:abc/app.bsky.feed.post/3m4jnj3efp22t"),
            "3m4jnj3efp22t"
        );
        assert_eq!(extract_rkey("3m4jnj3efp22t"), "3m4jnj3efp22t");
        assert_eq!(extract_rkey(""), "unknown");
    }

    #[test]
    fn test_threading_indicator_root() {
        assert_eq!(
            threading_indicator(0, "", "@alice/3kq8a3f1"),
            "@alice/3kq8a3f1"
        );
    }

    #[test]
    fn test_threading_indicator_depth_1() {
        assert_eq!(
            threading_indicator(1, "@a/…a3f1", "@bob/3kq8b2e4"),
            "└─@a/…a3f1 → @bob/3kq8b2e4"
        );
    }

    #[test]
    fn test_threading_indicator_depth_2() {
        assert_eq!(
            threading_indicator(2, "@b/…b2e4", "@carol/3kq8c3f5"),
            "  └─@b/…b2e4 → @carol/3kq8c3f5"
        );
    }

    #[test]
    fn test_threading_indicator_depth_3() {
        assert_eq!(
            threading_indicator(3, "@c/…c3f5", "@dave/3kq8d4f6"),
            "    └─@c/…c3f5 → @dave/3kq8d4f6"
        );
    }
}
