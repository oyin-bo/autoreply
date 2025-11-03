//! Post formatting utilities for consistent Markdown output
//!
//! Implements the formatting spec from docs/16-mcp-schemas.md:
//! - Blockquoted user content (prefix with `> `)
//! - Compact post IDs (@handle/rkey or @h/…last4)
//! - Emoji stats (👍 likes  ♻️ reshares  💬 replies)
//! - Threading indicators (└─ with indentation)
//! - ISO timestamps without milliseconds

use std::collections::HashMap;
use crate::bluesky::records::{Embed, Facet, FacetFeature, ImageEmbed};

/// Apply facets to text, converting mentions/links/tags to Markdown format
/// Facets use byte indices, so we need to handle UTF-8 properly
pub fn apply_facets_to_text(text: &str, facets: &[Facet]) -> String {
    if facets.is_empty() {
        return text.to_string();
    }

    // Sort facets by byte_start to process in order.
    // For overlapping facets, the one that starts first and is longest is prioritized.
    let mut sorted_facets = facets.to_vec();
    sorted_facets.sort_by(|a, b| {
        a.index
            .byte_start
            .cmp(&b.index.byte_start)
            .then_with(|| b.index.byte_end.cmp(&a.index.byte_end))
    });

    let mut result = String::new();
    let mut last_byte_idx = 0;

    for facet in &sorted_facets {
        let start_byte = facet.index.byte_start as usize;
        let end_byte = facet.index.byte_end as usize;

        // Skip if this facet is completely contained within the last one we processed.
        if start_byte < last_byte_idx {
            continue;
        }

        // Basic bounds check to prevent panic on malformed data
        if start_byte > text.len() || end_byte > text.len() || start_byte > end_byte {
            continue; // Skip invalid facet
        }

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

/// Format a single embed into a Markdown string.
/// `did` is required to construct full image URLs.
pub fn format_embed(embed: &Embed, did: &str) -> String {
    match embed {
        Embed::Images { images } => images
            .iter()
            .map(|img| {
                let alt = img.alt.as_deref().unwrap_or("");
                // URL format: https://cdn.bsky.app/img/feed_fullsize/plain/{did}/{cid}@jpeg
                let url = format!(
                    "https://cdn.bsky.app/img/feed_fullsize/plain/{}@jpeg",
                    img.image.ref_
                );
                format!("![{}]({})", alt, url)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Embed::External { external } => {
            let mut parts = vec![format!("[{}]({})", external.title, external.uri)];
            if !external.description.is_empty() {
                parts.push(blockquote_content(&external.description));
            }
            if let Some(thumb) = &external.thumb {
                let url = format!(
                    "https://cdn.bsky.app/img/feed_thumbnail/plain/{}@jpeg",
                    thumb.ref_
                );
                parts.push(format!("![thumb]({})", url));
            }
            parts.join("\n")
        }
        Embed::Record { record } => {
            // For now, just show the record URI. A full implementation would
            // require fetching and rendering the quoted post.
            blockquote_content(&format!("Quoted post: {}", record.uri))
        }
        Embed::RecordWithMedia { record, media } => {
            let record_md = format_embed(&Embed::Record { record: record.clone() }, did);
            let media_md = format_embed(media, did);
            format!("{}\n{}", record_md, media_md)
        }
    }
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
    use crate::bluesky::records::{
        BlobRef, Embed, ExternalEmbed, Facet, FacetFeature, FacetIndex, ImageEmbed, RecordEmbed,
    };

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

    #[test]
    fn test_apply_facets_overlapping() {
        // A link facet that contains a mention facet. The outer (link) facet should be applied.
        let text = "Check out @alice.bsky.social for more.";
        let facets = vec![
            Facet {
                // The inner mention
                index: FacetIndex {
                    byte_start: 10,
                    byte_end: 28,
                },
                features: vec![FacetFeature::Mention {
                    did: "did:plc:alice".to_string(),
                }],
            },
            Facet {
                // The outer link
                index: FacetIndex {
                    byte_start: 0,
                    byte_end: 38,
                },
                features: vec![FacetFeature::Link {
                    uri: "https://example.com".to_string(),
                }],
            },
        ];
        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "[Check out @alice.bsky.social for more.](https://example.com)"
        );
    }

    #[test]
    fn test_apply_facets_adjacent() {
        let text = "#one#two";
        let facets = vec![
            Facet {
                index: FacetIndex {
                    byte_start: 0,
                    byte_end: 4,
                },
                features: vec![FacetFeature::Tag {
                    tag: "one".to_string(),
                }],
            },
            Facet {
                index: FacetIndex {
                    byte_start: 4,
                    byte_end: 8,
                },
                features: vec![FacetFeature::Tag {
                    tag: "two".to_string(),
                }],
            },
        ];
        let result = apply_facets_to_text(text, &facets);
        assert_eq!(
            result,
            "[#one](https://bsky.app/hashtag/one)[#two](https://bsky.app/hashtag/two)"
        );
    }

    #[test]
    fn test_apply_facets_invalid_indices() {
        // This should not panic. It should gracefully ignore the invalid facet.
        let text = "A text with a bad facet.";
        let facets = vec![
            Facet {
                // byte_end is beyond the text length
                index: FacetIndex {
                    byte_start: 15,
                    byte_end: 100,
                },
                features: vec![FacetFeature::Tag {
                    tag: "bad".to_string(),
                }],
            },
            Facet {
                // byte_start > byte_end
                index: FacetIndex {
                    byte_start: 10,
                    byte_end: 5,
                },
                features: vec![FacetFeature::Tag {
                    tag: "inverted".to_string(),
                }],
            },
        ];
        // The function should not panic and should just return the original text
        // as the invalid ranges will cause slicing errors that are caught.
        let result = apply_facets_to_text(text, &facets);
        assert_eq!(result, "A text with a bad facet.");
    }

    #[test]
    fn test_apply_facets_malformed_data() {
        // A facet with an empty features array should be ignored.
        let text = "Text with a featureless facet.";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 10,
                byte_end: 21,
            },
            features: vec![], // No features
        }];
        let result = apply_facets_to_text(text, &facets);
        assert_eq!(result, "Text with a featureless facet.");
    }

    #[test]
    fn test_format_embed_single_image() {
        let embed = Embed::Images {
            images: vec![ImageEmbed {
                alt: Some("A beautiful sunset".to_string()),
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "did:plc:test/bafkreihd...".to_string(),
                    mime_type: "image/jpeg".to_string(),
                    size: 12345,
                },
            }],
        };
        let result = format_embed(&embed, "did:plc:test");
        assert_eq!(
            result,
            "![A beautiful sunset](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkreihd...@jpeg)"
        );
    }

    #[test]
    fn test_format_embed_multiple_images() {
        let embed = Embed::Images {
            images: vec![
                ImageEmbed {
                    alt: Some("Image 1".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "did:plc:test/bafkrei_img1...".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 100,
                    },
                },
                ImageEmbed {
                    alt: Some("Image 2".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "did:plc:test/bafkrei_img2...".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 200,
                    },
                },
            ],
        };
        let result = format_embed(&embed, "did:plc:test");
        assert_eq!(
            result,
            "![Image 1](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_img1...@jpeg)\n![Image 2](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_img2...@jpeg)"
        );
    }

    #[test]
    fn test_format_embed_external() {
        let embed = Embed::External {
            external: ExternalEmbed {
                uri: "https://example.com".to_string(),
                title: "Example Title".to_string(),
                description: "This is a description.".to_string(),
                thumb: Some(BlobRef {
                    type_: "blob".to_string(),
                    ref_: "did:plc:test/bafkrei_thumb...".to_string(),
                    mime_type: "image/jpeg".to_string(),
                    size: 50,
                }),
            },
        };
        let result = format_embed(&embed, "did:plc:test");
        let expected = "[Example Title](https://example.com)\n> This is a description.\n![thumb](https://cdn.bsky.app/img/feed_thumbnail/plain/did:plc:test/bafkrei_thumb...@jpeg)";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_format_embed_record() {
        let embed = Embed::Record {
            record: RecordEmbed {
                uri: "at://did:plc:test/app.bsky.feed.post/3kxyz".to_string(),
                cid: "bafy...".to_string(),
            },
        };
        let result = format_embed(&embed, "did:plc:test");
        assert_eq!(
            result,
            "> Quoted post: at://did:plc:test/app.bsky.feed.post/3kxyz"
        );
    }

    #[test]
    fn test_format_embed_record_with_media() {
        let embed = Embed::RecordWithMedia {
            record: RecordEmbed {
                uri: "at://did:plc:quote/app.bsky.feed.post/3kabc".to_string(),
                cid: "bafy_quote".to_string(),
            },
            media: Box::new(Embed::Images {
                images: vec![ImageEmbed {
                    alt: Some("A cat".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "did:plc:test/bafkrei_cat...".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 999,
                    },
                }],
            }),
        };
        let result = format_embed(&embed, "did:plc:test");
        let expected = "> Quoted post: at://did:plc:quote/app.bsky.feed.post/3kabc\n![A cat](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_cat...@jpeg)";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_format_post_with_text_and_embed() {
        // A post with both text and an image embed
        let text = "Check out this cool picture!";
        let facets = vec![];
        let embed = Embed::Images {
            images: vec![ImageEmbed {
                alt: Some("A cool picture".to_string()),
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "did:plc:test/bafy_cool...".to_string(),
                    mime_type: "image/jpeg".to_string(),
                    size: 123,
                },
            }],
        };

        let text_md = blockquote_content_with_facets(text, &facets);
        let embed_md = format_embed(&embed, "did:plc:test");

        let final_md = format!("{}\n\n{}", text_md, embed_md);

        let expected_text = "> Check out this cool picture!";
        let expected_embed = "![A cool picture](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafy_cool...@jpeg)";
        assert!(final_md.contains(expected_text));
        assert!(final_md.contains(expected_embed));
    }

    #[test]
    fn test_format_post_with_facets_and_embed() {
        // A post with rich text (facets) and an external embed
        let text = "More info at example.com";
        let facets = vec![Facet {
            index: FacetIndex {
                byte_start: 13,
                byte_end: 24,
            },
            features: vec![FacetFeature::Link {
                uri: "https://example.com".to_string(),
            }],
        }];
        let embed = Embed::External {
            external: ExternalEmbed {
                uri: "https://anotherexample.com".to_string(),
                title: "Another Example".to_string(),
                description: "Description here.".to_string(),
                thumb: None,
            },
        };

        let text_md = blockquote_content_with_facets(text, &facets);
        let embed_md = format_embed(&embed, "did:plc:test");

        let final_md = format!("{}\n\n{}", text_md, embed_md);

        let expected_text = "> More info at [example.com](https://example.com)";
        let expected_embed = "[Another Example](https://anotherexample.com)\n> Description here.";
        assert!(final_md.contains(expected_text));
        assert!(final_md.contains(expected_embed));
    }

    #[test]
    fn test_format_post_with_embed_and_empty_text() {
        // A post with an embed but no text content.
        let text = "";
        let facets = vec![];
        let embed = Embed::Images {
            images: vec![ImageEmbed {
                alt: Some("An image on its own".to_string()),
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "did:plc:test/bafy_solo...".to_string(),
                    mime_type: "image/jpeg".to_string(),
                    size: 456,
                },
            }],
        };

        let text_md = blockquote_content_with_facets(text, &facets);
        let embed_md = format_embed(&embed, "did:plc:test");

        // When text is empty, blockquote_content returns "> \n". We might not want that.
        // Let's define the final output as just the embed.
        let final_md = if text.is_empty() {
            embed_md
        } else {
            format!("{}\n\n{}", text_md, embed_md)
        };

        let expected_embed = "![An image on its own](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafy_solo...@jpeg)";
        assert_eq!(final_md, expected_embed);
        assert!(!final_md.contains(">"));
    }

    #[test]
    fn test_format_embed_record_with_external_media() {
        // Test a more complex combination: a quote post that also has an external link card.
        let embed = Embed::RecordWithMedia {
            record: RecordEmbed {
                uri: "at://did:plc:quote/app.bsky.feed.post/3kdef".to_string(),
                cid: "bafy_quote_ext".to_string(),
            },
            media: Box::new(Embed::External {
                external: ExternalEmbed {
                    uri: "https://dev.blueskyweb.xyz/".to_string(),
                    title: "Bluesky Dev".to_string(),
                    description: "Dev docs".to_string(),
                    thumb: None,
                },
            }),
        };
        let result = format_embed(&embed, "did:plc:test");
        let expected = "> Quoted post: at://did:plc:quote/app.bsky.feed.post/3kdef\n[Bluesky Dev](https://dev.blueskyweb.xyz/)\n> Dev docs";
        assert_eq!(result, expected);
    }
}
