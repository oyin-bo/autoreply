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

    // TODO: Implement actual feed fetching using atproto-client
    // For now, return a placeholder
    let markdown = format_feed_markdown(&[], feed_args.cursor.as_deref());
    
    Ok(ToolResult::text(markdown))
}

/// Format feed results as markdown
fn format_feed_markdown(posts: &[FeedPost], cursor: Option<&str>) -> String {
    let mut markdown = String::from("# BlueSky Feed\n\n");
    
    if posts.is_empty() {
        markdown.push_str("No posts available.\n");
    } else {
        for (i, post) in posts.iter().enumerate() {
            markdown.push_str(&format!("## Post {}\n", i + 1));
            markdown.push_str(&format!("**Author:** @{}\n", post.author));
            markdown.push_str(&format!("**Posted:** {}\n", post.created_at));
            markdown.push_str(&format!("\n{}\n", post.text));
            
            if let Some(uri) = &post.uri {
                markdown.push_str(&format!("\n**URI:** {}\n", uri));
            }
            
            // Add separator between posts
            if i < posts.len() - 1 {
                markdown.push_str("\n---\n\n");
            }
        }
    }
    
    if let Some(cursor_val) = cursor {
        markdown.push_str(&format!("\n**Cursor for pagination:** {}\n", cursor_val));
    }
    
    markdown
}

/// Temporary struct for feed posts (will be replaced with actual API response)
#[allow(dead_code)]
struct FeedPost {
    author: String,
    text: String,
    created_at: String,
    uri: Option<String>,
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
    fn test_format_feed_markdown() {
        let posts = vec![
            FeedPost {
                author: "alice.bsky.social".to_string(),
                text: "Hello world!".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                uri: Some("at://test/app.bsky.feed.post/1".to_string()),
            },
        ];

        let markdown = format_feed_markdown(&posts, Some("test-cursor"));
        
        assert!(markdown.contains("# BlueSky Feed"));
        assert!(markdown.contains("@alice.bsky.social"));
        assert!(markdown.contains("Hello world!"));
        assert!(markdown.contains("test-cursor"));
    }
}
