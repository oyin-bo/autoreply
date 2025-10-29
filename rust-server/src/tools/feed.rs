//! Feed tool implementation
//!
//! Implements the `feed` MCP tool to retrieve BlueSky feeds

use crate::cli::FeedArgs;
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::info;

/// Handle feed tool call
pub async fn handle_feed(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds
    match timeout(Duration::from_secs(120), handle_feed_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, "error", &format!("Feed request failed: {}", e)),
        },
        Err(_) => McpResponse::error(id, "timeout", "Feed request exceeded 120 second timeout"),
    }
}

async fn handle_feed_impl(args: Value) -> Result<ToolResult> {
    // Parse arguments
    let feed_args: FeedArgs = serde_json::from_value(args)
        .map_err(|e| anyhow::anyhow!("Invalid arguments: {}", e))?;

    // Execute using shared implementation
    execute_feed(feed_args).await
}

/// Execute feed tool (shared implementation for MCP and CLI)
pub async fn execute_feed(feed_args: FeedArgs) -> Result<ToolResult> {
    info!("Feed request - feed: {:?}, login: {:?}", feed_args.feed, feed_args.login);

    // Create client (authenticated if credentials provided, otherwise public)
    let client = if let (Some(login), Some(password)) = (&feed_args.login, &feed_args.password) {
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

    // Fetch feed
    let feed_response = client
        .get_feed(
            feed_args.feed.as_deref(),
            feed_args.cursor.as_deref(),
            feed_args.limit,
        )
        .await?;

    // Format as markdown
    let markdown = format_feed_results(&feed_response);
    
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

/// Format feed response as markdown
fn format_feed_results(response: &crate::bluesky::client::FeedResponse) -> String {
    let mut markdown = String::from("# BlueSky Feed\n\n");
    
    if response.feed.is_empty() {
        markdown.push_str("No posts available.\n");
    } else {
        for (i, feed_post) in response.feed.iter().enumerate() {
            let post = &feed_post.post;
            
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
            if i < response.feed.len() - 1 {
                markdown.push_str("\n---\n\n");
            }
        }
    }
    
    // Add cursor info if present
    if let Some(cursor) = &response.cursor {
        markdown.push_str(&format!("\n**Pagination cursor:** `{}`\n", cursor));
        markdown.push_str("\n*Use this cursor with the feed tool to fetch more posts.*\n");
    }
    
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_feed_args_parsing() {
        let args = json!({
            "feed": "test-feed",
            "login": "test.bsky.social"
        });

        let parsed: FeedArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.feed, Some("test-feed".to_string()));
        assert_eq!(parsed.login, Some("test.bsky.social".to_string()));
    }

    #[test]
    fn test_format_feed_results() {
        use crate::bluesky::client::{Author, FeedResponse, FeedViewPost, Post};
        
        let response = FeedResponse {
            feed: vec![FeedViewPost {
                post: Post {
                    uri: "at://test/app.bsky.feed.post/1".to_string(),
                    cid: "cid1".to_string(),
                    author: Author {
                        did: "did:plc:test".to_string(),
                        handle: "alice.bsky.social".to_string(),
                        display_name: Some("Alice".to_string()),
                    },
                    record: json!({
                        "text": "Hello world!"
                    }),
                    indexed_at: "2024-01-01T00:00:00Z".to_string(),
                    like_count: Some(5),
                    reply_count: Some(2),
                    repost_count: Some(1),
                    quote_count: None,
                },
            }],
            cursor: Some("test-cursor".to_string()),
        };

        let markdown = format_feed_results(&response);
        
        assert!(markdown.contains("# BlueSky Feed"));
        assert!(markdown.contains("@alice.bsky.social"));
        assert!(markdown.contains("Alice"));
        assert!(markdown.contains("Hello world!"));
        assert!(markdown.contains("5 likes"));
        assert!(markdown.contains("2 replies"));
        assert!(markdown.contains("test-cursor"));
    }
}
