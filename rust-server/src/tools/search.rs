//! Search tool implementation
//!
//! Implements the `search(from, query)` MCP tool

use crate::bluesky::did::DidResolver;
use crate::bluesky::provider::RepositoryProvider;
use crate::bluesky::records::{Facet, FacetFeature, FacetIndex, PostRecord};
use crate::bluesky::records::{Embed, ImageEmbed, ExternalEmbed, RecordEmbed, BlobRef};
use crate::car::cbor::{decode_cbor, get_array_field, get_int_field, get_map_field, get_text_field, CborValue};
use crate::cli::SearchArgs;
use crate::error::{normalize_text, validate_account, validate_query, AppError};
use crate::mcp::{McpResponse, ToolResult};
use crate::search::SearchEngine;
use anyhow::Result;
use std::collections::HashMap;

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
    let embed_map = get_map_field(post_map, "embed")?;
    parse_embed_map(embed_map)
}

/// Recursively parse an embed CBOR map (map is represented as slice of pairs)
fn parse_embed_map(embed_map: &[(CborValue, CborValue)]) -> Option<Vec<Embed>> {
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
            let media_embed = parse_embed_map(media_value)?.into_iter().next()?;
            Some(vec![Embed::RecordWithMedia {
                record,
                media: Box::new(media_embed),
            }])
        }
        _ => None,
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
        .find(|(k, _)| k == &CborValue::Text("ref"))
        .map(|(_, v)| v);

    let ref_ = match ref_val {
        Some(CborValue::Map(ref_map)) => get_text_field(ref_map, "$link").map(|s| s.to_string()),
        Some(CborValue::Text(s)) => Some(s.to_string()),
        _ => None,
    }?;

    Some(BlobRef {
        type_,
        ref_,
        mime_type,
        size,
    })
}

/// Format search results into markdown for display (used by tests and CLI)
pub fn format_search_results(posts: &[&PostRecord], handle: &str, query: &str) -> String {
    // Highlighter that splits query into words, finds all matches (case-insensitive),
    // merges adjacent/overlapping match ranges and wraps each merged range in **bold**.
    fn highlight(text: &str, query: &str) -> String {
        if query.is_empty() {
            return text.to_string();
        }

        let lower = text.to_lowercase();
        let mut ranges: Vec<(usize, usize)> = Vec::new();

        for term in query.split_whitespace() {
            if term.is_empty() {
                continue;
            }
            let term_l = term.to_lowercase();
            let mut idx = 0usize;
            while let Some(pos) = lower[idx..].find(&term_l) {
                let abs = idx + pos;
                // term.len() is byte length which aligns with positions returned by find on the
                // lowercased string (both are byte indices), so this is safe for UTF-8 slices
                // as long as we use the same string for slicing the original `text`.
                ranges.push((abs, abs + term.len()));
                idx = abs + term.len();
            }
        }

        if ranges.is_empty() {
            return text.to_string();
        }

        // Sort and merge ranges (merge adjacent and overlapping ranges)
        ranges.sort_by_key(|r| r.0);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in ranges {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    // Overlap or adjacent: extend the last range
                    if e > last.1 {
                        last.1 = e;
                    }
                } else {
                    // Allow merging across a single newline (soft line break) so that
                    // emphasis can span across lines inside a paragraph. Do NOT merge
                    // across spaces or multiple newlines (paragraph breaks).
                    if s == last.1 + 1 && text.as_bytes()[last.1] == b'\n' {
                        // extend across the single newline
                        if e > last.1 {
                            last.1 = e;
                        }
                    } else {
                        merged.push((s, e));
                    }
                }
            } else {
                merged.push((s, e));
            }
        }

        // Build highlighted string from merged ranges
        let mut res = String::new();
        let mut last_idx = 0usize;
        for (s, e) in merged {
            if last_idx < s {
                res.push_str(&text[last_idx..s]);
            }
            res.push_str("**");
            res.push_str(&text[s..e]);
            res.push_str("**");
            last_idx = e;
        }
        if last_idx < text.len() {
            res.push_str(&text[last_idx..]);
        }

        res
    }

    let mut md = String::new();
    md.push_str(&format!("# Search Results · {} posts\n\n", posts.len()));

    for post in posts {
        // Extract post id
        let post_id = post.uri.split('/').last().unwrap_or(&post.uri);
        md.push_str(&format!("@{}/{}\n\n", handle, post_id));

        // Quote highlighted text
        md.push_str(&format!("> {}\n\n", highlight(&post.text, query)));

        md.push_str(&format!("{}\n\n", post.created_at));

        // Links from external embeds and facets
        if let Some(embeds) = &post.embeds {
            for embed in embeds {
                match embed {
                    Embed::External { external } => {
                        md.push_str(&format!("- [{}]({})\n", external.title, external.uri));
                    }
                    Embed::Images { images } => {
                        for img in images {
                            let alt = img.alt.as_deref().unwrap_or("Image");
                            let alt_h = highlight(alt, query);
                            // Build CDN URL from BlobRef. Use mime subtype as extension
                            let ext = img.image.mime_type.split('/').nth(1).unwrap_or("jpeg");
                            let url = format!(
                                "https://cdn.bsky.app/img/feed_fullsize/plain/{}@{}",
                                img.image.ref_, ext
                            );
                            md.push_str(&format!("![{}]({})\n", alt_h, url));
                        }
                    }
                    _ => {}
                }
            }
        }

        md.push_str("\n---\n\n");
    }

    md
}

/// Handle search tool call (MCP)
pub async fn handle_search(id: Option<Value>, args: Value) -> McpResponse {
    match timeout(Duration::from_secs(120), handle_search_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Search request exceeded 120 second timeout"),
    }
}

async fn handle_search_impl(args: Value) -> Result<ToolResult, AppError> {
    let search_args: SearchArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    execute_search(search_args).await
}

