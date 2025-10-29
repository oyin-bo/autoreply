//! Thread tool implementation
//!
//! Implements the `thread` MCP tool to fetch BlueSky threads

use crate::cli::ThreadArgs;
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::info;

/// Handle thread tool call
pub async fn handle_thread(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds
    match timeout(Duration::from_secs(120), handle_thread_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, "error", &format!("Thread request failed: {}", e)),
        },
        Err(_) => McpResponse::error(id, "timeout", "Thread request exceeded 120 second timeout"),
    }
}

async fn handle_thread_impl(args: Value) -> Result<ToolResult> {
    // Parse arguments
    let thread_args: ThreadArgs = serde_json::from_value(args)
        .map_err(|e| anyhow::anyhow!("Invalid arguments: {}", e))?;

    // Execute using shared implementation
    execute_thread(thread_args).await
}

/// Execute thread tool (shared implementation for MCP and CLI)
pub async fn execute_thread(thread_args: ThreadArgs) -> Result<ToolResult> {
    info!("Thread request for post_uri: {}", thread_args.post_uri);

    // Validate post_uri
    if thread_args.post_uri.is_empty() {
        return Err(anyhow::anyhow!("post_uri is required and cannot be empty"));
    }

    // Parse and normalize the post URI
    let post_uri = parse_post_uri(&thread_args.post_uri)?;

    // Create client (authenticated if credentials provided, otherwise public)
    let client = if let (Some(login), Some(password)) = (&thread_args.login, &thread_args.password) {
        // Try to authenticate
        match authenticate_user(login, password).await {
            Ok(token) => crate::bluesky::client::BskyClient::with_auth(token),
            Err(_) => {
                // Fall back to unauthenticated if auth fails
                crate::bluesky::client::BskyClient::new()
            }
        }
    } else {
        crate::bluesky::client::BskyClient::new()
    };

    // Fetch thread
    let thread_response = client.get_post_thread(&post_uri).await?;

    // Format as markdown
    let markdown = format_thread_results(&thread_response, &post_uri);
    
    Ok(ToolResult::text(markdown))
}

/// Authenticate user and return access token
async fn authenticate_user(login: &str, password: &str) -> Result<String> {
    use crate::auth::{Credentials, SessionManager, DEFAULT_SERVICE};
    
    let credentials = Credentials {
        identifier: login.to_string(),
        password: password.to_string(),
        service: DEFAULT_SERVICE.to_string(),
    };
    
    let session_manager = SessionManager::new()?;
    let session = session_manager.login(&credentials).await?;
    
    Ok(session.access_jwt)
}

/// Parse post URI from various formats (at:// URI or bsky.app URL)
fn parse_post_uri(uri: &str) -> Result<String> {
    // If it's already an at:// URI, return it
    if uri.starts_with("at://") {
        return Ok(uri.to_string());
    }
    
    // Try to parse bsky.app URL format: https://bsky.app/profile/{handle}/post/{postid}
    if uri.contains("bsky.app/profile/") {
        let parts: Vec<&str> = uri.split('/').collect();
        if parts.len() >= 6 {
            let _handle = parts[parts.len() - 3];
            let _post_id = parts[parts.len() - 1];
            
            // For simplicity, we need to resolve the handle to DID
            // For now, just return an error asking for at:// URI format
            return Err(anyhow::anyhow!(
                "Please provide the post URI in at:// format. URL parsing is not yet supported."
            ));
        }
    }
    
    Ok(uri.to_string())
}

/// Flatten thread into a list of posts
fn flatten_thread(thread_view: &crate::bluesky::client::ThreadView) -> Vec<&crate::bluesky::client::Post> {
    let mut posts = Vec::new();
    
    match thread_view {
        crate::bluesky::client::ThreadView::Post(thread_post) => {
            // Add parent posts first (if any)
            if let Some(parent) = &thread_post.parent {
                posts.extend(flatten_thread(parent));
            }
            
            // Add current post
            posts.push(&thread_post.post);
            
            // Add replies
            if let Some(replies) = &thread_post.replies {
                for reply in replies {
                    posts.extend(flatten_thread(reply));
                }
            }
        }
        _ => {
            // Handle NotFound or Blocked - just skip
        }
    }
    
    posts
}

