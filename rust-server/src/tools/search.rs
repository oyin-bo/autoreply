//! Search tool implementation
//!
//! Implements the `search(from, query)` MCP tool

use crate::bluesky::did::DidResolver;
use crate::bluesky::provider::RepositoryProvider;
use crate::bluesky::records::{Facet, FacetFeature, FacetIndex, PostRecord};
use crate::car::cbor::{decode_cbor, get_array_field, get_int_field, get_map_field, get_text_field, CborValue};
use crate::cli::SearchArgs;
use crate::error::{normalize_text, validate_account, validate_query, AppError};
use crate::mcp::{McpResponse, ToolResult};
use crate::search::SearchEngine;
use anyhow::Result;

use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::debug;

/// Extract facets from CBOR map (Vec of tuples)
fn extract_facets(post_map: &[(CborValue, CborValue)]) -> Vec<Facet> {
    let facets_array = match get_array_field(post_map, "facets") {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    facets_array
        .iter()
        .filter_map(|facet_value| {
            if let CborValue::Map(facet_map) = facet_value {
                // Extract index
                let index_map = get_map_field(facet_map, "index")?;
                let byte_start = get_int_field(index_map, "byteStart")? as u32;
                let byte_end = get_int_field(index_map, "byteEnd")? as u32;

                // Extract features
                let features_array = get_array_field(facet_map, "features")?;
                let features: Vec<FacetFeature> = features_array
                    .iter()
                    .filter_map(|feature_value| {
                        if let CborValue::Map(feature_map) = feature_value {
                            let type_str = get_text_field(feature_map, "$type")?;

                            match type_str {
                                "app.bsky.richtext.facet#mention" => {
                                    let did = get_text_field(feature_map, "did")?.to_string();
                                    Some(FacetFeature::Mention { did })
                                }
                                "app.bsky.richtext.facet#link" => {
                                    let uri = get_text_field(feature_map, "uri")?.to_string();
                                    Some(FacetFeature::Link { uri })
                                }
                                "app.bsky.richtext.facet#tag" => {
                                    let tag = get_text_field(feature_map, "tag")?.to_string();
                                    Some(FacetFeature::Tag { tag })
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if features.is_empty() {
                    return None;
                }

                Some(Facet {
                    index: FacetIndex {
                        byte_start,
                        byte_end,
                    },
                    features,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Extract embeds from CBOR map
fn extract_embeds(post_map: &[(CborValue, CborValue)]) -> Option<Vec<Embed>> {
    let embed_value = get_map_field(post_map, "embed")?;
    parse_embed(embed_value)
}

/// Recursively parse an embed CBOR value
fn parse_embed(embed_value: &CborValue) -> Option<Vec<Embed>> {
    if let CborValue::Map(embed_map) = embed_value {
        let type_str = get_text_field(embed_map, "$type")?;
        match type_str {
            "app.bsky.embed.images" => {
                let images_array = get_array_field(embed_map, "images")?;
                let images = images_array.iter().filter_map(parse_image_embed).collect();
                Some(vec![Embed::Images { images }])
            }
            "app.bsky.embed.external" => {
                let external_map = get_map_field(embed_map, "external")?;
                let external = parse_external_embed(external_map)?;
                Some(vec![Embed::External { external }])
            }
            "app.bsky.embed.record" => {
                let record_map = get_map_field(embed_map, "record")?;
                let record = parse_record_embed(record_map)?;
                Some(vec![Embed::Record { record }])
            }
            "app.bsky.embed.recordWithMedia" => {
                let record_map = get_map_field(embed_map, "record")?;
                let record = parse_record_embed(record_map)?;
                let media_value = get_map_field(embed_map, "media")?;
                // The `media` field contains another embed, so we recurse.
                // It should resolve to a single-element Vec, so we take the first.
                let media_embed = parse_embed(media_value)?.into_iter().next()?;
                Some(vec![Embed::RecordWithMedia {
                    record,
                    media: Box::new(media_embed),
                }])
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Parse an ImageEmbed from a CBOR map
fn parse_image_embed(image_value: &CborValue) -> Option<ImageEmbed> {
    if let CborValue::Map(image_map) = image_value {
        let alt = get_text_field(image_map, "alt").map(|s| s.to_string());
        let image_blob_map = get_map_field(image_map, "image")?;
        let image = parse_blob_ref(image_blob_map)?;
        Some(ImageEmbed { alt, image })
    } else {
        None
    }
}

/// Parse an ExternalEmbed from a CBOR map
fn parse_external_embed(external_map: &[(CborValue, CborValue)]) -> Option<ExternalEmbed> {
    let uri = get_text_field(external_map, "uri")?.to_string();
    let title = get_text_field(external_map, "title")?.to_string();
    let description = get_text_field(external_map, "description")?.to_string();
    let thumb = get_map_field(external_map, "thumb").and_then(parse_blob_ref);
    Some(ExternalEmbed {
        uri,
        title,
        description,
        thumb,
    })
}

/// Parse a RecordEmbed from a CBOR map
fn parse_record_embed(record_map: &[(CborValue, CborValue)]) -> Option<RecordEmbed> {
    let uri = get_text_field(record_map, "uri")?.to_string();
    let cid = get_text_field(record_map, "cid")?.to_string();
    Some(RecordEmbed { uri, cid })
}

/// Parse a BlobRef from a CBOR map
fn parse_blob_ref(blob_map: &[(CborValue, CborValue)]) -> Option<BlobRef> {
    let type_ = get_text_field(blob_map, "$type")?.to_string();
    let mime_type = get_text_field(blob_map, "mimeType")?.to_string();
    let size = get_int_field(blob_map, "size")? as u64;
    // The 'ref' can be a map with a '$link' key
    let ref_val = blob_map
        .iter()
        .find(|(k, _)| k == &CborValue::Text("ref".to_string()))
        .map(|(_, v)| v);

    let ref_ = match ref_val {
        Some(CborValue::Map(ref_map)) => get_text_field(ref_map, "$link").map(|s| s.to_string()),
        Some(CborValue::Text(s)) => Some(s.clone()),
        _ => None,
    }?;

    Some(BlobRef {
        type_,
        ref_,
        mime_type,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bluesky::records::PostRecord;
    use serde_json::json;

    #[tokio::test]
    async fn test_search_args_parsing() {
        let args = json!({
            "from": "test.bsky.social",
            "query": "hello world"
        });

        let parsed: SearchArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.from, "test.bsky.social");
        assert_eq!(parsed.query, "hello world");
    }

    #[test]
    fn test_fuzzy_search_integration() {
        use crate::search::SearchEngine;

        let posts = vec![
            PostRecord {
                uri: "at://test/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Hello world, this is a test post".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
            PostRecord {
                uri: "at://test/app.bsky.feed.post/2".to_string(),
                cid: "cid2".to_string(),
                text: "This is another post about programming".to_string(),
                created_at: "2024-01-02T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
            PostRecord {
                uri: "at://test/app.bsky.feed.post/3".to_string(),
                cid: "cid3".to_string(),
                text: "Hello everyone, how are you doing?".to_string(),
                created_at: "2024-01-03T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
        ];

        let mut engine = SearchEngine::new();
        let results = engine.search("hello", &posts, |p| p.get_searchable_text());

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.item.text.contains("Hello world")));
        assert!(results
            .iter()
            .any(|r| r.item.text.contains("Hello everyone")));

        let results = engine.search("programming", &posts, |p| p.get_searchable_text());
        assert_eq!(results.len(), 1);
        assert!(results[0].item.text.contains("programming"));

        let results = engine.search("nonexistent", &posts, |p| p.get_searchable_text());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_format_search_results() {
        let post = PostRecord {
            uri: "at://test/app.bsky.feed.post/1".to_string(),
            cid: "cid1".to_string(),
            text: "Hello world, this is a test".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: vec![],
            facets: vec![],
        };

        let posts = vec![&post];
        let markdown = format_search_results(&posts, "test.bsky.social", "hello");

        assert!(markdown.contains("# Search Results · 1 posts"));
        assert!(markdown.contains("@test.bsky.social/1"));
        assert!(markdown.contains("> **Hello** world, this is a test"));
        assert!(markdown.contains("2024-01-01T00:00:00Z"));
    }

    #[test]
    fn test_extract_facets() {
        use crate::car::cbor::CborValue;

        // Build CBOR structure for a post with facets
        let facets_cbor = vec![
            CborValue::Map(vec![
                (
                    CborValue::Text("index"),
                    CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(0)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(10)),
                    ]),
                ),
                (
                    CborValue::Text("features"),
                    CborValue::Array(vec![CborValue::Map(vec![
                        (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#mention")),
                        (CborValue::Text("did"), CborValue::Text("did:plc:test123")),
                    ])]),
                ),
            ]),
            CborValue::Map(vec![
                (
                    CborValue::Text("index"),
                    CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(15)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(30)),
                    ]),
                ),
                (
                    CborValue::Text("features"),
                    CborValue::Array(vec![CborValue::Map(vec![
                        (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#link")),
                        (CborValue::Text("uri"), CborValue::Text("https://example.com")),
                    ])]),
                ),
            ]),
            CborValue::Map(vec![
                (
                    CborValue::Text("index"),
                    CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(35)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(45)),
                    ]),
                ),
                (
                    CborValue::Text("features"),
                    CborValue::Array(vec![CborValue::Map(vec![
                        (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#tag")),
                        (CborValue::Text("tag"), CborValue::Text("rust")),
                    ])]),
                ),
            ]),
        ];

        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Test post")),
            (CborValue::Text("facets"), CborValue::Array(facets_cbor)),
        ];

        let facets = extract_facets(&post_map);

        assert_eq!(facets.len(), 3);

        // Test mention facet
        assert_eq!(facets[0].index.byte_start, 0);
        assert_eq!(facets[0].index.byte_end, 10);
        match &facets[0].features[0] {
            FacetFeature::Mention { did } => assert_eq!(did, "did:plc:test123"),
            _ => panic!("Expected mention facet"),
        }

        // Test link facet
        assert_eq!(facets[1].index.byte_start, 15);
        assert_eq!(facets[1].index.byte_end, 30);
        match &facets[1].features[0] {
            FacetFeature::Link { uri } => assert_eq!(uri, "https://example.com"),
            _ => panic!("Expected link facet"),
        }

        // Test tag facet
        assert_eq!(facets[2].index.byte_start, 35);
        assert_eq!(facets[2].index.byte_end, 45);
        match &facets[2].features[0] {
            FacetFeature::Tag { tag } => assert_eq!(tag, "rust"),
            _ => panic!("Expected tag facet"),
        }
    }

    #[test]
    fn test_extract_facets_empty() {
        use crate::car::cbor::CborValue;

        // Post with no facets field
        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Test post")),
        ];

        let facets = extract_facets(&post_map);
        assert_eq!(facets.len(), 0);

        // Post with empty facets array
        let post_map_empty = vec![
            (CborValue::Text("text"), CborValue::Text("Test post")),
            (CborValue::Text("facets"), CborValue::Array(vec![])),
        ];

        let facets = extract_facets(&post_map_empty);
        assert_eq!(facets.len(), 0);
    }

    #[test]
    fn test_search_and_highlight_in_embed_alt_text() {
        use crate::bluesky::records::{BlobRef, Embed, ImageEmbed};

        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/embed_search".to_string(),
            cid: "cid_embed_search".to_string(),
            text: "This post has an image.".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: Some(vec![Embed::Images {
                images: vec![ImageEmbed {
                    alt: Some("A detailed photo of a fuzzy brown cat".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "did:plc:test/bafkrei_cat_fuzzy...".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 12345,
                    },
                }],
            }]),
            facets: vec![],
        };

        // 1. Test that search finds the post based on embed content
        let mut engine = SearchEngine::new();
        let search_results = engine.search("fuzzy cat", &vec![post.clone()], |p| {
            p.get_searchable_text()
        });

        assert_eq!(
            search_results.len(),
            1,
            "Should find post by searching embed alt text"
        );
        assert_eq!(search_results[0].item.uri, post.uri);

        // 2. Test that the formatted result highlights the match inside the embed markdown
        let matching_posts = vec![&post];
        let markdown =
            format_search_results(&matching_posts, "test.bsky.social", "fuzzy cat");

        // The expected highlighted alt text part of the image markdown
        let expected_highlight = "![A detailed photo of a **fuzzy** brown **cat**](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_cat_fuzzy...@jpeg)";
        
        assert!(
            markdown.contains(expected_highlight),
            "Markdown should contain the highlighted alt text.\nMarkdown was:\n---\n{}\n---\nExpected to find:\n---\n{}\n---",
            markdown,
            expected_highlight
        );
    }
}