/// Shared implementation for search (used by MCP and CLI)
pub async fn execute_search(search_args: SearchArgs) -> Result<ToolResult, AppError> {
    // Validate inputs
    validate_account(&search_args.from)?;
    validate_query(&search_args.query)?;

    debug!("Search request for account: {}, query: '{}'", search_args.from, search_args.query);

    // Normalize query as specified
    let normalized_query = normalize_text(&search_args.query);
    if normalized_query.is_empty() {
        return Err(AppError::InvalidInput(
            "Query is empty after normalization".to_string(),
        ));
    }

    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(&search_args.from).await?;

    // Determine display handle for markdown
    let display_handle = if search_args.from.starts_with("did:plc:") {
        // If input was a DID, use it verbatim
        search_args.from.clone()
    } else {
        search_args
            .from
            .strip_prefix('@')
            .unwrap_or(&search_args.from)
            .to_string()
    };

    let did_str = did
        .as_ref()
        .ok_or_else(|| AppError::DidResolveFailed("DID resolution failed".to_string()))?;

    // Fetch CAR and extract CID->rkey mapping to reconstruct post rkeys
    let provider = RepositoryProvider::new()?;
    let car_path = provider.fetch_repo_car(did_str).await?;
    let car_bytes = tokio::fs::read(&car_path)
        .await
        .map_err(|e| AppError::CacheError(format!("Failed to read CAR file: {}", e)))?;

    debug!("Extracting CID->rkey mappings from MST for collection app.bsky.feed.post");
    let cid_to_rkey = crate::bluesky::mst::extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post")
        .map_err(|e| {
            AppError::RepoParseFailed(format!("Failed to extract MST mappings: {:?}", e))
        })?;

    debug!("Extracted {} CID->rkey mappings", cid_to_rkey.len());

    // Stream records and collect posts with rkeys
    let records = provider.records(did_str).await?;

    // Iterate records, decode each CBOR entry and build PostRecord directly
    let mut posts: Vec<PostRecord> = Vec::new();
    for record_result in records {
        let (record_type, cbor_data, cid_str) = match record_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if record_type != "app.bsky.feed.post" {
            continue;
        }

        if let Ok(CborValue::Map(post_map)) = decode_cbor(&cbor_data) {
            if let Some(post) = collect_post_from_map(did_str, post_map.as_slice(), &cid_str, &cid_to_rkey) {
                posts.push(post);
            }
        }
    }

    debug!("Extracted {} post records with rkeys", posts.len());

    // Use fuzzy search engine
    run_search_on_posts(&posts, &display_handle, &search_args.query, search_args.limit).await
}

/// Construct PostRecord vector from decoded CBOR maps
///
/// `decoded` is a vec of tuples: (record_type, post_map, cid_str)
/// Build a single PostRecord from a decoded CBOR post map. Returns None if required fields missing
pub(crate) fn collect_post_from_map(
    did_str: &str,
    post_map: &[(CborValue, CborValue)],
    cid_str: &str,
    cid_to_rkey: &HashMap<String, String>,
) -> Option<PostRecord> {
    let text = get_text_field(post_map, "text")?.to_string();
    let created_at = get_text_field(post_map, "createdAt")?.to_string();

    let facets = extract_facets(post_map);
    let embeds = extract_embeds(post_map);

    let collection_rkey = cid_to_rkey.get(cid_str)?.clone();

    Some(PostRecord {
        uri: format!("at://{}/app.bsky.feed.post/{}", did_str, collection_rkey),
        cid: cid_str.to_string(),
        text,
        created_at,
        embeds,
        facets,
    })
}

// Compatibility wrapper used by some tests: build posts from a decoded-records shape
pub(crate) fn collect_posts_from_maps(
    did_str: &str,
    decoded: &[(String, Vec<(CborValue, CborValue)>, String)],
    cid_to_rkey: &HashMap<String, String>,
) -> Vec<PostRecord> {
    let mut posts: Vec<PostRecord> = Vec::new();
    for (_record_type, post_map, cid_str) in decoded.iter() {
        if let Some(post) = collect_post_from_map(did_str, post_map.as_slice(), cid_str, cid_to_rkey) {
            posts.push(post);
        }
    }
    posts
}

