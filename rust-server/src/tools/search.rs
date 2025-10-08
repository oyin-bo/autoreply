//! Search tool implementation
//!
//! Implements the `search(from, query, login)` MCP tool

use crate::bluesky::did::DidResolver;
use crate::bluesky::provider::RepositoryProvider;
use crate::bluesky::records::PostRecord;
use crate::cli::SearchArgs;
use crate::error::{normalize_text, validate_query, AppError};
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;

use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

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
    // Validate that either from or login is provided
    if search_args.from.is_none() && search_args.login.is_none() {
        return Err(AppError::InvalidInput(
            "Either 'from' or 'login' parameter must be provided".to_string(),
        ));
    }

    // Validate query
    validate_query(&search_args.query)?;

    // Normalize query as specified
    let normalized_query = normalize_text(&search_args.query);
    if normalized_query.is_empty() {
        return Err(AppError::InvalidInput(
            "Query is empty after normalization".to_string(),
        ));
    }

    let limit = search_args.limit.unwrap_or(50).clamp(1, 200);

    // Perform search based on provided parameters
    let (car_posts, api_posts, display_handle) = if let Some(ref login) = search_args.login {
        // Login-based search
        let normalized_login = normalize_handle(login);
        
        info!(
            "Search request with login: '{}', query: '{}'",
            normalized_login, search_args.query
        );

        // Perform API search with authenticated user
        let api_results = search_via_api(&normalized_login, &search_args.query, limit).await?;
        
        // If from is also provided, perform CAR search too
        let car_results = if let Some(ref from) = search_args.from {
            validate_from(from)?;
            Some(perform_car_search(from, &normalized_query, limit).await?)
        } else {
            None
        };

        let handle = normalized_login;
        (car_results, Some(api_results), handle)
    } else if let Some(ref from) = search_args.from {
        // Traditional CAR-only search
        validate_from(from)?;
        
        info!(
            "Search request for from: {}, query: '{}'",
            from, search_args.query
        );

        let results = perform_car_search(from, &normalized_query, limit).await?;
        let display_handle = if from.starts_with("did:plc:") {
            from.clone()
        } else {
            from.strip_prefix('@').unwrap_or(from).to_string()
        };

        (Some(results), None, display_handle)
    } else {
        unreachable!("Should have been caught by earlier validation");
    };

    // Merge and deduplicate results
    let merged_posts = merge_and_deduplicate_results(car_posts, api_posts);

    if merged_posts.is_empty() {
        return Err(AppError::NotFound(format!(
            "No posts found matching query '{}'",
            search_args.query
        )));
    }

    info!(
        "Found {} matching posts for query: '{}'",
        merged_posts.len(),
        search_args.query
    );

    // Format results as markdown
    let markdown = format_search_results(&merged_posts.iter().collect::<Vec<_>>(), &display_handle, &search_args.query);

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
        markdown.push_str(&post.to_markdown(handle, query));

        // Add separator between posts
        if i < posts.len() - 1 {
            markdown.push('\n');
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

/// Normalize handle by removing @ prefix and trimming whitespace, converting to lowercase
fn normalize_handle(handle: &str) -> String {
    handle.trim().trim_start_matches('@').to_lowercase()
}

/// Validate from parameter (handle or DID)
fn validate_from(from: &str) -> Result<(), AppError> {
    let trimmed = from.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("'from' parameter cannot be empty".to_string()));
    }
    Ok(())
}

