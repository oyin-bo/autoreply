//! Post tool implementation
//!
//! Implements the `post(postAs, text, replyTo)` MCP tool

use crate::auth::storage::CredentialStorage;
use crate::auth::SessionManager;
use crate::cli::PostArgs;
use crate::error::AppError;
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::debug;

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
    debug!(
        "Post request for account: {}, text: '{}'",
        post_args.postAs, post_args.text
    );

    // Get credentials for the account
    let storage = CredentialStorage::new()?;

    // Try to get stored session first (for OAuth accounts)
    let session = if let Some(stored_session) = storage.get_session(&post_args.postAs)? {
        debug!("Using stored session for {}", post_args.postAs);
        stored_session
    } else {
        // Fallback to creating new session with credentials (for app password accounts)
        debug!(
            "No stored session, creating new session for {}",
            post_args.postAs
        );
        let credentials = storage.get_credentials(&post_args.postAs)?;
        let session_manager = SessionManager::new()?;
        session_manager.login(&credentials).await?
    };

    debug!("Authenticated as {} (DID: {})", session.handle, session.did);

    // Parse reply-to if provided
    let reply_ref = if let Some(reply_to) = &post_args.replyTo {
        Some(parse_and_fetch_reply(&session, reply_to).await?)
    } else {
        None
    };

    // Create the post
    let client = crate::http::client_with_timeout(std::time::Duration::from_secs(120));
    let url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);

    let mut record = serde_json::json!({
        "$type": "app.bsky.feed.post",
        "text": post_args.text,
        "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    });

    // Add reply information if present
    if let Some(reply) = reply_ref {
        record["reply"] = reply;
    }

    let body = serde_json::json!({
        "repo": session.did,
        "collection": "app.bsky.feed.post",
        "record": record,
    });

    debug!("Creating post with body: {}", serde_json::to_string(&body)?);

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Post creation request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Post creation failed with status {}: {}",
            status, error_text
        )));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse response: {}", e)))?;

    let post_uri = result["uri"]
        .as_str()
        .ok_or_else(|| AppError::ParseError("No URI in response".to_string()))?;

    debug!("Post created successfully: {}", post_uri);

    // Format result as markdown
    let markdown = if post_args.replyTo.is_some() {
        format!(
            "# Reply Posted\n\n**Post URI:** {}\n\n**Text:** {}\n\n**Reply To:** {}\n",
            post_uri,
            post_args.text,
            post_args.replyTo.as_ref().unwrap()
        )
    } else {
        format!(
            "# Post Created\n\n**Post URI:** {}\n\n**Text:** {}\n",
            post_uri, post_args.text
        )
    };

    Ok(ToolResult::text(markdown))
}

/// Parse a post URI/URL and fetch the post details to create a reply reference
async fn parse_and_fetch_reply(
    session: &crate::auth::Session,
    reply_to: &str,
) -> Result<serde_json::Value, AppError> {
    // Parse the URI using the shared utility
    let post_ref = crate::bluesky::uri::parse_post_uri(reply_to).await?;

    // Fetch the post to get its CID
    let client = crate::http::client_with_timeout(std::time::Duration::from_secs(120));
    let url = format!(
        "{}/xrpc/com.atproto.repo.getRecord?repo={}&collection=app.bsky.feed.post&rkey={}",
        session.service, post_ref.did, post_ref.rkey
    );

    debug!("Fetching reply-to post from: {}", url);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to fetch reply-to post: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Failed to fetch reply-to post with status {}: {}",
            status, error_text
        )));
    }

    let post_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse post data: {}", e)))?;

    let post_uri = post_data["uri"]
        .as_str()
        .ok_or_else(|| AppError::ParseError("No URI in post data".to_string()))?;
    let post_cid = post_data["cid"]
        .as_str()
        .ok_or_else(|| AppError::ParseError("No CID in post data".to_string()))?;

    // Check if the post itself is a reply to build the proper reply chain
    let root_ref = if let Some(reply) = post_data["value"].get("reply") {
        if let Some(root) = reply.get("root") {
            root.clone()
        } else {
            serde_json::json!({
                "uri": post_uri,
                "cid": post_cid
            })
        }
    } else {
        serde_json::json!({
            "uri": post_uri,
            "cid": post_cid
        })
    };

    Ok(serde_json::json!({
        "root": root_ref,
        "parent": {
            "uri": post_uri,
            "cid": post_cid
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_post_args_parsing() {
        let args = json!({
            "postAs": "test.bsky.social",
            "text": "Hello, world!"
        });

        let parsed: PostArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.postAs, "test.bsky.social");
        assert_eq!(parsed.text, "Hello, world!");
        assert!(parsed.replyTo.is_none());
    }

    #[tokio::test]
    async fn test_post_args_with_reply() {
        let args = json!({
            "postAs": "test.bsky.social",
            "text": "Reply text",
            "replyTo": "at://did:plc:abc/app.bsky.feed.post/123"
        });

        let parsed: PostArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.postAs, "test.bsky.social");
        assert_eq!(parsed.text, "Reply text");
        assert_eq!(
            parsed.replyTo,
            Some("at://did:plc:abc/app.bsky.feed.post/123".to_string())
        );
    }
}