/// Format thread response as markdown
fn format_thread_results(response: &crate::bluesky::client::ThreadResponse, requested_uri: &str) -> String {
    let mut markdown = format!("# Thread for {}\n\n", requested_uri);
    
    let posts = flatten_thread(&response.thread);
    
    if posts.is_empty() {
        markdown.push_str("Thread not found or empty.\n");
    } else {
        for (i, post) in posts.iter().enumerate() {
            markdown.push_str(&format!("## Post {}\n\n", i + 1));
            
            // Author info
            markdown.push_str(&format!("**Author:** @{}", post.author.handle));
            if let Some(display_name) = &post.author.display_name {
                markdown.push_str(&format!(" ({})", display_name));
            }
            markdown.push_str("\n\n");
            
            // Post text
            if let Some(text) = post.record.get("text").and_then(|v| v.as_str()) {
                markdown.push_str(&format!("{}\n\n", text));
            }
            
            // Post metadata
            markdown.push_str(&format!("**Posted:** {}\n", post.indexed_at));
            markdown.push_str(&format!("**URI:** {}\n", post.uri));
            
            // Check if this is a reply
            if let Some(reply) = post.record.get("reply") {
                if let Some(parent_uri) = reply.get("parent").and_then(|p| p.get("uri")).and_then(|u| u.as_str()) {
                    markdown.push_str(&format!("**Reply to:** {}\n", parent_uri));
                }
            }
            
            // Engagement stats
            let mut stats = vec![];
            if let Some(likes) = post.like_count {
                if likes > 0 {
                    stats.push(format!("{} likes", likes));
                }
            }
            if let Some(replies) = post.reply_count {
                if replies > 0 {
                    stats.push(format!("{} replies", replies));
                }
            }
            if let Some(reposts) = post.repost_count {
                if reposts > 0 {
                    stats.push(format!("{} reposts", reposts));
                }
            }
            
            if !stats.is_empty() {
                markdown.push_str(&format!("\n**Engagement:** {}\n", stats.join(", ")));
            }
            
            // Add separator between posts
            if i < posts.len() - 1 {
                markdown.push_str("\n---\n\n");
            }
        }
    }
    
    markdown.push_str(&format!("\n**Total posts in thread:** {}\n", posts.len()));
    
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_thread_args_parsing() {
        let args = json!({
            "post_uri": "at://did:plc:test/app.bsky.feed.post/abc123",
            "login": "test.bsky.social"
        });

        let parsed: ThreadArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.post_uri, "at://did:plc:test/app.bsky.feed.post/abc123");
        assert_eq!(parsed.login, Some("test.bsky.social".to_string()));
    }

    #[test]
    fn test_parse_post_uri() {
        // Test at:// URI (should pass through)
        let result = parse_post_uri("at://did:plc:test/app.bsky.feed.post/abc123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "at://did:plc:test/app.bsky.feed.post/abc123");
    }

    #[test]
    fn test_format_thread_results() {
        use crate::bluesky::client::{Author, Post, ThreadResponse, ThreadView, ThreadViewPost};
        
        let response = ThreadResponse {
            thread: ThreadView::Post(ThreadViewPost {
                post: Post {
                    uri: "at://test/app.bsky.feed.post/1".to_string(),
                    cid: "cid1".to_string(),
                    author: Author {
                        did: "did:plc:test".to_string(),
                        handle: "alice.bsky.social".to_string(),
                        display_name: Some("Alice".to_string()),
                    },
                    record: json!({
                        "text": "Original post"
                    }),
                    indexed_at: "2024-01-01T00:00:00Z".to_string(),
                    like_count: Some(3),
                    reply_count: Some(1),
                    repost_count: None,
                    quote_count: None,
                },
                parent: None,
                replies: Some(vec![ThreadView::Post(ThreadViewPost {
                    post: Post {
                        uri: "at://test/app.bsky.feed.post/2".to_string(),
                        cid: "cid2".to_string(),
                        author: Author {
                            did: "did:plc:test2".to_string(),
                            handle: "bob.bsky.social".to_string(),
                            display_name: Some("Bob".to_string()),
                        },
                        record: json!({
                            "text": "Reply to the post",
                            "reply": {
                                "parent": {
                                    "uri": "at://test/app.bsky.feed.post/1"
                                }
                            }
                        }),
                        indexed_at: "2024-01-01T01:00:00Z".to_string(),
                        like_count: None,
                        reply_count: None,
                        repost_count: None,
                        quote_count: None,
                    },
                    parent: None,
                    replies: None,
                })]),
            }),
        };

        let markdown = format_thread_results(&response, "at://test/app.bsky.feed.post/1");
        
        println!("Generated markdown:\n{}", markdown);
        
        assert!(markdown.contains("# Thread for"));
        assert!(markdown.contains("@alice.bsky.social"));
        assert!(markdown.contains("@bob.bsky.social"));
        assert!(markdown.contains("Original post"));
        assert!(markdown.contains("Reply to the post"));
        assert!(markdown.contains("Total posts in thread:"), "Should contain 'Total posts in thread:' but got:\n{}", markdown);
    }

    #[tokio::test]
    async fn test_thread_empty_uri_validation() {
        let args = json!({
            "post_uri": ""
        });

        let parsed: ThreadArgs = serde_json::from_value(args).unwrap();
        let result = execute_thread(parsed).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("post_uri is required"));
    }
}