/// Perform CAR-based search on a user's repository
async fn perform_car_search(
    from: &str,
    normalized_query: &str,
    limit: usize,
) -> Result<Vec<PostRecord>, AppError> {
    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(from).await?;

    debug!("Resolved {} to DID: {:?}", from, did);

    // Get posts using streaming iterator
    let provider = RepositoryProvider::new()?;
    let records = provider
        .records(
            did.as_ref()
                .ok_or_else(|| AppError::DidResolveFailed("DID resolution failed".to_string()))?,
        )
        .await?;

    // Stream through records and collect posts
    let posts: Vec<PostRecord> = records
        .filter_map(|record_result| {
            let (record_type, cbor_data) = record_result.ok()?;

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

                // For now, we'll use a placeholder for rkey - we'd need to extract it from the MST key
                // This is a limitation of the current approach vs the specialized get_posts method
                Some(PostRecord {
                    uri: format!(
                        "at://{}/app.bsky.feed.post/unknown",
                        did.as_deref().unwrap_or("unknown")
                    ),
                    cid: "unknown".to_string(), // Would need CID from CAR entry
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

    debug!(
        "Extracted {} post records using streaming iterator",
        posts.len()
    );

    // Search posts
    let mut matching_posts = search_posts(&posts, normalized_query);

    // Sort by created_at descending (ISO8601 lexicographic)
    matching_posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply limit
    if matching_posts.len() > limit {
        matching_posts.truncate(limit);
    }

    // Construct full URIs from DID + collection + rkey
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

    Ok(enriched)
}

/// Perform authenticated API search using BlueSky searchPosts endpoint
async fn search_via_api(
    login: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<PostRecord>, AppError> {
    use crate::auth::storage::CredentialStorage;
    use crate::auth::session::SessionManager;

    // Load credentials from storage
    let storage = CredentialStorage::new()?;
    let credentials = storage
        .get_credentials(login)
        .map_err(|_| {
            AppError::Authentication(format!(
                "Login '{}' not found. Please login first using the login tool.",
                login
            ))
        })?;

    // Create or restore session
    let session = if let Ok(Some(existing_session)) = storage.get_session(login) {
        existing_session
    } else {
        // No existing session, create new one
        let session_manager = SessionManager::new()?;
        session_manager.login(&credentials).await?
    };

    // Make API request to searchPosts endpoint
    let client = crate::http::client_with_timeout(std::time::Duration::from_secs(30));
    let url = format!("{}/xrpc/app.bsky.feed.searchPosts", session.service);

    #[derive(serde::Deserialize)]
    struct SearchPostsResponse {
        posts: Vec<ApiPost>,
    }

    #[derive(serde::Deserialize)]
    struct ApiPost {
        uri: String,
        cid: String,
        record: ApiPostRecord,
        #[serde(default)]
        #[serde(rename = "likeCount")]
        #[allow(dead_code)]
        like_count: Option<u32>,
        #[serde(default)]
        #[serde(rename = "replyCount")]
        #[allow(dead_code)]
        reply_count: Option<u32>,
        #[serde(default)]
        #[serde(rename = "repostCount")]
        #[allow(dead_code)]
        repost_count: Option<u32>,
        #[serde(default)]
        #[serde(rename = "quoteCount")]
        #[allow(dead_code)]
        quote_count: Option<u32>,
    }

    #[derive(serde::Deserialize)]
    struct ApiPostRecord {
        text: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    }

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Authenticated search failed: {}",
            response.status()
        )));
    }

    let search_result: SearchPostsResponse = response.json().await?;

    // Convert API posts to PostRecord format
    let posts: Vec<PostRecord> = search_result
        .posts
        .into_iter()
        .map(|api_post| PostRecord {
            uri: api_post.uri,
            cid: api_post.cid,
            text: api_post.record.text,
            created_at: api_post.record.created_at,
            embeds: Vec::new(), // TODO: Convert embeds from API if needed
            facets: Vec::new(), // TODO: Convert facets from API if needed
        })
        .collect();

    debug!("Found {} posts via API search", posts.len());
    Ok(posts)
}

/// Merge and deduplicate search results from CAR and API sources
fn merge_and_deduplicate_results(
    car_posts: Option<Vec<PostRecord>>,
    api_posts: Option<Vec<PostRecord>>,
) -> Vec<PostRecord> {
    use std::collections::HashMap;

    let mut posts_by_uri: HashMap<String, PostRecord> = HashMap::new();

    // Add CAR posts first
    if let Some(car_results) = car_posts {
        for post in car_results {
            posts_by_uri.insert(post.uri.clone(), post);
        }
    }

    // Add or merge API posts
    // API posts have priority for stats merging
    if let Some(api_results) = api_posts {
        for post in api_results {
            posts_by_uri.insert(post.uri.clone(), post);
        }
    }

    // Convert back to Vec and sort by created_at descending
    let mut merged: Vec<PostRecord> = posts_by_uri.into_values().collect();
    merged.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    merged
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
        assert_eq!(parsed.from, Some("test.bsky.social".to_string()));
        assert_eq!(parsed.query, "hello world");

        // Test with login parameter
        let args_with_login = json!({
            "login": "alice.bsky.social",
            "query": "test query"
        });

        let parsed2: SearchArgs = serde_json::from_value(args_with_login).unwrap();
        assert_eq!(parsed2.login, Some("alice.bsky.social".to_string()));
        assert_eq!(parsed2.query, "test query");
        assert_eq!(parsed2.from, None);
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
        assert!(markdown.contains("@test.bsky.social/1"));
        assert!(markdown.contains("> **Hello** world"));
        assert!(markdown.contains("Found 1 matching post"));
    }

    #[test]
    fn test_normalize_handle() {
        assert_eq!(normalize_handle("alice.bsky.social"), "alice.bsky.social");
        assert_eq!(normalize_handle("@alice.bsky.social"), "alice.bsky.social");
        assert_eq!(normalize_handle("  @bob.bsky.social  "), "bob.bsky.social");
        assert_eq!(normalize_handle("   carol.bsky.social"), "carol.bsky.social");
        // Test case-insensitive normalization
        assert_eq!(normalize_handle("Alice.Bsky.Social"), "alice.bsky.social");
        assert_eq!(normalize_handle("@ALICE.BSKY.SOCIAL"), "alice.bsky.social");
        assert_eq!(normalize_handle("  @Bob.BSKY.social  "), "bob.bsky.social");
    }

    #[test]
    fn test_merge_and_deduplicate_results() {
        let car_posts = vec![
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Post 1 from CAR".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/2".to_string(),
                cid: "cid2".to_string(),
                text: "Post 2 from CAR".to_string(),
                created_at: "2024-01-02T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
        ];

        let api_posts = vec![
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/2".to_string(), // Duplicate
                cid: "cid2".to_string(),
                text: "Post 2 from API".to_string(),
                created_at: "2024-01-02T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/3".to_string(),
                cid: "cid3".to_string(),
                text: "Post 3 from API".to_string(),
                created_at: "2024-01-03T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
        ];

        let merged = merge_and_deduplicate_results(Some(car_posts), Some(api_posts));

        // Should have 3 unique posts
        assert_eq!(merged.len(), 3);

        // Should be sorted by created_at descending
        assert_eq!(merged[0].uri, "at://did:plc:123/app.bsky.feed.post/3");
        assert_eq!(merged[1].uri, "at://did:plc:123/app.bsky.feed.post/2");
        assert_eq!(merged[2].uri, "at://did:plc:123/app.bsky.feed.post/1");

        // Duplicate should use API version (later in merge)
        assert!(merged[1].text.contains("API"));
    }

    #[test]
    fn test_merge_car_only() {
        let car_posts = vec![
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Post 1".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
        ];

        let merged = merge_and_deduplicate_results(Some(car_posts), None);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "Post 1");
    }

    #[test]
    fn test_merge_api_only() {
        let api_posts = vec![
            PostRecord {
                uri: "at://did:plc:123/app.bsky.feed.post/1".to_string(),
                cid: "cid1".to_string(),
                text: "Post 1 from API".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                embeds: vec![],
                facets: vec![],
            },
        ];

        let merged = merge_and_deduplicate_results(None, Some(api_posts));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "Post 1 from API");
    }

    #[test]
    fn test_merge_empty() {
        let merged = merge_and_deduplicate_results(None, None);
        assert_eq!(merged.len(), 0);
    }
}
