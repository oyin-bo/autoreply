//! Feed tool implementation
//!
//! Implements the `feed` MCP tool for fetching BlueSky feeds

use crate::cli::FeedArgs;
use crate::error::AppError;
use crate::http::client_with_timeout;
use crate::mcp::{McpResponse, ToolResult};
use crate::tools::util::at_uri_to_bsky_url;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info};

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
    info!("Feed request for feed: {:?}", feed_args.feed);

    let client = client_with_timeout(Duration::from_secs(30));
    
    // Determine the feed URI to use
    let feed_uri = feed_args.feed.as_deref().unwrap_or(
        "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot"
    );

    // Build the URL for the getFeed endpoint
    let mut url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.feed.getFeed?feed={}",
        urlencoding::encode(feed_uri)
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

    // Format as markdown
    let mut markdown = String::new();
    markdown.push_str("# BlueSky Feed\n\n");
    markdown.push_str(&format!("Found {} posts.\n\n", feed_response.feed.len()));

    for (i, feed_post) in feed_response.feed.iter().enumerate() {
        let post = &feed_post.post;
        markdown.push_str(&format!("## Post {}\n\n", i + 1));
        markdown.push_str(&format!("**@{}", post.author.handle));
        if let Some(display_name) = &post.author.display_name {
            markdown.push_str(&format!(" ({})", display_name));
        }
        markdown.push_str("\n\n");
        
        // Convert at:// URI to web URL
        let web_url = at_uri_to_bsky_url(&post.uri, &post.author.handle);
        markdown.push_str(&format!("**Link:** {}\n\n", web_url));
        
        markdown.push_str(&format!("{}\n\n", post.record.text));
        markdown.push_str(&format!("**Created:** {}\n\n", post.record.created_at));
        
        // Add engagement stats if available
        let stats: Vec<String> = vec![
            post.like_count.map(|c| format!("{} likes", c)),
            post.reply_count.map(|c| format!("{} replies", c)),
            post.repost_count.map(|c| format!("{} reposts", c)),
            post.quote_count.map(|c| format!("{} quotes", c)),
        ]
        .into_iter()
        .flatten()
        .collect();
        
        if !stats.is_empty() {
            markdown.push_str(&format!("**Stats:** {}\n\n", stats.join(", ")));
        }

        markdown.push_str("---\n\n");
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
