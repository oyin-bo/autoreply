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

    // TODO: Implement actual thread fetching using atproto-client
    // For now, return a placeholder
    let markdown = format_thread_markdown(&[], &thread_args.post_uri);
    
    Ok(ToolResult::text(markdown))
}

/// Format thread results as markdown
fn format_thread_markdown(posts: &[ThreadPost], post_uri: &str) -> String {
    let mut markdown = format!("# Thread for {}\n\n", post_uri);
    
    if posts.is_empty() {
        markdown.push_str("Thread not found or empty.\n");
    } else {
        for (i, post) in posts.iter().enumerate() {
            markdown.push_str(&format!("## Reply {}\n", i + 1));
            markdown.push_str(&format!("**Author:** @{}\n", post.author));
            markdown.push_str(&format!("**Posted:** {}\n", post.created_at));
            markdown.push_str(&format!("\n{}\n", post.text));
            
            if let Some(uri) = &post.uri {
                markdown.push_str(&format!("\n**URI:** {}\n", uri));
            }
            
            if let Some(reply_to) = &post.reply_to_uri {
                markdown.push_str(&format!("**Reply to:** {}\n", reply_to));
            }
            
            // Add separator between posts
            if i < posts.len() - 1 {
                markdown.push_str("\n---\n\n");
            }
        }
    }
    
    markdown.push_str(&format!("\n**Thread with {} posts**\n", posts.len()));
    
    markdown
}

/// Temporary struct for thread posts (will be replaced with actual API response)
#[allow(dead_code)]
struct ThreadPost {
    author: String,
    text: String,
    created_at: String,
    uri: Option<String>,
    reply_to_uri: Option<String>,
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
    fn test_format_thread_markdown() {
        let posts = vec![
            ThreadPost {
                author: "alice.bsky.social".to_string(),
                text: "Original post".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                uri: Some("at://test/app.bsky.feed.post/1".to_string()),
                reply_to_uri: None,
            },
            ThreadPost {
                author: "bob.bsky.social".to_string(),
                text: "Reply to the post".to_string(),
                created_at: "2024-01-01T01:00:00Z".to_string(),
                uri: Some("at://test/app.bsky.feed.post/2".to_string()),
                reply_to_uri: Some("at://test/app.bsky.feed.post/1".to_string()),
            },
        ];

        let markdown = format_thread_markdown(&posts, "at://test/app.bsky.feed.post/1");
        
        assert!(markdown.contains("# Thread for"));
        assert!(markdown.contains("@alice.bsky.social"));
        assert!(markdown.contains("@bob.bsky.social"));
        assert!(markdown.contains("Original post"));
        assert!(markdown.contains("Reply to the post"));
        assert!(markdown.contains("Thread with 2 posts"));
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