/// Run search + formatting on an existing set of posts.
/// Extracted into a helper to allow tests to call the search/format pipeline directly.
pub(crate) async fn run_search_on_posts(
    posts: &[PostRecord],
    display_handle: &str,
    query: &str,
    limit_opt: Option<usize>,
) -> Result<ToolResult, AppError> {
    let mut search_engine = SearchEngine::new();
    let search_results = search_engine.search(query, posts, |post| post.get_searchable_text());

    let limit = limit_opt.unwrap_or(50usize);
    let matching_posts: Vec<&PostRecord> = search_results.iter().take(limit).map(|r| &r.item).collect();

    if matching_posts.is_empty() {
        return Err(AppError::NotFound(format!(
            "No posts found matching query '{}' for results",
            query
        )));
    }

    let markdown = format_search_results(&matching_posts, display_handle, query);
    Ok(ToolResult::text(markdown))
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
                embeds: Some(vec![]),
                facets: vec![],
            },
            PostRecord {
                uri: "at://test/app.bsky.feed.post/2".to_string(),
                cid: "cid2".to_string(),
                text: "This is another post about programming".to_string(),
                created_at: "2024-01-02T00:00:00Z".to_string(),
                embeds: Some(vec![]),
                facets: vec![],
            },
            PostRecord {
                uri: "at://test/app.bsky.feed.post/3".to_string(),
                cid: "cid3".to_string(),
                text: "Hello everyone, how are you doing?".to_string(),
                created_at: "2024-01-03T00:00:00Z".to_string(),
                embeds: Some(vec![]),
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
            embeds: Some(vec![]),
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

    #[test]
    fn test_format_search_results_with_query_highlighting() {
        // Test that search results properly highlight query terms
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/123".to_string(),
            cid: "cid_123".to_string(),
            text: "I love programming in Rust".to_string(),
            created_at: "2024-01-15T10:30:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let posts = vec![&post];
        let markdown = format_search_results(&posts, "test.bsky.social", "Rust");

        // Query should be highlighted
        assert!(
            markdown.contains("**Rust**"),
            "Should highlight the query term 'Rust'"
        );
        assert!(markdown.contains("programming"), "Should contain original text");
    }

    #[test]
    fn test_format_search_results_empty_results() {
        // Test formatting with empty results
        let posts: Vec<&PostRecord> = vec![];
        let markdown = format_search_results(&posts, "test.bsky.social", "nonexistent");

        // Should produce valid markdown even with empty results
        assert!(!markdown.is_empty(), "Should produce output even for empty results");
    }

    #[test]
    fn test_format_search_results_multiple_posts() {
        // Test formatting with multiple search results
        let posts = vec![
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "First post about Rust".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/2".to_string(),
                cid: "cid2".to_string(),
                text: "Second post about Rust performance".to_string(),
                created_at: "2024-01-15T11:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
        ];

        let post_refs: Vec<&PostRecord> = posts.iter().collect();
        let markdown = format_search_results(&post_refs, "test.bsky.social", "Rust");

        // Should contain both posts
        assert!(
            markdown.contains("First post"),
            "Should contain first post in results"
        );
        assert!(
            markdown.contains("Second post"),
            "Should contain second post in results"
        );
        assert!(markdown.matches("**Rust**").count() >= 2, "Should highlight Rust in both posts");
    }

    #[test]
    fn test_highlight_merge_adjacent_letters() {
        // Two single-letter matches adjacent in text should produce one merged bold span
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/adj1".to_string(),
            cid: "cid_adj1".to_string(),
            text: "ab".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let markdown = format_search_results(&vec![&post], "host", "a b");
        assert!(markdown.contains("**ab**"), "Adjacent single-letter matches should merge into **ab**; got:\n{}", markdown);
    }

    #[test]
    fn test_highlight_merge_within_word() {
        // Two matches that are adjacent within a single word should merge
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/adj2".to_string(),
            cid: "cid_adj2".to_string(),
            text: "programming".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        // terms "gram" and "ming" abut in the word
        let markdown = format_search_results(&vec![&post], "host", "gram ming");
        // expect the merged bold span covering the joined substring
        assert!(markdown.contains("pro**gramming**"), "Within-word adjacent matches should merge; got:\n{}", markdown);
    }

    #[test]
    fn test_highlight_not_merge_across_space() {
        // Matches separated by a space should not be merged (space remains between bold spans)
        // Use a minimal example where the two matches are separated by exactly one space
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/edge1".to_string(),
            cid: "cid_edge1".to_string(),
            text: "a b".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let markdown = format_search_results(&vec![&post], "host", "a b");
        // Should contain two separate bold spans with a space between them
        assert!(markdown.contains("**a** **b**"), "Matches across a space should not be merged; got:\n{}", markdown);
    }

    #[test]
    fn test_highlight_not_merge_across_newline() {
        // Matches separated by a single newline (soft break) should be merged into one
        // emphasis span so that emphasis can span across lines inside a paragraph.
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/edge2".to_string(),
            cid: "cid_edge2".to_string(),
            text: "abc\ndef".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let markdown = format_search_results(&vec![&post], "host", "c d");
        // Expect a single bold span containing the newline between the matched characters
        assert!(markdown.contains("**c\nd**"), "Matches across a single newline should merge into one bold span; got:\n{}", markdown);
    }

    #[test]
    fn test_highlight_not_merge_across_paragraph() {
        // Matches separated by a blank line (paragraph break) must NOT be merged
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/edge3".to_string(),
            cid: "cid_edge3".to_string(),
            text: "abc\n\ndef".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let markdown = format_search_results(&vec![&post], "host", "c d");
        // Should not merge across paragraph break; expect separate bold spans
        assert!(markdown.contains("**c**") && markdown.contains("**d**") && !markdown.contains("**c\n\nd**"), "Matches across paragraph should not merge; got:\n{}", markdown);
    }

    #[test]
    fn test_extract_facets_with_link() {
        // Test extracting facets with link features
        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Check out https://example.com")),
            (CborValue::Text("facets"), CborValue::Array(vec![
                CborValue::Map(vec![
                    (CborValue::Text("index"), CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(10)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(29)),
                    ])),
                    (CborValue::Text("features"), CborValue::Array(vec![
                        CborValue::Map(vec![
                            (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#link")),
                            (CborValue::Text("uri"), CborValue::Text("https://example.com")),
                        ]),
                    ])),
                ]),
            ])),
        ];

        let facets = extract_facets(&post_map);
        assert_eq!(facets.len(), 1, "Should extract one facet");
        assert_eq!(facets[0].index.byte_start, 10);
        assert_eq!(facets[0].index.byte_end, 29);
        assert_eq!(facets[0].features.len(), 1);
    }

    #[test]
    fn test_extract_facets_overlapping_and_utf8() {
        // Overlapping facets and a facet that sits on a UTF-8 multi-byte boundary
        let post_map = vec![
            (
                CborValue::Text("text"),
                CborValue::Text("hélloéworld"), // contains multi-byte chars
            ),
            (
                CborValue::Text("facets"),
                CborValue::Array(vec![
                    // First facet: covers bytes 0..6 (may cut inside UTF-8 but parser returns it)
                    CborValue::Map(vec![
                        (
                            CborValue::Text("index"),
                            CborValue::Map(vec![
                                (CborValue::Text("byteStart"), CborValue::Integer(0)),
                                (CborValue::Text("byteEnd"), CborValue::Integer(6)),
                            ]),
                        ),
                        (
                            CborValue::Text("features"),
                            CborValue::Array(vec![CborValue::Map(vec![
                                (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#tag")),
                                (CborValue::Text("tag"), CborValue::Text("he")),
                            ])]),
                        ),
                    ]),
                    // Second facet overlapping: 4..12
                    CborValue::Map(vec![
                        (
                            CborValue::Text("index"),
                            CborValue::Map(vec![
                                (CborValue::Text("byteStart"), CborValue::Integer(4)),
                                (CborValue::Text("byteEnd"), CborValue::Integer(12)),
                            ]),
                        ),
                        (
                            CborValue::Text("features"),
                            CborValue::Array(vec![CborValue::Map(vec![
                                (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#tag")),
                                (CborValue::Text("tag"), CborValue::Text("loew")),
                            ])]),
                        ),
                    ]),
                ]),
            ),
        ];

        let facets = extract_facets(&post_map);
        // Both facets should be returned and preserve their byte indices
        assert_eq!(facets.len(), 2);
        assert_eq!(facets[0].index.byte_start, 0);
        assert_eq!(facets[1].index.byte_start, 4);
    }

    #[test]
    fn test_extract_embeds_images_and_external() {
        // Build a CBOR-like embed: images and external
        let image_blob = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/jpeg")),
            (CborValue::Text("size"), CborValue::Integer(1234)),
            (CborValue::Text("ref"), CborValue::Text("bafkrei_image_ref")),
        ]);

        let image_map = CborValue::Map(vec![
            (CborValue::Text("alt"), CborValue::Text("A cat")),
            (CborValue::Text("image"), image_blob),
        ]);

        let images_array = CborValue::Array(vec![image_map]);

        let images_embed = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.images")),
            (CborValue::Text("images"), images_array),
        ]);

        let external_map = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.external")),
            (CborValue::Text("external"), CborValue::Map(vec![
                (CborValue::Text("uri"), CborValue::Text("https://example.com")),
                (CborValue::Text("title"), CborValue::Text("Example")),
                (CborValue::Text("description"), CborValue::Text("An example site")),
            ])),
        ]);

        // Test image embed parsing
        let post_map_images = vec![(CborValue::Text("embed"), images_embed)];
        let embeds = extract_embeds(&post_map_images).expect("images embed should parse");
        assert!(!embeds.is_empty());
        match &embeds[0] {
            Embed::Images { images } => {
                assert_eq!(images.len(), 1);
                assert_eq!(images[0].alt.as_deref().unwrap_or(""), "A cat");
            }
            _ => panic!("Expected images embed"),
        }

        // Test external embed parsing
        let post_map_ext = vec![(CborValue::Text("embed"), external_map)];
        let embeds_ext = extract_embeds(&post_map_ext).expect("external embed should parse");
        assert!(!embeds_ext.is_empty());
        match &embeds_ext[0] {
            Embed::External { external } => {
                assert_eq!(external.uri, "https://example.com");
                assert_eq!(external.title, "Example");
            }
            _ => panic!("Expected external embed"),
        }
    }

    #[test]
    fn test_highlight_unicode_multibyte() {
        // Ensure highlighting works with multibyte characters (emoji)
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/ub1".to_string(),
            cid: "cid_ub1".to_string(),
            text: "a😊b".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let markdown = format_search_results(&vec![&post], "host", "😊");
        assert!(markdown.contains("**😊**"), "Emoji should be highlighted correctly; got:\n{}", markdown);
    }

    #[test]
    fn test_extract_facets_with_mention() {
        // Test extracting facets with mention features
        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Hey @alice check this out")),
            (CborValue::Text("facets"), CborValue::Array(vec![
                CborValue::Map(vec![
                    (CborValue::Text("index"), CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(4)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(10)),
                    ])),
                    (CborValue::Text("features"), CborValue::Array(vec![
                        CborValue::Map(vec![
                            (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#mention")),
                            (CborValue::Text("did"), CborValue::Text("did:plc:alice123")),
                        ]),
                    ])),
                ]),
            ])),
        ];

        let facets = extract_facets(&post_map);
        assert_eq!(facets.len(), 1);
        match &facets[0].features[0] {
            FacetFeature::Mention { did } => {
                assert_eq!(did, "did:plc:alice123");
            }
            _ => panic!("Expected mention facet"),
        }
    }

    #[test]
    fn test_fuzzy_search_integration_with_ranking() {
        // Integration test: fuzzy search with proper ranking
        let posts = vec![
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "rustc compiler options".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/2".to_string(),
                cid: "cid2".to_string(),
                text: "rust programming tutorial".to_string(),
                created_at: "2024-01-15T11:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/3".to_string(),
                cid: "cid3".to_string(),
                text: "Rust ownership system explained".to_string(),
                created_at: "2024-01-15T12:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
        ];

        let mut engine = SearchEngine::new();
        let results = engine.search("rust", &posts, |p| p.get_searchable_text());

        assert_eq!(results.len(), 3, "Should find all three posts");
        // Results should be ranked by relevance
        for result in &results {
            // Check that we have actual results (score type checking is internal)
            assert!(result.item.uri.contains("app.bsky.feed.post"));
        }
    }

    #[test]
    fn test_search_with_special_characters_in_query() {
        // Test that search handles special characters gracefully
        let posts = vec![
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Web3 and blockchain technology".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
        ];

        let mut engine = SearchEngine::new();
        // Query with numbers and letters should work
        let results = engine.search("web3", &posts, |p| p.get_searchable_text());
        assert_eq!(results.len(), 1, "Should find post with 'web3'");
    }

    #[test]
    fn test_search_with_unicode_emoji() {
        // Test that search works with unicode and emoji
        let posts = vec![
            PostRecord {
                uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Love programming ❤️ 🦀".to_string(),
                created_at: "2024-01-15T10:00:00Z".to_string(),
                embeds: None,
                facets: vec![],
            },
        ];

        let mut engine = SearchEngine::new();
        let results = engine.search("programming", &posts, |p| p.get_searchable_text());
        assert_eq!(results.len(), 1, "Should find post even with emoji present");
    }

    #[test]
    fn test_parse_record_with_media_embed() {
        use crate::car::cbor::CborValue;

        // Build a recordWithMedia embed where media is images
        let image_blob = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(12)),
            (CborValue::Text("ref"), CborValue::Text("bafkrei_img")),
        ]);

        let image_map = CborValue::Map(vec![
            (CborValue::Text("alt"), CborValue::Text("An img")),
            (CborValue::Text("image"), image_blob),
        ]);

        let images_array = CborValue::Array(vec![image_map]);

        let media_embed = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.images")),
            (CborValue::Text("images"), images_array),
        ]);

        let record_map = CborValue::Map(vec![
            (CborValue::Text("uri"), CborValue::Text("at://did:plc:test/app.bsky.feed.post/1")),
            (CborValue::Text("cid"), CborValue::Text("cid1")),
        ]);

        let record_with_media = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.recordWithMedia")),
            (CborValue::Text("record"), record_map),
            (CborValue::Text("media"), media_embed),
        ]);

        let post_map = vec![(CborValue::Text("embed"), record_with_media)];

        let embeds = extract_embeds(&post_map).expect("recordWithMedia should parse");
        assert!(!embeds.is_empty());
        match &embeds[0] {
            Embed::RecordWithMedia { record, media } => {
                assert!(record.uri.contains("app.bsky.feed.post"));
                match **media {
                    Embed::Images { .. } => {}
                    _ => panic!("Expected nested media images"),
                }
            }
            _ => panic!("Expected recordWithMedia embed"),
        }
    }

    #[test]
    fn test_parse_blob_ref_map_ref() {
        use crate::car::cbor::CborValue;

        // Build blob map where 'ref' is a Map with '$link'
        let ref_map = CborValue::Map(vec![(CborValue::Text("$link"), CborValue::Text("bafkrei_link"))]);

        let blob_map = vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/jpeg")),
            (CborValue::Text("size"), CborValue::Integer(1234)),
            (CborValue::Text("ref"), ref_map),
        ];

        let br = parse_blob_ref(&blob_map).expect("Should parse blob ref with map ref");
        assert_eq!(br.ref_, "bafkrei_link");
        assert_eq!(br.mime_type, "image/jpeg");
    }

    #[test]
    fn test_parse_blob_ref_text_ref() {
        use crate::car::cbor::CborValue;

        // Build blob map where 'ref' is a plain text value
        let blob_map = vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(10)),
            (CborValue::Text("ref"), CborValue::Text("bafkrei_text_ref")),
        ];

        let br = parse_blob_ref(&blob_map).expect("Should parse blob ref with text ref");
        assert_eq!(br.ref_, "bafkrei_text_ref");
        assert_eq!(br.mime_type, "image/png");
    }

    #[test]
    fn test_extract_facets_skips_empty_features() {
        use crate::car::cbor::CborValue;

        // Build a facet entry that has an index but features array contains an unknown type
        let facets_cbor = vec![
            CborValue::Map(vec![
                (
                    CborValue::Text("index"),
                    CborValue::Map(vec![
                        (CborValue::Text("byteStart"), CborValue::Integer(0)),
                        (CborValue::Text("byteEnd"), CborValue::Integer(5)),
                    ]),
                ),
                (
                    CborValue::Text("features"),
                    CborValue::Array(vec![CborValue::Map(vec![
                        (CborValue::Text("$type"), CborValue::Text("app.unknown.facet#unknown")),
                    ])]),
                ),
            ]),
        ];

        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Test post")),
            (CborValue::Text("facets"), CborValue::Array(facets_cbor)),
        ];

        let facets = extract_facets(&post_map);

        // The facet should be skipped because all features were filtered out
        assert_eq!(facets.len(), 0);
    }

    #[test]
    fn test_parse_image_embed_non_map_returns_none() {
        use crate::car::cbor::CborValue;

        // Pass a text value instead of map
        let non_map = CborValue::Text("not a map");
        let res = parse_image_embed(&non_map);
        assert!(res.is_none(), "parse_image_embed should return None for non-map values");
    }

    #[test]
    fn test_parse_embed_map_unknown_type_returns_none() {
        use crate::car::cbor::CborValue;

        // Embed map with unknown $type should not parse
        let unknown_embed = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.unknown")),
        ]);

        let post_map = vec![(CborValue::Text("embed"), unknown_embed)];
        let embeds = extract_embeds(&post_map);
        assert!(embeds.is_none(), "Unknown embed $type should result in None");
    }

    #[test]
    fn test_parse_image_embed_with_map_ref_image() {
        use crate::car::cbor::CborValue;

        // Build an image blob where 'ref' is a Map containing '$link'
        let ref_map = CborValue::Map(vec![(CborValue::Text("$link"), CborValue::Text("bafkrei_map_ref"))]);

        let image_blob = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(42)),
            (CborValue::Text("ref"), ref_map),
        ]);

        let image_map = CborValue::Map(vec![
            (CborValue::Text("alt"), CborValue::Text("MapRefImg")),
            (CborValue::Text("image"), image_blob),
        ]);

        if let Some(img) = parse_image_embed(&image_map) {
            assert_eq!(img.alt.unwrap_or_default(), "MapRefImg");
            assert_eq!(img.image.ref_, "bafkrei_map_ref");
            assert_eq!(img.image.mime_type, "image/png");
        } else {
            panic!("Expected parse_image_embed to succeed for image blob with map ref");
        }
    }

    #[test]
    fn test_parse_blob_ref_missing_ref_returns_none() {
        use crate::car::cbor::CborValue;

        // Blob map missing 'ref' field should result in None
        let blob_map = vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(10)),
            // no ref entry
        ];

        let br = parse_blob_ref(&blob_map);
        assert!(br.is_none(), "parse_blob_ref should return None when ref is missing");
    }

    #[test]
    fn test_extract_embeds_embed_not_map_returns_none() {
        use crate::car::cbor::CborValue;

        // embed value exists but is not a map
        let post_map = vec![(CborValue::Text("embed"), CborValue::Text("not a map"))];
        let embeds = extract_embeds(&post_map);
        assert!(embeds.is_none(), "extract_embeds should return None for non-map embed value");
    }

    #[test]
    fn test_format_highlight_punctuation_separator() {
        // punctuation separators (like comma) should NOT cause merging across the separator
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/punct".to_string(),
            cid: "cid_punct".to_string(),
            text: "alpha,beta".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let markdown = format_search_results(&vec![&post], "host", "alpha beta");

        // Expect two separate bold spans with the comma between them
        assert!(markdown.contains("**alpha**,**beta**") || markdown.contains("**alpha**, **beta**"), "Punctuation separator should produce separate bold spans; got:\n{}", markdown);
    }

    #[test]
    fn test_extract_facets_malformed_index_skipped() {
        use crate::car::cbor::CborValue;

        // Index is not a map -> facet should be skipped
        let facets_cbor = vec![
            CborValue::Map(vec![
                (CborValue::Text("index"), CborValue::Text("not a map")),
                (
                    CborValue::Text("features"),
                    CborValue::Array(vec![CborValue::Map(vec![
                        (CborValue::Text("$type"), CborValue::Text("app.bsky.richtext.facet#tag")),
                        (CborValue::Text("tag"), CborValue::Text("x")),
                    ])]),
                ),
            ]),
        ];

        let post_map = vec![(CborValue::Text("text"), CborValue::Text("Test")), (CborValue::Text("facets"), CborValue::Array(facets_cbor))];
        let facets = extract_facets(&post_map);
        assert!(facets.is_empty(), "Malformed index map should cause facet to be skipped");
    }

    #[test]
    fn test_parse_record_with_media_external_thumb_map_ref() {
        use crate::car::cbor::CborValue;

        // Build a recordWithMedia embed where media is external and has a thumb with map ref
        let thumb_ref_map = CborValue::Map(vec![(CborValue::Text("$link"), CborValue::Text("bafkrei_thumb"))]);

        let thumb_blob = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(7)),
            (CborValue::Text("ref"), thumb_ref_map),
        ]);

        let external_embed = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.external")),
            (CborValue::Text("external"), CborValue::Map(vec![
                (CborValue::Text("uri"), CborValue::Text("https://example.com")),
                (CborValue::Text("title"), CborValue::Text("Example")),
                (CborValue::Text("description"), CborValue::Text("desc")),
                (CborValue::Text("thumb"), thumb_blob),
            ])),
        ]);

        let record_map = CborValue::Map(vec![
            (CborValue::Text("uri"), CborValue::Text("at://did:plc:test/app.bsky.feed.post/1")),
            (CborValue::Text("cid"), CborValue::Text("cid1")),
        ]);

        let record_with_media = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.recordWithMedia")),
            (CborValue::Text("record"), record_map),
            (CborValue::Text("media"), external_embed),
        ]);

        let post_map = vec![(CborValue::Text("embed"), record_with_media)];

        let embeds = extract_embeds(&post_map).expect("recordWithMedia should parse");
        assert!(!embeds.is_empty());
        match &embeds[0] {
            Embed::RecordWithMedia { record: _, media } => {
                match **media {
                    Embed::External { ref external } => {
                        assert!(external.thumb.is_some());
                        let thumb = external.thumb.as_ref().unwrap();
                        assert_eq!(thumb.ref_, "bafkrei_thumb");
                    }
                    _ => panic!("Expected nested external media"),
                }
            }
            _ => panic!("Expected recordWithMedia embed"),
        }
    }

    #[test]
    fn test_image_embed_mime_subtype_missing_defaults_to_jpeg_in_url() {
        // When mimeType lacks a subtype, the format_search_results should fallback to 'jpeg' extension
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/img1".to_string(),
            cid: "cid_img1".to_string(),
            text: "Image post".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: Some(vec![Embed::Images { images: vec![ImageEmbed {
                alt: Some("Alt text".to_string()),
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "bafkrei_imgref".to_string(),
                    mime_type: "image".to_string(), // no '/'
                    size: 1,
                },
            }]}]),
            facets: vec![],
        };

        let md = format_search_results(&vec![&post], "host", "Alt");
        assert!(md.contains("@jpeg"), "Should use @jpeg fallback when mime subtype missing; got:\n{}", md);
    }

    #[test]
    fn test_format_search_results_empty_query_preserves_text() {
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/emptyq".to_string(),
            cid: "cid_emptyq".to_string(),
            text: "No highlight here".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let md = format_search_results(&vec![&post], "host", "");
        // The quoted text should equal the original text (no ** markers)
        assert!(md.contains("> No highlight here"), "Empty query should preserve original text; got:\n{}", md);
    }

    #[test]
    fn test_post_id_extraction_edge_cases() {
        // Trailing slash case
        let post_trailing = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/99/".to_string(),
            cid: "cid99".to_string(),
            text: "Trailing".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let md_trail = format_search_results(&vec![&post_trailing], "me", "Trailing");
        // Because split('/').last() yields an empty string, header should contain "@me/"
        assert!(md_trail.contains("@me/"), "Trailing slash should produce empty id segment in header; got:\n{}", md_trail);

        // No slash (plain id)
        let post_plain = PostRecord {
            uri: "justid".to_string(),
            cid: "cidp".to_string(),
            text: "Plain".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let md_plain = format_search_results(&vec![&post_plain], "me", "Plain");
        assert!(md_plain.contains("@me/justid"), "Plain uri should be used as id; got:\n{}", md_plain);
    }

    #[test]
    fn test_format_search_results_image_alt_none() {
        // When image alt is None, the formatter should use the default "Image" alt
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/noalt".to_string(),
            cid: "cid_noalt".to_string(),
            text: "No alt image".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: Some(vec![Embed::Images { images: vec![ImageEmbed {
                alt: None,
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "bafkrei_noaltref".to_string(),
                    mime_type: "image/png".to_string(),
                    size: 10,
                },
            }]}]),
            facets: vec![],
        };

        let md = format_search_results(&vec![&post], "host", "altterm");
        // Default alt text should be present in markdown
        assert!(md.contains("![Image]"), "Should render default alt when alt missing; got:\n{}", md);
    }

    #[test]
    fn test_parse_external_embed_thumb_as_text_ref() {
        use crate::car::cbor::CborValue;

        // Build external embed where 'thumb' is a direct text ref (not a blob map)
        let external_map = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.external")),
            (CborValue::Text("external"), CborValue::Map(vec![
                (CborValue::Text("uri"), CborValue::Text("https://ex")),
                (CborValue::Text("title"), CborValue::Text("Ex")),
                (CborValue::Text("description"), CborValue::Text("d")),
                (CborValue::Text("thumb"), CborValue::Text("bafkrei_thumb_text")),
            ])),
        ]);

        let post_map = vec![(CborValue::Text("embed"), external_map)];
        // The thumb is not a blob map, so parse_external_embed should treat it as absent
        let embeds = extract_embeds(&post_map).expect("external embed should parse");
        match &embeds[0] {
            Embed::External { external } => {
                // thumb should be None because parse_external_embed expects a blob map
                assert!(external.thumb.is_none(), "Thumb should be None for non-map thumb");
            }
            _ => panic!("Expected external embed"),
        }
    }

    #[test]
    fn test_parse_blob_ref_map_without_link_returns_none() {
        use crate::car::cbor::CborValue;

        // Build blob map where 'ref' is a Map but missing '$link' key
        let bad_ref_map = CborValue::Map(vec![(CborValue::Text("not_link"), CborValue::Text("x"))]);

        let blob_map = vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(5)),
            (CborValue::Text("ref"), bad_ref_map),
        ];

        let br = parse_blob_ref(&blob_map);
        assert!(br.is_none(), "parse_blob_ref should return None when '$link' missing in map ref");
    }

    // --- New tests added: execute_search edge flows and additional embed parsing branches ---

    #[tokio::test]
    async fn test_execute_search_empty_query_normalizes_to_error() {
        // Query with only whitespace should normalize to empty and produce InvalidInput
        let args = crate::cli::SearchArgs {
            from: "test.bsky.social".to_string(),
            query: "   \n\t  ".to_string(),
            limit: None,
        };

        let res = execute_search(args).await;
        assert!(res.is_err(), "Expected error when query normalizes to empty");
        match res {
            Err(AppError::InvalidInput(msg)) => assert!(msg.contains("Query is empty after normalization")),
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_search_invalid_account_error() {
        // Invalid account string should be rejected by validate_account before resolution
        let args = crate::cli::SearchArgs {
            from: "".to_string(),
            query: "hello".to_string(),
            limit: None,
        };

        let res = execute_search(args).await;
        assert!(res.is_err(), "Expected error for invalid account");
        match res {
            Err(AppError::InvalidInput(_)) => {}
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn test_collect_posts_from_maps_builds_post() {
        use crate::car::cbor::CborValue;
        use std::collections::HashMap;

        // Build a decoded post map (as produced by decode_cbor)
        let post_map = vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.feed.post")),
            (CborValue::Text("text"), CborValue::Text("Hello from map")),
            (CborValue::Text("createdAt"), CborValue::Text("2025-11-08T00:00:00Z")),
        ];

        let decoded = vec![("app.bsky.feed.post".to_string(), post_map, "cid1".to_string())];

        let mut mapping = HashMap::new();
        mapping.insert("cid1".to_string(), "rkey1".to_string());

        let posts = collect_posts_from_maps("did:plc:test", &decoded, &mapping);

        assert_eq!(posts.len(), 1);
        assert!(posts[0].uri.contains("rkey1"));
        assert_eq!(posts[0].text, "Hello from map");
    }

    #[tokio::test]
    async fn test_run_search_on_posts_success() {
        // Build a single PostRecord and run the search pipeline
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
            cid: "cid1".to_string(),
            text: "Find me by keyword".to_string(),
            created_at: "2025-11-08T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let res = run_search_on_posts(&[post.clone()], "test.handle", "keyword", Some(10)).await;
        assert!(res.is_ok(), "Search should return results");
        if let Ok(tool) = res {
            // Inspect the returned ToolResult content text
            assert!(!tool.content.is_empty(), "ToolResult should contain content");
            let s = &tool.content[0].text;
            assert!(s.contains("Find me") || s.contains("keyword") || s.contains("Find"));
        }
    }

    #[tokio::test]
    async fn test_handle_search_invalid_args_produces_error_response() {
        // Call the MCP wrapper with invalid args to exercise the error branch
        let id = serde_json::json!("testid");
        let args = serde_json::json!({}); // missing required fields -> serde error

        let resp = handle_search(Some(id.into()), args).await;
        // Should be an error response (error field present)
        assert!(resp.error.is_some(), "Expected McpResponse to carry an error");
    }

    #[test]
    fn test_parse_embed_record_with_media_unknown_media_returns_none() {
        use crate::car::cbor::CborValue;

        // Build a recordWithMedia where the nested media embed has an unknown $type
        let record_map = CborValue::Map(vec![
            (CborValue::Text("uri"), CborValue::Text("at://did:plc:test/app.bsky.feed.post/1")),
            (CborValue::Text("cid"), CborValue::Text("cid1")),
        ]);

        let unknown_media = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.unknown_media")),
        ]);

        let record_with_media = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.recordWithMedia")),
            (CborValue::Text("record"), record_map),
            (CborValue::Text("media"), unknown_media),
        ]);

        let post_map = vec![(CborValue::Text("embed"), record_with_media)];

        let embeds = extract_embeds(&post_map);
        // Because the nested media failed to parse, the top-level parse should return None
        assert!(embeds.is_none(), "recordWithMedia with unknown nested media should not parse");
    }

    #[test]
    fn test_parse_image_embed_missing_image_key_returns_none_direct() {
        use crate::car::cbor::CborValue;

        // Image map that lacks the 'image' key should not parse as an ImageEmbed
        let image_map = CborValue::Map(vec![(CborValue::Text("alt"), CborValue::Text("NoImage"))]);

        let res = parse_image_embed(&image_map);
        assert!(res.is_none(), "parse_image_embed should return None when 'image' key is missing");
    }

    #[test]
    fn test_extract_embeds_images_with_empty_array_returns_images_empty() {
        use crate::car::cbor::CborValue;

        let images_array = CborValue::Array(vec![]);
        let images_embed = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.images")),
            (CborValue::Text("images"), images_array),
        ]);

        let post_map = vec![(CborValue::Text("embed"), images_embed)];
        let embeds = extract_embeds(&post_map).expect("images embed should parse and return an empty images vec");
        match &embeds[0] {
            Embed::Images { images } => assert!(images.is_empty(), "images vector should be empty when embed contains an empty array"),
            _ => panic!("Expected Images embed"),
        }
    }

    #[test]
    fn test_collect_post_from_map_missing_rkey_returns_none() {
        use crate::car::cbor::CborValue;
        use std::collections::HashMap;

        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Hello")),
            (CborValue::Text("createdAt"), CborValue::Text("2025-11-08T00:00:00Z")),
        ];

        let mapping: HashMap<String, String> = HashMap::new();

        let res = collect_post_from_map("did:plc:test", post_map.as_slice(), "cid_missing", &mapping);
        assert!(res.is_none(), "Should return None when CID->rkey mapping missing");
    }

    #[tokio::test]
    async fn test_run_search_on_posts_no_matches_returns_not_found() {
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
            cid: "cid1".to_string(),
            text: "No match here".to_string(),
            created_at: "2025-11-08T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };

        let res = run_search_on_posts(&[post], "handle", "something", None).await;
        assert!(res.is_err(), "Expected NotFound for no matching posts");
        match res {
            Err(AppError::NotFound(_)) => {}
            other => panic!("Unexpected result: {:?}", other),
        }
    }

    #[test]
    fn test_parse_blob_ref_integer_ref_returns_none() {
        use crate::car::cbor::CborValue;

        // Build blob map where 'ref' is an integer (invalid shape)
        let blob_map = vec![
            (CborValue::Text("$type"), CborValue::Text("blob")),
            (CborValue::Text("mimeType"), CborValue::Text("image/png")),
            (CborValue::Text("size"), CborValue::Integer(5)),
            (CborValue::Text("ref"), CborValue::Integer(42)),
        ];

        let br = parse_blob_ref(&blob_map);
        assert!(br.is_none(), "parse_blob_ref should return None when ref is a non-text, non-map value");
    }

    #[test]
    fn test_format_search_results_images_use_mime_subtype() {
        // Ensure image mime subtype is used to form the CDN URL extension (e.g., @png)
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/img2".to_string(),
            cid: "cid_img2".to_string(),
            text: "Image post with png".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: Some(vec![Embed::Images { images: vec![ImageEmbed {
                alt: Some("PNG image".to_string()),
                image: BlobRef {
                    type_: "blob".to_string(),
                    ref_: "bafkrei_pngref".to_string(),
                    mime_type: "image/png".to_string(),
                    size: 10,
                },
            }]}]),
            facets: vec![],
        };

        let md = format_search_results(&vec![&post], "host", "PNG");
        assert!(md.contains("@png"), "Should use @png when mime subtype is 'png'; got:\n{}", md);
    }

    #[test]
    fn test_extract_embeds_external_missing_title_returns_none() {
        use crate::car::cbor::CborValue;

        // External embed missing the required 'title' should fail to parse
        let external_map = CborValue::Map(vec![
            (CborValue::Text("$type"), CborValue::Text("app.bsky.embed.external")),
            (CborValue::Text("external"), CborValue::Map(vec![
                (CborValue::Text("uri"), CborValue::Text("https://ex")),
                // title missing
                (CborValue::Text("description"), CborValue::Text("d")),
            ])),
        ]);

        let post_map = vec![(CborValue::Text("embed"), external_map)];
        let embeds = extract_embeds(&post_map);
        assert!(embeds.is_none(), "External embed missing title should not parse");
    }

    #[test]
    fn test_collect_post_from_map_success() {
        use crate::car::cbor::CborValue;
        use std::collections::HashMap;

        let post_map = vec![
            (CborValue::Text("text"), CborValue::Text("Collected post")),
            (CborValue::Text("createdAt"), CborValue::Text("2025-11-08T12:00:00Z")),
        ];

        let mut mapping = HashMap::new();
        mapping.insert("cid_ok".to_string(), "rkey_ok".to_string());

        let res = collect_post_from_map("did:plc:alice", post_map.as_slice(), "cid_ok", &mapping);
        assert!(res.is_some(), "Should build PostRecord when mapping present");
        let p = res.unwrap();
        assert!(p.uri.contains("rkey_ok"));
        assert_eq!(p.text, "Collected post");
    }

    #[test]
    fn test_parse_record_embed_direct() {
        use crate::car::cbor::CborValue;

        let record_map = CborValue::Map(vec![
            (CborValue::Text("uri"), CborValue::Text("at://did:plc:alice/app.bsky.feed.post/77")),
            (CborValue::Text("cid"), CborValue::Text("cid77")),
        ]);

        if let Some(rec) = parse_record_embed(match &record_map { CborValue::Map(m) => m.as_slice(), _ => &[] }) {
            assert!(rec.uri.contains("app.bsky.feed.post/77"));
            assert_eq!(rec.cid, "cid77");
        } else {
            panic!("parse_record_embed should succeed for valid record map");
        }
    }
}
