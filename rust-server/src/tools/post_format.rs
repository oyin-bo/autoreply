//! Post formatting utilities for consistent Markdown output
//!
//! Implements the formatting spec from docs/16-mcp-schemas.md:
//! - Blockquoted user content (prefix with `> `)
//! - Compact post IDs (@handle/rkey or @h/…last4)
//! - Emoji stats (👍 likes  ♻️ reshares  💬 replies)
//! - Threading indicators (└─ with indentation)
//! - ISO timestamps without milliseconds

use std::collections::HashMap;
use crate::bluesky::records::{Facet, FacetFeature};

/// Apply facets to text, converting mentions/links/tags to Markdown format
/// Facets use byte indices, so we need to handle UTF-8 properly
pub fn apply_facets_to_text(text: &str, facets: &[Facet]) -> String {
    if facets.is_empty() {
        return text.to_string();
    }

    // Sort facets by byte_start to process in order
    let mut sorted_facets = facets.to_vec();
    sorted_facets.sort_by_key(|f| f.index.byte_start);

    let mut result = String::new();
    let mut last_byte_idx = 0;

    for facet in &sorted_facets {
        let start_byte = facet.index.byte_start as usize;
        let end_byte = facet.index.byte_end as usize;

        // Add text before this facet
        if last_byte_idx < start_byte {
            result.push_str(&text[last_byte_idx..start_byte]);
        }

        // Get the text covered by this facet
        let facet_text = &text[start_byte..end_byte];

        // Apply the facet formatting based on feature type
        let formatted = format_facet_feature(facet_text, &facet.features);
        result.push_str(&formatted);

        last_byte_idx = end_byte;
    }

    // Add remaining text after last facet
    if last_byte_idx < text.len() {
        result.push_str(&text[last_byte_idx..]);
    }

    result
}

/// Format a facet feature (mention, link, or tag) as Markdown
fn format_facet_feature(text: &str, features: &[FacetFeature]) -> String {
    // Use the first feature if multiple are present
    if let Some(feature) = features.first() {
        match feature {
            FacetFeature::Mention { did: _ } => {
                // The text already contains the @ symbol and handle
                // Extract handle without the @ prefix for the URL
                let handle = text.trim_start_matches('@');
                format!("[{}](https://bsky.app/profile/{})", text, handle)
            }
            FacetFeature::Link { uri } => {
                // Create a markdown link
                format!("[{}]({})", text, uri)
            }
            FacetFeature::Tag { tag } => {
                // Link to hashtag search
                format!("[#{}](https://bsky.app/hashtag/{})", tag, tag)
            }
        }
    } else {
        // No features, return text as-is
        text.to_string()
    }
}

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
        &rkey[rkey.len() - 4..]
    } else {
        rkey
    };
    format!("@{}/…{}", first_letter, last_four)
}

/// Blockquote user content - prefix every line with `> `
/// If facets are provided, applies them first to convert mentions/links/tags to Markdown
pub fn blockquote_content(text: &str) -> String {
    if text.is_empty() {
        return "> \n".to_string();
    }

    text.lines()
        .map(|line| format!("> {}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Blockquote user content with facets applied
/// This is the preferred method when you have facet data available
pub fn blockquote_content_with_facets(text: &str, facets: &[Facet]) -> String {
    let formatted_text = apply_facets_to_text(text, facets);
    blockquote_content(&formatted_text)
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
        format!(
            "{}Z",
            timestamp
                .trim_end_matches('+')
                .split('+')
                .next()
                .unwrap_or(timestamp)
        )
    }
}

/// Extract rkey from at:// URI
/// at://did:plc:abc123/app.bsky.feed.post/3m4jnj3efp22t -> 3m4jnj3efp22t
pub fn extract_rkey(uri: &str) -> &str {
    if uri.is_empty() {
        return "unknown";
    }
    uri.split('/').next_back().unwrap_or("unknown")
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
    use crate::bluesky::records::{Facet, FacetFeature, FacetIndex};

    #[test]
    fn test_apply_facets_mention() {
        let text = "Hello @alice.bsky.social how are you?";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 6,
                byte_end: 24,
            },
            features: vec![FacetFeature::Mention {
                did: "did:plc:abc123".to_string(),
            }],
        }];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "Hello [@alice.bsky.social](https://bsky.app/profile/alice.bsky.social) how are you?"
        );
    }

    #[test]
    fn test_apply_facets_link() {
        let text = "Check out https://example.com for more info";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 10,
                byte_end: 29,
            },
            features: vec![FacetFeature::Link {
                uri: "https://example.com".to_string(),
            }],
        }];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "Check out [https://example.com](https://example.com) for more info"
        );
    }

    #[test]
    fn test_apply_facets_hashtag() {
        let text = "This is #awesome stuff";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 8,
                byte_end: 16,
            },
            features: vec![FacetFeature::Tag {
                tag: "awesome".to_string(),
            }],
        }];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "This is [#awesome](https://bsky.app/hashtag/awesome) stuff"
        );
    }

    #[test]
    fn test_apply_facets_multiple() {
        let text = "Hey @bob check https://test.com and #cool";
        let facets = vec![
            Facet {
                index: FacetIndex {
                    byte_start: 4,
                    byte_end: 8,
                },
                features: vec![FacetFeature::Mention {
                    did: "did:plc:xyz".to_string(),
                }],
            },
            Facet {
                index: FacetIndex {
                    byte_start: 15,
                    byte_end: 31,
                },
                features: vec![FacetFeature::Link {
                    uri: "https://test.com".to_string(),
                }],
            },
            Facet {
                index: FacetIndex {
                    byte_start: 36,
                    byte_end: 41,
                },
                features: vec![FacetFeature::Tag {
                    tag: "cool".to_string(),
                }],
            },
        ];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "Hey [@bob](https://bsky.app/profile/bob) check [https://test.com](https://test.com) and [#cool](https://bsky.app/hashtag/cool)"
        );
    }

    #[test]
    fn test_apply_facets_emoji() {
        // Test with multi-byte UTF-8 characters (emoji)
        // "Hello 👋 @alice"
        // H=0, e=1, l=2, l=3, o=4, space=5, 👋=6-9, space=10, @=11, a=12, l=13, i=14, c=15, e=16
        let text = "Hello 👋 @alice";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 11, // Start of @alice (after "Hello 👋 ")
                byte_end: 17,   // End of @alice
            },
            features: vec![FacetFeature::Mention {
                did: "did:plc:test".to_string(),
            }],
        }];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(result, "Hello 👋 [@alice](https://bsky.app/profile/alice)");
    }

    #[test]
    fn test_apply_facets_empty() {
        let text = "No facets here";
        let facets = vec![];

        let result = apply_facets_to_text(text, &facets);
        assert_eq!(result, "No facets here");
    }

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
        assert_eq!(
            ultra_compact_id("alice.bsky.social", "3kq8a3f1"),
            "@a/…a3f1"
        );
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
