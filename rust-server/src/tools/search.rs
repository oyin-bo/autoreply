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

/// Handle search tool call
pub async fn handle_search(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds as specified
    match timeout(Duration::from_secs(120), handle_search_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Search request exceeded 120 second timeout"),
    }
}

async fn handle_search_impl(args: Value) -> Result<ToolResult, AppError> {
    // Parse arguments
    let search_args: SearchArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    // Execute using shared implementation
    execute_search(search_args).await
}

/// Execute search tool (shared implementation for MCP and CLI)
pub async fn execute_search(search_args: SearchArgs) -> Result<ToolResult, AppError> {
    // Validate parameters
    validate_account(&search_args.from)?;
    validate_query(&search_args.query)?;

    debug!(
        "Search request for account: {}, query: '{}'",
        search_args.from, search_args.query
    );

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

    // Determine the handle for display
    let display_handle = if search_args.from.starts_with("did:plc:") {
        // If input was a DID, we might not have the handle - use DID for now
        search_args.from.clone()
    } else {
        search_args
            .from
            .strip_prefix('@')
            .unwrap_or(&search_args.from)
            .to_string()
    };

    debug!("Resolved {} to DID: {:?}", search_args.from, did);

    let did_str = did
        .as_ref()
        .ok_or_else(|| AppError::DidResolveFailed("DID resolution failed".to_string()))?;

    // Get CAR file and extract CID->rkey mappings using MST
    let provider = RepositoryProvider::new()?;
    let car_path = provider.fetch_repo_car(did_str).await?;
    let car_bytes = tokio::fs::read(&car_path)
        .await
        .map_err(|e| AppError::CacheError(format!("Failed to read CAR file: {}", e)))?;

    debug!("Extracting CID->rkey mappings from MST for collection app.bsky.feed.post");
    let cid_to_rkey =
        crate::bluesky::mst::extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post")
            .map_err(|e| {
                AppError::RepoParseFailed(format!("Failed to extract MST mappings: {:?}", e))
            })?;

    debug!("Extracted {} CID->rkey mappings", cid_to_rkey.len());

    // Get posts using streaming iterator with CID tracking
    let records = provider.records(did_str).await?;

    // Stream through records and collect posts with proper rkeys
    let posts: Vec<PostRecord> = records
        .filter_map(|record_result| {
            let (record_type, cbor_data, cid_str) = record_result.ok()?;

            // Only process post records
            if record_type != "app.bsky.feed.post" {
                return None;
            }

            // Decode CBOR data to extract post fields
            if let Ok(CborValue::Map(post_map)) = decode_cbor(&cbor_data) {
                let text = match get_text_field(&post_map, "text") {
                    Some(t) => t.to_string(),
                    _ => return None,
                };

                let created_at = match get_text_field(&post_map, "createdAt") {
                    Some(t) => t.to_string(),
                    _ => return None,
                };

                // Extract facets using helper function
                let facets = extract_facets(&post_map);

                // Look up rkey from CID->rkey mapping; skip if not present
                let collection_rkey = match cid_to_rkey.get(&cid_str) {
                    Some(s) => s.as_str(),
                    None => return None,
                };

                Some(PostRecord {
                    uri: format!("at://{}/{}", did_str, collection_rkey),
                    cid: cid_str,
                    text,
                    created_at,
                    embeds: Vec::new(), // TODO: Convert embeds if needed in future
                    facets,
                })
            } else {
                None
            }
        })
        .collect();

    debug!("Extracted {} post records with rkeys", posts.len());

    // Use new fuzzy search engine
    let mut search_engine = SearchEngine::new();

    let search_results = search_engine.search(&search_args.query, &posts, |post| {
        post.get_searchable_text()
    });

    // Extract just the posts from search results, preserving relevance order
    // (highest score first as determined by SearchEngine). We'll keep this
    // ordering to prioritize quality of match, and apply recency only as a
    // secondary tie-breaker implicitly when scores are equal (engine already
    // returned a stable order for equals).
    let limit = search_args.limit.unwrap_or(50);
    let matching_posts: Vec<&PostRecord> =
        search_results.iter().take(limit).map(|r| &r.item).collect();

    // Limit applied above via iterator .take(limit)

    if matching_posts.is_empty() {
        return Err(AppError::NotFound(format!(
            "No posts found matching query '{}' for account {}",
            search_args.query, search_args.from
        )));
    }

    debug!(
        "Found {} matching posts for query: '{}' (fuzzy search with {} total matches before limit)",
        matching_posts.len(),
        search_args.query,
        search_results.len()
    );

    // Construct full URIs from DID + collection + rkey (already extracted from CAR)
    let enriched: Vec<PostRecord> = matching_posts
        .into_iter()
        .map(|p| {
            let mut pr = p.clone();
            // URI format: at://{did}/app.bsky.feed.post/{rkey}
            if !pr.uri.is_empty() && !pr.uri.starts_with("at://") {
                pr.uri = format!(
                    "at://{}/app.bsky.feed.post/{}",
                    did.as_deref().unwrap_or("unknown"),
                    pr.uri
                );
            }
            pr
        })
        .collect();
    let enriched_refs: Vec<&PostRecord> = enriched.iter().collect();

    // Format results as markdown
    let markdown = format_search_results(&enriched_refs, &display_handle, &search_args.query);

    Ok(ToolResult::text(markdown))
}

