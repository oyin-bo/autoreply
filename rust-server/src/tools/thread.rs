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
use std::collections::HashMap;
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

    let client = client_with_timeout(Duration::from_secs(30));

    // Parse the post URI - it could be a URL or an at:// URI
    let post_uri = parse_post_uri(&client, &thread_args.post_uri).await?;

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

/// Format a thread as markdown per docs/16-mcp-schemas.md spec
fn format_thread(node: &ThreadNode) -> String {
    use crate::tools::post_format::*;
    use std::collections::HashMap;
    
    let mut markdown = String::new();
    
    // Count total posts
    let total_posts = count_posts(node);
    markdown.push_str(&format!("# Thread · {} posts\n\n", total_posts));
    
    // Track seen posts for ID compaction
    let mut seen_posts: HashMap<String, String> = HashMap::new();
    
    // Format thread recursively with proper threading indicators
    format_thread_recursive(node, &mut markdown, &mut seen_posts, 0, None);
    
    markdown
}

/// Count total posts in thread
fn count_posts(node: &ThreadNode) -> usize {
    match node {
        ThreadNode::ThreadViewPost { post: _, replies } => {
            1 + replies.iter().map(|r| count_posts(r)).sum::<usize>()
        }
        _ => 0,
    }
}

/// Recursively format thread with proper indentation and threading indicators
fn format_thread_recursive(
    node: &ThreadNode,
    markdown: &mut String,
    seen_posts: &mut HashMap<String, String>,
    depth: usize,
    parent_post: Option<&ThreadPost>,
) {
    use crate::tools::post_format::*;
    
    if let ThreadNode::ThreadViewPost { post, replies } = node {
        let rkey = extract_rkey(&post.uri);
        let full_id = format!("{}/{}", post.author.handle, rkey);
        
        // Build the first line with threading indicator (INDENTED)
        let author_id = compact_post_id(&post.author.handle, rkey, seen_posts);
        
        if depth == 0 {
            // Root post - just the author ID, no indent
            markdown.push_str(&format!("{}\n", author_id));
        } else if let Some(parent) = parent_post {
            // Reply - show threading indicator with indentation
            let parent_rkey = extract_rkey(&parent.uri);
            let parent_compact = ultra_compact_id(&parent.author.handle, parent_rkey);
            let indicator = threading_indicator(depth, &parent_compact, &author_id);
            markdown.push_str(&format!("{}\n", indicator));
        }
        
        // Mark this post as seen for future compaction
        seen_posts.insert(full_id, post.uri.clone());
        
        // Blockquote the content (ALWAYS FLUSH-LEFT, NO INDENTATION)
        markdown.push_str(&blockquote_content(&post.record.text));
        markdown.push('\n');
        
        // Stats and timestamp on same line (FLUSH-LEFT)
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
        
        // Blank line before next post
        markdown.push('\n');
        
        // Process replies recursively
        for reply in replies {
            format_thread_recursive(reply, markdown, seen_posts, depth + 1, Some(post));
        }
    }
}

/// Parse a post URI from either a BlueSky URL or an at:// URI
async fn parse_post_uri(client: &reqwest::Client, uri: &str) -> Result<String, AppError> {
    let trimmed = uri.trim();
    
    // If it's already an at:// URI, return it
    if trimmed.starts_with("at://") {
        return Ok(trimmed.to_string());
    }

    // Try compact format @handle/rkey
    if trimmed.starts_with('@') && trimmed.contains('/') {
        let without_at = &trimmed[1..]; // Remove leading @
        let parts: Vec<&str> = without_at.split('/').collect();
        
        if parts.len() >= 2 {
            let handle = parts[0];
            let rkey = parts[1];
            
            // Resolve handle to DID
            let did = resolve_handle(client, handle).await?;
            
            // Construct at:// URI
            let at_uri = format!("at://{}/app.bsky.feed.post/{}", did, rkey);
            debug!("Resolved compact format to at:// URI: {}", at_uri);
            
            return Ok(at_uri);
        }
    }

    // Try to parse as a BlueSky URL
    // Format: https://bsky.app/profile/{handle}/post/{postId}
    if let Some(captures) = trimmed.strip_prefix("https://bsky.app/profile/") {
        if let Some((handle, post_id)) = captures.split_once("/post/") {
            // Extract just the post ID (remove trailing slashes or query params)
            let post_id = post_id.split('/').next().unwrap_or(post_id);
            let post_id = post_id.split('?').next().unwrap_or(post_id);
            
            debug!("Parsing URL: handle={}, post_id={}", handle, post_id);
            
            // Check if handle is already a DID
            let did = if handle.starts_with("did:") {
                handle.to_string()
            } else {
                // Resolve handle to DID
                resolve_handle(client, handle).await?
            };
            
            // Construct at:// URI
            let at_uri = format!("at://{}/app.bsky.feed.post/{}", did, post_id);
            debug!("Resolved URL to at:// URI: {}", at_uri);
            
            return Ok(at_uri);
        }
    }

    Err(AppError::InvalidInput(format!(
        "Invalid post URI: {}. Expected at:// URI, https://bsky.app/profile/handle/post/id URL, or @handle/rkey",
        uri
    )))
}

