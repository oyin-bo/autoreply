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
    markdown.push_str("# Thread\n\n");
    
    format_thread_recursive(node, &mut markdown, 0);
    
    markdown
}

/// Recursively format thread nodes
fn format_thread_recursive(node: &ThreadNode, markdown: &mut String, depth: usize) {
    match node {
        ThreadNode::ThreadViewPost { post, replies } => {
            // Indent based on depth
            let indent = "  ".repeat(depth);
            
            // For root post, use "Post 1"; for replies, just "Post"
            let post_header = if depth == 0 {
                format!("{}## Post 1\n", indent)
            } else {
                format!("{}## Post\n", indent)
            };
            markdown.push_str(&post_header);
            
            // Extract rkey from URI (at://did/app.bsky.feed.post/rkey)
            let rkey = post.uri.split('/').next_back().unwrap_or("");
            let post_url = format!(
                "https://bsky.app/profile/{}/post/{}",
                post.author.handle, rkey
            );
            markdown.push_str(&format!("{}**Link:** {}\n", indent, post_url));
            markdown.push_str(&format!("{}**Created:** {}\n\n", indent, post.record.created_at));
            
            // Format post text with proper indentation
            for line in post.record.text.lines() {
                markdown.push_str(&format!("{}{}\n", indent, line));
            }
            markdown.push('\n');
            
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
                markdown.push_str(&format!("{}*{}*\n", indent, stats.join(", ")));
            }
            markdown.push('\n');

            // Process replies recursively
            if !replies.is_empty() {
                markdown.push_str(&format!("{}### Replies:\n\n", indent));
                for reply in replies {
                    format_thread_recursive(reply, markdown, depth + 1);
                }
            }
        }
        ThreadNode::NotFoundPost { uri, .. } => {
            markdown.push_str(&format!("*Post not found: {}*\n\n", uri));
        }
        ThreadNode::BlockedPost { uri, .. } => {
            markdown.push_str(&format!("*Post blocked: {}*\n\n", uri));
        }
    }
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
