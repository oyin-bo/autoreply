//! Feed tool implementation
//!
//! Implements the `feed` MCP tool for fetching BlueSky feeds

use crate::cli::FeedArgs;
use crate::error::AppError;
use crate::http::client_with_timeout;
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PostAuthor {
    did: String,
    handle: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PostRecord {
    text: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct FeedPost {
    uri: String,
    cid: String,
    author: PostAuthor,
    record: PostRecord,
    #[serde(rename = "indexedAt")]
    indexed_at: String,
    #[serde(rename = "likeCount")]
    like_count: Option<i32>,
    #[serde(rename = "replyCount")]
    reply_count: Option<i32>,
    #[serde(rename = "repostCount")]
    repost_count: Option<i32>,
    #[serde(rename = "quoteCount")]
    quote_count: Option<i32>,
}

#[derive(Deserialize)]
struct FeedViewPost {
    post: FeedPost,
}

#[derive(Deserialize)]
struct FeedResponse {
    feed: Vec<FeedViewPost>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct FeedGeneratorView {
    uri: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct PopularFeedsResponse {
    feeds: Vec<FeedGeneratorView>,
}

/// Resolve a feed name/query to a full at:// URI by searching
async fn resolve_feed_uri(client: &reqwest::Client, query: &str) -> Result<String, AppError> {
    let search_url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.unspecced.getPopularFeedGenerators?query={}",
        urlencoding::encode(query)
    );

    debug!("Searching for feed with query: {}", query);

    let response = client
        .get(&search_url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to search for feed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Feed search API returned error {}",
            response.status()
        )));
    }

    let search_response: PopularFeedsResponse = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse feed search response: {}", e)))?;

    if search_response.feeds.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "No feeds found matching '{}'. Please provide a valid feed URI (at://...) or search term.",
            query
        )));
    }

    let first_feed = &search_response.feeds[0];
    debug!(
        "Found feed: {} ({})",
        first_feed.display_name.as_deref().unwrap_or("unnamed"),
        first_feed.uri
    );

    Ok(first_feed.uri.clone())
}

/// Handle feed tool call
pub async fn handle_feed(id: Option<Value>, args: Value) -> McpResponse {
    match timeout(Duration::from_secs(120), handle_feed_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Feed request exceeded 120 second timeout"),
    }
}

async fn handle_feed_impl(args: Value) -> Result<ToolResult, AppError> {
    let feed_args: FeedArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    execute_feed(feed_args).await
}

/// Execute feed tool
pub async fn execute_feed(feed_args: FeedArgs) -> Result<ToolResult, AppError> {
    debug!("Feed request for feed: {:?}", feed_args.feed);

    let client = client_with_timeout(Duration::from_secs(120));
    
    // Resolve the feed URI
    let feed_uri = match &feed_args.feed {
        Some(feed_input) => {
            // Check if it's already a valid at:// URI
            if feed_input.starts_with("at://") && feed_input.contains("/app.bsky.feed.generator/") {
                feed_input.clone()
            } else {
                // Not a full URI - search for feed by name
                debug!("Feed '{}' is not a full URI, searching...", feed_input);
                resolve_feed_uri(&client, feed_input).await?
            }
        }
        None => {
            // Default to "What's Hot" feed
            "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot".to_string()
        }
    };

    debug!("Using feed URI: {}", feed_uri);

    // Build the URL for the getFeed endpoint
    let mut url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.feed.getFeed?feed={}",
        urlencoding::encode(&feed_uri)
    );

    if let Some(cursor) = &feed_args.cursor {
        url.push_str(&format!("&cursor={}", urlencoding::encode(cursor)));
    }

    if let Some(limit) = feed_args.limit {
        let clamped_limit = limit.clamp(1, 100);
        url.push_str(&format!("&limit={}", clamped_limit));
    }

    debug!("Fetching feed from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to fetch feed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!(
            "Feed API returned error {}: {}",
            status, error_text
        )));
    }

    let feed_response: FeedResponse = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse feed response: {}", e)))?;

    debug!("Received {} posts from feed", feed_response.feed.len());

    // Format as markdown per docs/16-mcp-schemas.md spec
    let mut markdown = String::new();
    markdown.push_str(&format!("# Feed · {} posts\n\n", feed_response.feed.len()));

    use crate::tools::post_format::*;
    use std::collections::HashMap;
    let mut seen_posts: HashMap<String, String> = HashMap::new();

    for feed_post in &feed_response.feed {
        let post = &feed_post.post;
        let rkey = extract_rkey(&post.uri);
        let full_id = format!("{}/{}", post.author.handle, rkey);
        
        // Author ID line
        let author_id = compact_post_id(&post.author.handle, rkey, &seen_posts);
        markdown.push_str(&format!("{}\n", author_id));
        seen_posts.insert(full_id, post.uri.clone());
        
        // Blockquote content
        markdown.push_str(&blockquote_content(&post.record.text));
        markdown.push('\n');
        
        // Stats and timestamp
        let stats = format_stats(
            post.like_count.unwrap_or(0),
            post.repost_count.unwrap_or(0),
            post.quote_count.unwrap_or(0),
            post.reply_count.unwrap_or(0),
        );
        let timestamp = format_timestamp(&post.record.created_at);
        
        if !stats.is_empty() {
            markdown.push_str(&format!("{}  {}\n", stats, timestamp));
        } else {
            markdown.push_str(&format!("{}\n", timestamp));
        }
        
        markdown.push('\n');
    }

    if let Some(cursor) = feed_response.cursor {
        markdown.push_str(&format!("**Next cursor:** `{}`\n", cursor));
    }

    Ok(ToolResult::text(markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_args_deserialize() {
        let json = serde_json::json!({
            "feed": "at://did:plc:example/app.bsky.feed.generator/test",
            "limit": 50
        });

        let args: FeedArgs = serde_json::from_value(json).unwrap();
        assert_eq!(
            args.feed,
            Some("at://did:plc:example/app.bsky.feed.generator/test".to_string())
        );
        assert_eq!(args.limit, Some(50));
    }

    #[test]
    fn test_feed_args_optional_fields() {
        let json = serde_json::json!({});
        let args: FeedArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.feed, None);
        assert_eq!(args.limit, None);
        assert_eq!(args.cursor, None);
    }
}
