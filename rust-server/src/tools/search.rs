//! Search tool implementation
//!
//! Implements the `search(account, query)` MCP tool

use crate::bluesky::car::CarProcessor;
use crate::bluesky::did::DidResolver;
use crate::bluesky::records::PostRecord;
use crate::error::{normalize_text, validate_account, validate_query, AppError};
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Search tool arguments
#[derive(Debug, Deserialize)]
struct SearchArgs {
    account: String,
    query: String,
    #[serde(default)]
    limit: Option<usize>,
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

    // Validate parameters
    validate_account(&search_args.account)?;
    validate_query(&search_args.query)?;

    info!(
        "Search request for account: {}, query: '{}'",
        search_args.account, search_args.query
    );

    // Normalize query as specified
    let normalized_query = normalize_text(&search_args.query);
    if normalized_query.is_empty() {
        return Err(AppError::InvalidInput("Query is empty after normalization".to_string()));
    }

    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(&search_args.account).await?;

    // Determine the handle for display
    let display_handle = if search_args.account.starts_with("did:plc:") {
        // If input was a DID, we might not have the handle - use DID for now
        search_args.account.clone()
    } else {
        search_args.account.strip_prefix('@').unwrap_or(&search_args.account).to_string()
    };

    debug!("Resolved {} to DID: {}", search_args.account, did);

    // Fetch and process repository
    let car_processor = CarProcessor::new()?;
    let car_data = car_processor.fetch_repo(&did).await?;

    debug!("Fetched CAR data: {} bytes", car_data.len());

    // Extract post records (parse-only)
    let posts = car_processor.extract_posts(&car_data).await?;

    debug!("Extracted {} post records", posts.len());

    // Search posts
    let mut matching_posts = search_posts(&posts, &normalized_query);

    // Sort by created_at descending (ISO8601 lexicographic)
    matching_posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply limit (default 50, max 200)
    let limit = search_args.limit.unwrap_or(50).clamp(1, 200);
    if matching_posts.len() > limit { matching_posts.truncate(limit); }

    if matching_posts.is_empty() {
        return Err(AppError::NotFound(format!(
            "No posts found matching query '{}' for account {}",
            search_args.query, search_args.account
        )));
    }

    info!(
        "Found {} matching posts for query: '{}'",
        matching_posts.len(),
        search_args.query
    );

    // Resolve URIs only for matched posts
    use std::collections::HashSet;
    let needed: HashSet<String> = matching_posts
        .iter()
        .filter(|p| !p.cid.is_empty())
        .map(|p| p.cid.clone())
        .collect();
    let cid_to_uri = car_processor.resolve_uris_for_cids(&did, &needed).await?;

    // Enrich matched posts with URIs
    let mut enriched: Vec<PostRecord> = matching_posts
        .into_iter()
        .map(|p| {
            let mut pr = p.clone();
            if pr.uri.is_empty() {
                if let Some(u) = cid_to_uri.get(&pr.cid) {
                    pr.uri = u.clone();
                }
            }
            pr
        })
        .collect();
    let enriched_refs: Vec<&PostRecord> = enriched.iter().collect();

    // Format results as markdown
    let markdown = format_search_results(&enriched_refs, &display_handle, &search_args.query);

    Ok(ToolResult::text(markdown))
}

/// Search posts for query matches
fn search_posts<'a>(posts: &'a [PostRecord], query: &str) -> Vec<&'a PostRecord> {
    let lower_query = query.to_lowercase();
    
    posts
        .iter()
        .filter(|post| {
            // Check if any searchable text contains the query
            post.get_searchable_text()
                .iter()
                .any(|text| normalize_text(text).to_lowercase().contains(&lower_query))
        })
        .collect()
}

/// Format search results as markdown
fn format_search_results(posts: &[&PostRecord], handle: &str, query: &str) -> String {
    let mut markdown = format!("# Search Results for \"{}\" in @{}\n\n", query, handle);

    for (i, post) in posts.iter().enumerate() {
        markdown.push_str(&format!("## Post {}\n", i + 1));
        markdown.push_str(&post.to_markdown(handle, query));
        
        // Add separator between posts
        if i < posts.len() - 1 {
            markdown.push_str("---\n\n");
        }
    }

    // Add summary
    let summary = if posts.len() == 1 {
        "Found 1 matching post".to_string()
    } else {
        format!("Found {} matching posts", posts.len())
    };

    // Add summary at the end
    markdown.push_str(&format!("\n---\n\n*{}*\n", summary));

    markdown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bluesky::records::PostRecord;
    use serde_json::json;

    #[tokio::test]
    async fn test_search_args_parsing() {
        let args = json!({
            "account": "test.bsky.social",
            "query": "hello world"
        });

        let parsed: SearchArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.account, "test.bsky.social");
        assert_eq!(parsed.query, "hello world");
    }

    #[test]
    fn test_search_posts() {
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

        let results = search_posts(&posts, "hello");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|p| p.text.contains("Hello world")));
        assert!(results.iter().any(|p| p.text.contains("Hello everyone")));

        let results = search_posts(&posts, "programming");
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("programming"));

        let results = search_posts(&posts, "nonexistent");
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

        assert!(markdown.contains("# Search Results for \"hello\" in @test.bsky.social"));
        assert!(markdown.contains("## Post 1"));
        assert!(markdown.contains("Found 1 matching post"));
    }
}