/// Resolve a handle to a DID
async fn resolve_handle(client: &reqwest::Client, handle: &str) -> Result<String, AppError> {
    let url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
        urlencoding::encode(handle.trim_start_matches('@'))
    );

    debug!("Resolving handle: {}", handle);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to resolve handle: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::InvalidInput(format!(
            "Failed to resolve handle '{}': {}",
            handle,
            response.status()
        )));
    }

    #[derive(Deserialize)]
    struct ResolveHandleResponse {
        did: String,
    }

    let resolve_response: ResolveHandleResponse = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse handle resolution response: {}", e)))?;

    debug!("Resolved handle {} to DID {}", handle, resolve_response.did);

    Ok(resolve_response.did)
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

    #[tokio::test]
    async fn test_parse_post_uri_at_protocol() {
        let client = client_with_timeout(Duration::from_secs(5));
        let uri = "at://did:plc:example/app.bsky.feed.post/123";
        let result = parse_post_uri(&client, uri).await.unwrap();
        assert_eq!(result, uri);
    }

    #[tokio::test]
    async fn test_parse_post_uri_url_with_did() {
        let client = client_with_timeout(Duration::from_secs(5));
        let uri = "https://bsky.app/profile/did:plc:example/post/123";
        let result = parse_post_uri(&client, uri).await.unwrap();
        assert_eq!(result, "at://did:plc:example/app.bsky.feed.post/123");
    }

    #[tokio::test]
    async fn test_parse_post_uri_invalid() {
        let client = client_with_timeout(Duration::from_secs(5));
        let uri = "invalid://something";
        let result = parse_post_uri(&client, uri).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_post_uri_compact_format_with_did() {
        let client = client_with_timeout(Duration::from_secs(5));
        // Note: This would normally resolve the handle, but we can't test that without a real server
        // For now, just verify it rejects invalid formats
        let uri = "@/xyz123"; // Invalid: no handle
        let result = parse_post_uri(&client, uri).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_count_posts() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post("alice", "3kq8a3f1", "Root post"),
            replies: vec![
                ThreadNode::ThreadViewPost {
                    post: create_mock_post("bob", "3kq8b2e4", "Reply 1"),
                    replies: vec![
                        ThreadNode::ThreadViewPost {
                            post: create_mock_post("carol", "3kq8c3f5", "Nested reply"),
                            replies: vec![],
                        },
                    ],
                },
                ThreadNode::ThreadViewPost {
                    post: create_mock_post("dave", "3kq8d4f6", "Reply 2"),
                    replies: vec![],
                },
            ],
        };

        assert_eq!(count_posts(&thread), 4);
    }

    #[test]
    fn test_format_thread_single_post() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post("utopia-defer.red", "3m4jnj3efp22t", "Test post content"),
            replies: vec![],
        };

        let markdown = format_thread(&thread);

        assert!(markdown.contains("# Thread · 1 posts"));
        assert!(markdown.contains("@utopia-defer.red/3m4jnj3efp22t"));
        assert!(markdown.contains("> Test post content"));
        assert!(markdown.contains("👍 33  ♻️ 1  💬 1"));
        assert!(markdown.contains("2024-10-06T10:15:33Z"));
        assert!(!markdown.contains("## Post 1"));
        assert!(!markdown.contains("**Link:**"));
        assert!(!markdown.contains("**Created:**"));
    }

    #[test]
    fn test_format_thread_with_replies() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post("alice.bsky.social", "3kq8a3f1", "Root post"),
            replies: vec![
                ThreadNode::ThreadViewPost {
                    post: create_mock_post("bob.bsky.social", "3kq8b2e4", "First reply"),
                    replies: vec![],
                },
                ThreadNode::ThreadViewPost {
                    post: create_mock_post("carol.bsky.social", "3kq8c3f5", "Second reply"),
                    replies: vec![],
                },
            ],
        };

        let markdown = format_thread(&thread);

        assert!(markdown.contains("# Thread · 3 posts"));
        
        // Root post - full ID
        assert!(markdown.contains("@alice.bsky.social/3kq8a3f1\n> Root post"));
        
        // First reply - shows parent in ultra-compact format
        assert!(markdown.contains("└─@a/…a3f1 → @bob.bsky.social/3kq8b2e4\n> First reply"));
        
        // Second reply - also shows parent in ultra-compact
        assert!(markdown.contains("└─@a/…a3f1 → @carol.bsky.social/3kq8c3f5\n> Second reply"));
        
        // All content blockquoted
        assert!(markdown.matches("> ").count() >= 3);
    }

    #[test]
    fn test_format_thread_nested_replies() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post("alice", "3kq8a3f1", "Root"),
            replies: vec![
                ThreadNode::ThreadViewPost {
                    post: create_mock_post("bob", "3kq8b2e4", "Reply depth 1"),
                    replies: vec![
                        ThreadNode::ThreadViewPost {
                            post: create_mock_post("carol", "3kq8c3f5", "Reply depth 2"),
                            replies: vec![],
                        },
                    ],
                },
            ],
        };

        let markdown = format_thread(&thread);

        // Check indentation levels - ONLY the threading indicator is indented, NOT the content
        assert!(markdown.contains("@alice/3kq8a3f1")); // Root, no indent
        assert!(markdown.contains("└─@a/…a3f1 → @bob/3kq8b2e4")); // Depth 1, no spaces before └─
        assert!(markdown.contains("  └─@b/…b2e4 → @carol/3kq8c3f5")); // Depth 2, 2 spaces before └─
        
        // Content should always be flush-left (no indentation)
        assert!(markdown.contains("\n> Root\n"));
        assert!(markdown.contains("\n> Reply depth 1\n"));
        assert!(markdown.contains("\n> Reply depth 2\n"));
    }

    #[test]
    fn test_format_thread_multiline_content() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post_multiline(
                "alice",
                "3kq8a3f1",
                "Line 1\nLine 2\nLine 3"
            ),
            replies: vec![],
        };

        let markdown = format_thread(&thread);

        // Each line should be block-quoted
        assert!(markdown.contains("> Line 1\n> Line 2\n> Line 3"));
    }

    #[test]
    fn test_format_thread_with_markdown_in_content() {
        let thread = ThreadNode::ThreadViewPost {
            post: create_mock_post(
                "alice",
                "3kq8a3f1",
                "# This looks like a header\n## But it's quoted!"
            ),
            replies: vec![],
        };

        let markdown = format_thread(&thread);

        // Markdown syntax should be inside blockquotes
        assert!(markdown.contains("> # This looks like a header\n> ## But it's quoted!"));
    }

    // Helper to create mock posts
    fn create_mock_post(handle: &str, rkey: &str, text: &str) -> ThreadPost {
        ThreadPost {
            uri: format!("at://did:plc:test{}/app.bsky.feed.post/{}", handle, rkey),
            cid: format!("cid{}", rkey),
            author: PostAuthor {
                did: format!("did:plc:test{}", handle),
                handle: handle.to_string(),
                display_name: Some(format!("Display {}", handle)),
            },
            record: PostRecord {
                text: text.to_string(),
                created_at: "2024-10-06T10:15:33.123Z".to_string(),
            },
            indexed_at: Some("2024-10-06T10:15:34Z".to_string()),
            like_count: Some(33),
            reply_count: Some(1),
            repost_count: Some(0),
            quote_count: Some(1),
        }
    }

    fn create_mock_post_multiline(handle: &str, rkey: &str, text: &str) -> ThreadPost {
        create_mock_post(handle, rkey, text)
    }
}
