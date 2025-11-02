//! Search tool implementation
//!
//! Implements the `search(from, query)` MCP tool

use crate::bluesky::did::DidResolver;
use crate::bluesky::provider::RepositoryProvider;
use crate::bluesky::records::PostRecord;
use crate::cli::SearchArgs;
use crate::error::{normalize_text, validate_account, validate_query, AppError};
use crate::mcp::{McpResponse, ToolResult};
use crate::search::SearchEngine;
use anyhow::Result;

use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::debug;

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
            if let Ok(serde_cbor::Value::Map(post_map)) =
                serde_cbor::from_slice::<serde_cbor::Value>(&cbor_data)
            {
                let text = match post_map.get(&serde_cbor::Value::Text("text".to_string())) {
                    Some(serde_cbor::Value::Text(t)) => t.clone(),
                    _ => return None,
                };

                let created_at =
                    match post_map.get(&serde_cbor::Value::Text("createdAt".to_string())) {
                        Some(serde_cbor::Value::Text(t)) => t.clone(),
                        _ => return None,
                    };

                // Look up rkey from CID->rkey mapping
                let collection_rkey = cid_to_rkey
                    .get(&cid_str)
                    .map(|s| s.as_str())
                    .unwrap_or("app.bsky.feed.post/unknown");

                Some(PostRecord {
                    uri: format!("at://{}/{}", did_str, collection_rkey),
                    cid: cid_str,
                    text,
                    created_at,
                    embeds: Vec::new(), // TODO: Convert embeds if needed in future
                    facets: Vec::new(), // TODO: Convert facets if needed in future
                })
            } else {
                None
            }
        })
        .collect();

    debug!("Extracted {} post records with rkeys", posts.len());

    // Use new fuzzy search engine
    let mut search_engine = SearchEngine::new();
    
    let search_results = search_engine.search(
        &search_args.query,
        &posts,
        |post| post.get_searchable_text(),
    );

    // Extract just the posts from search results
    let mut matching_posts: Vec<&PostRecord> = search_results
        .iter()
        .map(|r| &r.item)
        .collect();

    // Sort by created_at descending (ISO8601 lexicographic)
    matching_posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply limit (default 50, no maximum - will fetch as many as requested)
    let limit = search_args.limit.unwrap_or(50);
    if matching_posts.len() > limit {
        matching_posts.truncate(limit);
    }

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
        let highlighted_text = highlight_query(&post.text, query);
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

    if !lower_text.contains(&lower_query) {
        return text.to_string();
    }

    let mut result = String::new();
    let mut last_end = 0;

    while let Some(start) = lower_text[last_end..].find(&lower_query) {
        let absolute_start = last_end + start;
        let absolute_end = absolute_start + query.len();

        // Add text before match
        result.push_str(&text[last_end..absolute_start]);

        // Add highlighted match
        result.push_str("**");
        result.push_str(&text[absolute_start..absolute_end]);
        result.push_str("**");

        last_end = absolute_end;
    }

    // Add remaining text
    result.push_str(&text[last_end..]);

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
        assert!(results.iter().any(|r| r.item.text.contains("Hello everyone")));

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
}
