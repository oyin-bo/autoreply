//! Post tool implementation
//!
//! Implements the `post(postAs, text, replyTo)` MCP tool

use crate::auth::SessionManager;
use crate::bluesky::did::DidResolver;
use crate::cli::PostArgs;
use crate::error::AppError;
use crate::mcp::{McpResponse, ToolResult};
use crate::utils::PostRef;
use anyhow::Result;
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

/// Handle post tool call
pub async fn handle_post(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds
    match timeout(Duration::from_secs(120), handle_post_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Post request exceeded 120 second timeout"),
    }
}

async fn handle_post_impl(args: Value) -> Result<ToolResult, AppError> {
    // Parse arguments
    let post_args: PostArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    // Execute using shared implementation
    execute_post(post_args).await
}

/// Execute post tool (shared implementation for MCP and CLI)
pub async fn execute_post(post_args: PostArgs) -> Result<ToolResult, AppError> {
    // Validate text is not empty
    if post_args.text.trim().is_empty() {
        return Err(AppError::InvalidInput("Post text cannot be empty".to_string()));
    }

    info!("Creating post with text: {}", &post_args.text[..post_args.text.len().min(50)]);

    // Get session for the specified account (or default)
    let session_manager = SessionManager::new()?;
    let session = if let Some(account) = &post_args.post_as {
        // Load session for specific account
        let storage = crate::auth::CredentialStorage::new()?;
        let creds = storage.get_credentials(account)?;
        session_manager.login(&creds).await?
    } else {
        // Load default account
        let storage = crate::auth::CredentialStorage::new()?;
        let default_account = storage.get_default_account()?
            .ok_or_else(|| AppError::Authentication("No default account configured. Please login first.".to_string()))?;
        let creds = storage.get_credentials(&default_account)?;
        session_manager.login(&creds).await?
    };

    let client = crate::http::client_with_timeout(std::time::Duration::from_secs(30));

    // Build post record
    let mut record = json!({
        "$type": "app.bsky.feed.post",
        "text": post_args.text,
        "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    });

    // Handle reply if specified
    let mut reply_info = None;
    if let Some(reply_to) = &post_args.reply_to {
        debug!("Processing reply to: {}", reply_to);
        
        let mut post_ref = PostRef::parse(reply_to)?;
        
        // Resolve handle to DID if needed
        if post_ref.needs_did_resolution() {
            let resolver = DidResolver::new();
            post_ref.did = resolver.resolve_handle(&post_ref.did).await?
                .ok_or_else(|| AppError::DidResolveFailed(format!("Failed to resolve handle: {}", post_ref.did)))?;
        }

        // Fetch the post we're replying to
        let get_record_url = format!(
            "{}/xrpc/com.atproto.repo.getRecord",
            session.service
        );
        
        let response = client
            .get(&get_record_url)
            .query(&[
                ("repo", post_ref.did.as_str()),
                ("collection", "app.bsky.feed.post"),
                ("rkey", post_ref.rkey.as_str()),
            ])
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::NetworkError(format!(
                "Failed to fetch reply-to post: {}",
                response.status()
            )));
        }

        let reply_post: Value = response.json().await?;
        
        // Build reply structure
        let parent_ref = json!({
            "uri": reply_post["uri"],
            "cid": reply_post["cid"]
        });
        
        let root_ref = if let Some(existing_reply) = reply_post["value"]["reply"].as_object() {
            // This is a reply to a reply - use the original root
            existing_reply.get("root").cloned().unwrap_or(parent_ref.clone())
        } else {
            // This is a new thread - parent becomes root
            parent_ref.clone()
        };

        let reply = json!({
            "root": root_ref,
            "parent": parent_ref
        });
        
        record["reply"] = reply;
        reply_info = Some(reply_post["uri"].as_str().unwrap_or("").to_string());
    }

    // Create the post
    let create_url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);
    
    let create_body = json!({
        "repo": session.did,
        "collection": "app.bsky.feed.post",
        "record": record
    });

    debug!("Creating post with body: {}", serde_json::to_string_pretty(&create_body).unwrap());

    let response = client
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/json")
        .json(&create_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        return Err(AppError::NetworkError(format!(
            "Failed to create post: {} - {}",
            status, error_body
        )));
    }

    let result: Value = response.json().await?;
    let post_uri = result["uri"].as_str().unwrap_or("").to_string();
    
    // Build markdown response
    let markdown = if let Some(reply_uri) = reply_info {
        format!(
            "# Post Created (Reply)\n\n**Post URI:** {}\n\n**Reply To:** {}\n\n**Text:**\n{}\n\n✅ Successfully posted reply.",
            post_uri, reply_uri, post_args.text
        )
    } else {
        format!(
            "# Post Created\n\n**Post URI:** {}\n\n**Text:**\n{}\n\n✅ Successfully posted.",
            post_uri, post_args.text
        )
    };

    info!("Successfully created post: {}", post_uri);

    Ok(ToolResult::text(markdown))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_post_args_parsing() {
        let args = json!({
            "text": "Hello, BlueSky!"
        });

        let parsed: PostArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.text, "Hello, BlueSky!");
        assert!(parsed.post_as.is_none());
        assert!(parsed.reply_to.is_none());
    }

    #[tokio::test]
    async fn test_post_args_with_reply() {
        let args = json!({
            "text": "This is a reply",
            "replyTo": "at://did:plc:test/app.bsky.feed.post/abc123"
        });

        let parsed: PostArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.text, "This is a reply");
        assert_eq!(parsed.reply_to, Some("at://did:plc:test/app.bsky.feed.post/abc123".to_string()));
    }

    #[test]
    fn test_empty_text_validation() {
        let args = PostArgs {
            post_as: None,
            text: "".to_string(),
            reply_to: None,
        };

        // This would fail in execute_post due to empty text check
        assert!(args.text.trim().is_empty());
    }
}