/// Format search results as markdown per docs/16-mcp-schemas.md spec
fn format_search_results(posts: &[&PostRecord], handle: &str, query: &str) -> String {
    use crate::tools::post_format::*;
    use std::collections::HashMap;

    let mut markdown = format!("# Search Results · {} posts\n\n", posts.len());

    let mut seen_posts: HashMap<String, String> = HashMap::new();

    for post in posts {
        let rkey = extract_rkey(&post.uri);
        let full_id = format!("{}/{}", handle, rkey);

        // Author ID line
        let author_id = compact_post_id(handle, rkey, &seen_posts);
        markdown.push_str(&format!("{}\n", author_id));
        seen_posts.insert(full_id, post.uri.clone());

        // Blockquote content (with highlighting preserved inside quote)
        // First apply facets, then highlight
        let text_with_facets = if !post.facets.is_empty() {
            crate::tools::post_format::apply_facets_to_text(&post.text, &post.facets)
        } else {
            post.text.clone()
        };
        let highlighted_text = highlight_query(&text_with_facets, query);
        markdown.push_str(&blockquote_content(&highlighted_text));
        markdown.push('\n');

        // Stats and timestamp (search results don't have engagement stats from CAR)
        let timestamp = format_timestamp(&post.created_at);
        markdown.push_str(&format!("{}\n", timestamp));

        markdown.push('\n');
    }

    markdown
}

/// Highlight query matches in text with **bold** markdown
fn highlight_query(text: &str, query: &str) -> String {
    if query.is_empty() {
        return text.to_string();
    }

    // Simple case-insensitive highlighting
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    if lower_text.contains(&lower_query) {
        let mut result = String::new();
        let mut last_end = 0;
        while let Some(start) = lower_text[last_end..].find(&lower_query) {
            let absolute_start = last_end + start;
            let absolute_end = absolute_start + query.len();
            result.push_str(&text[last_end..absolute_start]);
            result.push_str("**");
            result.push_str(&text[absolute_start..absolute_end]);
            result.push_str("**");
            last_end = absolute_end;
        }
        result.push_str(&text[last_end..]);
        return result;
    }

    // Fallback: fuzzy subsequence highlighting (bold the matched characters
    // of the query in order). This provides visual feedback even when the
    // match isn't a contiguous substring.
    let mut result = String::with_capacity(text.len() + query.len() * 2);
    let mut qi = 0usize;
    let qchars: Vec<char> = lower_query.chars().collect();
    for (i, ch) in text.chars().enumerate() {
        if qi < qchars.len() {
            // Compare lowercased versions without allocating per-char
            let tch = ch.to_lowercase().next().unwrap_or(ch);
            if tch == qchars[qi] {
                result.push_str("**");
                result.push(ch);
                result.push_str("**");
                qi += 1;
                continue;
            }
        }
        result.push(ch);
        // avoid unused i warning
        let _ = i;
    }
    result
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
}
