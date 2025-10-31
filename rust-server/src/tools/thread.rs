//! Thread tool implementation
//!
//! Implements the `thread` MCP tool for fetching BlueSky threads

use crate::cli::ThreadArgs;
use crate::error::AppError;
use crate::http::client_with_timeout;
use crate::mcp::{McpResponse, ToolResult};
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
struct ThreadPost {
    uri: String,
    cid: String,
    author: PostAuthor,
    record: PostRecord,
    #[serde(rename = "indexedAt")]
    indexed_at: Option<String>,
    #[serde(rename = "likeCount")]
    like_count: Option<i32>,
    #[serde(rename = "replyCount")]
    reply_count: Option<i32>,
    #[serde(rename = "repostCount")]
    repost_count: Option<i32>,
    #[serde(rename = "quoteCount")]
    quote_count: Option<i32>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "$type")]
enum ThreadNode {
    #[serde(rename = "app.bsky.feed.defs#threadViewPost")]
    ThreadViewPost {
        post: ThreadPost,
        #[serde(default)]
        replies: Vec<ThreadNode>,
    },
    #[serde(rename = "app.bsky.feed.defs#notFoundPost")]
    NotFoundPost {
        uri: String,
        #[serde(rename = "notFound")]
        not_found: bool,
    },
    #[serde(rename = "app.bsky.feed.defs#blockedPost")]
    BlockedPost {
        uri: String,
        blocked: bool,
    },
}

#[derive(Deserialize)]
struct ThreadResponse {
    thread: ThreadNode,
}

/// Handle thread tool call
pub async fn handle_thread(id: Option<Value>, args: Value) -> McpResponse {
    match timeout(Duration::from_secs(120), handle_thread_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Thread request exceeded 120 second timeout"),
    }
}

async fn handle_thread_impl(args: Value) -> Result<ToolResult, AppError> {
    let thread_args: ThreadArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    execute_thread(thread_args).await
}

/// Execute thread tool
pub async fn execute_thread(thread_args: ThreadArgs) -> Result<ToolResult, AppError> {
    info!("Thread request for post: {}", thread_args.post_uri);

    // Parse the post URI - it could be a URL or an at:// URI
    let post_uri = parse_post_uri(&thread_args.post_uri)?;

    let client = client_with_timeout(Duration::from_secs(30));

    // Build the URL for the getPostThread endpoint
    let url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread?uri={}",
        urlencoding::encode(&post_uri)
    );

    debug!("Fetching thread from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to fetch thread: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!(
            "Thread API returned error {}: {}",
            status, error_text
        )));
    }

    let thread_response: ThreadResponse = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse thread response: {}", e)))?;

    // Format thread as markdown
    let markdown = format_thread(&thread_response.thread);

    debug!("Thread formatted successfully");

    Ok(ToolResult::text(markdown))
}

/// Format a thread as markdown
fn format_thread(node: &ThreadNode) -> String {
    let mut markdown = String::new();
    markdown.push_str("# BlueSky Thread\n\n");
    
    // Flatten the thread first to get the count
    let posts = flatten_thread(node);
    markdown.push_str(&format!("Found {} posts in thread.\n\n", posts.len()));
    
    for (i, post) in posts.iter().enumerate() {
        format_thread_post(post, &mut markdown, i + 1);
    }
    
    markdown
}

/// Flatten thread into a list of posts
fn flatten_thread(node: &ThreadNode) -> Vec<&ThreadPost> {
    let mut posts = Vec::new();
    flatten_thread_recursive(node, &mut posts);
    posts
}

/// Recursively flatten thread nodes into a list
fn flatten_thread_recursive<'a>(node: &'a ThreadNode, posts: &mut Vec<&'a ThreadPost>) {
    if let ThreadNode::ThreadViewPost { post, replies } = node {
        posts.push(post);
        for reply in replies {
            flatten_thread_recursive(reply, posts);
        }
    }
}

/// Format a single post in the thread
fn format_thread_post(post: &ThreadPost, markdown: &mut String, post_num: usize) {
    markdown.push_str(&format!("## Post {}\n\n", post_num));
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

/// Convert AT URI to BlueSky web URL
/// at://did:plc:abc/app.bsky.feed.post/xyz -> https://bsky.app/profile/handle/post/xyz
fn at_uri_to_bsky_url(at_uri: &str, handle: &str) -> String {
    // Parse AT URI: at://{did}/{collection}/{rkey}
    if !at_uri.starts_with("at://") {
        return at_uri.to_string();
    }

    let parts: Vec<&str> = at_uri.trim_start_matches("at://").split('/').collect();
    if parts.len() < 3 {
        return at_uri.to_string();
    }

    // parts[0] = DID
    // parts[1] = collection (e.g., app.bsky.feed.post)
    // parts[2] = rkey
    let rkey = parts[2];

    format!("https://bsky.app/profile/{}/post/{}", handle, rkey)
}

/// Parse a post URI from either a BlueSky URL or an at:// URI
fn parse_post_uri(uri: &str) -> Result<String, AppError> {
    // If it's already an at:// URI, return it
    if uri.starts_with("at://") {
        return Ok(uri.to_string());
    }

    // Try to parse as a BlueSky URL
    // Format: https://bsky.app/profile/{handle}/post/{postId}
    if let Some(captures) = uri.strip_prefix("https://bsky.app/profile/") {
        if let Some((handle, rest)) = captures.split_once("/post/") {
            // We need to resolve the handle to a DID for the at:// URI
            // For now, we'll return an error asking for the at:// URI directly
            // In a full implementation, we'd resolve the handle to DID
            return Err(AppError::InvalidInput(format!(
                "Please provide the at:// URI directly. URL format is not yet supported. Handle: {}, Post ID: {}",
                handle, rest
            )));
        }
    }

    Err(AppError::InvalidInput(format!(
        "Invalid post URI: {}. Expected at:// URI or https://bsky.app/profile/handle/post/id URL",
        uri
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_args_deserialize() {
        let json = serde_json::json!({
            "postURI": "at://did:plc:example/app.bsky.feed.post/123"
        });

        let args: ThreadArgs = serde_json::from_value(json).unwrap();
        assert_eq!(
            args.post_uri,
            "at://did:plc:example/app.bsky.feed.post/123"
        );
    }

    #[test]
    fn test_parse_post_uri_at_protocol() {
        let uri = "at://did:plc:example/app.bsky.feed.post/123";
        let result = parse_post_uri(uri).unwrap();
        assert_eq!(result, uri);
    }

    #[test]
    fn test_parse_post_uri_url_not_supported() {
        let uri = "https://bsky.app/profile/alice.bsky.social/post/123";
        let result = parse_post_uri(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_post_uri_invalid() {
        let uri = "invalid://something";
        let result = parse_post_uri(uri);
        assert!(result.is_err());
    }
}
