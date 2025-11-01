//! React tool implementation
//!
//! Implements the `react(reactAs, like, unlike, repost, delete)` MCP tool
//! Supports batching multiple operations in a single call

use crate::auth::storage::CredentialStorage;
use crate::auth::SessionManager;
use crate::cli::ReactArgs;
use crate::error::AppError;
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::debug;

/// Handle react tool call
pub async fn handle_react(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds
    match timeout(Duration::from_secs(120), handle_react_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "React request exceeded 120 second timeout"),
    }
}

async fn handle_react_impl(args: Value) -> Result<ToolResult, AppError> {
    // Parse arguments
    let react_args: ReactArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    // Execute using shared implementation
    execute_react(react_args).await
}

/// Execute react tool (shared implementation for MCP and CLI)
pub async fn execute_react(react_args: ReactArgs) -> Result<ToolResult, AppError> {
    debug!(
        "React request for account: {}, like: {}, unlike: {}, repost: {}, delete: {}",
        react_args.reactAs,
        react_args.like.len(),
        react_args.unlike.len(),
        react_args.repost.len(),
        react_args.delete.len()
    );

    // Get credentials for the account
    let storage = CredentialStorage::new()?;

    // Try to get stored session first (for OAuth accounts)
    let session = if let Some(stored_session) = storage.get_session(&react_args.reactAs)? {
        debug!("Using stored session for {}", react_args.reactAs);
        stored_session
    } else {
        // Fallback to creating new session with credentials (for app password accounts)
        debug!(
            "No valid session for {}, using app password flow",
            react_args.reactAs
        );
        let credentials = storage.get_credentials(&react_args.reactAs)?;
        let session_manager = SessionManager::new()?;
        session_manager.login(&credentials).await?
    };

    debug!("Authenticated as {} (DID: {})", session.handle, session.did);

    let client = crate::http::client_with_timeout(std::time::Duration::from_secs(120));

    // Track results
    let mut results = Vec::new();
    let mut errors = Vec::new();

    // Process likes
    for post_uri in &react_args.like {
        match process_like(&client, &session, post_uri).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("Like failed for {}: {}", post_uri, e)),
        }
    }

    // Process unlikes
    for post_uri in &react_args.unlike {
        match process_unlike(&client, &session, post_uri).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("Unlike failed for {}: {}", post_uri, e)),
        }
    }

    // Process reposts
    for post_uri in &react_args.repost {
        match process_repost(&client, &session, post_uri).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("Repost failed for {}: {}", post_uri, e)),
        }
    }

    // Process deletes
    for post_uri in &react_args.delete {
        match process_delete(&client, &session, post_uri).await {
            Ok(msg) => results.push(msg),
            Err(e) => errors.push(format!("Delete failed for {}: {}", post_uri, e)),
        }
    }

    // Format results as markdown
    let mut markdown = String::from("# React Operations Results\n\n");

    if !results.is_empty() {
        markdown.push_str("## Successful Operations\n\n");
        for (i, result) in results.iter().enumerate() {
            markdown.push_str(&format!("{}. {}\n", i + 1, result));
        }
        markdown.push('\n');
    }

    if !errors.is_empty() {
        markdown.push_str("## Failed Operations\n\n");
        for (i, error) in errors.iter().enumerate() {
            markdown.push_str(&format!("{}. {}\n", i + 1, error));
        }
        markdown.push('\n');
    }

    markdown.push_str(&format!(
        "**Summary:** {} successful, {} failed\n",
        results.len(),
        errors.len()
    ));

    debug!(
        "React operations completed: {} successful, {} failed",
        results.len(),
        errors.len()
    );

    Ok(ToolResult::text(markdown))
}

/// Process a like operation
async fn process_like(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    post_uri: &str,
) -> Result<String, AppError> {
    let (did, _rkey, uri, cid) = fetch_post_info(client, session, post_uri).await?;

    let url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);

    let body = serde_json::json!({
        "repo": session.did,
        "collection": "app.bsky.feed.like",
        "record": {
            "$type": "app.bsky.feed.like",
            "subject": {
                "uri": uri,
                "cid": cid
            },
            "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Like request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Like failed with status {}: {}",
            status, error_text
        )));
    }

    Ok(format!("Liked post: {} ({})", post_uri, did))
}

/// Process an unlike operation
async fn process_unlike(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    post_uri: &str,
) -> Result<String, AppError> {
    // First, we need to find the like record for this post
    let (did, _rkey, uri, _cid) = fetch_post_info(client, session, post_uri).await?;

    // List likes to find the one for this post
    // Note: This lists up to 100 likes. For users with more likes, this is a known limitation.
    // A future improvement would be to implement pagination if the like is not found in the first page.
    let list_url = format!(
        "{}/xrpc/com.atproto.repo.listRecords?repo={}&collection=app.bsky.feed.like&limit=100",
        session.service, session.did
    );

    let response = client
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to list likes: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to list likes: {}",
            status
        )));
    }

    let list_result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse likes list: {}", e)))?;

    // Find the like record for this post
    let like_rkey = list_result["records"]
        .as_array()
        .and_then(|records| {
            records.iter().find_map(|record| {
                let subject_uri = record["value"]["subject"]["uri"].as_str()?;
                if subject_uri == uri {
                    record["uri"].as_str().and_then(|u| {
                        // Extract rkey from at://{did}/app.bsky.feed.like/{rkey}
                        u.split('/').next_back().map(|s| s.to_string())
                    })
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| AppError::NotFound(format!("No like found for post: {}", post_uri)))?;

    // Delete the like record
    let delete_url = format!("{}/xrpc/com.atproto.repo.deleteRecord", session.service);

    let delete_body = serde_json::json!({
        "repo": session.did,
        "collection": "app.bsky.feed.like",
        "rkey": like_rkey
    });

    let response = client
        .post(&delete_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .json(&delete_body)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Unlike request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Unlike failed with status {}: {}",
            status, error_text
        )));
    }

    Ok(format!("Unliked post: {} ({})", post_uri, did))
}

/// Process a repost operation
async fn process_repost(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    post_uri: &str,
) -> Result<String, AppError> {
    let (did, _rkey, uri, cid) = fetch_post_info(client, session, post_uri).await?;

    let url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);

    let body = serde_json::json!({
        "repo": session.did,
        "collection": "app.bsky.feed.repost",
        "record": {
            "$type": "app.bsky.feed.repost",
            "subject": {
                "uri": uri,
                "cid": cid
            },
            "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Repost request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Repost failed with status {}: {}",
            status, error_text
        )));
    }

    Ok(format!("Reposted post: {} ({})", post_uri, did))
}

/// Process a delete operation
async fn process_delete(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    post_uri: &str,
) -> Result<String, AppError> {
    let (did, rkey, _uri, _cid) = fetch_post_info(client, session, post_uri).await?;

    // Verify the post belongs to the authenticated user
    if did != session.did {
        return Err(AppError::InvalidInput(format!(
            "Cannot delete post that doesn't belong to you: {}",
            post_uri
        )));
    }

    let url = format!("{}/xrpc/com.atproto.repo.deleteRecord", session.service);

    let body = serde_json::json!({
        "repo": session.did,
        "collection": "app.bsky.feed.post",
        "rkey": rkey
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Delete request failed: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Delete failed with status {}: {}",
            status, error_text
        )));
    }

    Ok(format!("Deleted post: {}", post_uri))
}

/// Fetch post information (DID, rkey, URI, CID) from a URI/URL
async fn fetch_post_info(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    post_uri: &str,
) -> Result<(String, String, String, String), AppError> {
    // Parse the URI using the shared utility
    let post_ref = crate::bluesky::uri::parse_post_uri(post_uri).await?;

    // Fetch the post to get its CID
    let url = format!(
        "{}/xrpc/com.atproto.repo.getRecord?repo={}&collection=app.bsky.feed.post&rkey={}",
        session.service, post_ref.did, post_ref.rkey
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| AppError::NetworkError(format!("Failed to fetch post: {}", e)))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::NetworkError(format!(
            "Failed to fetch post with status {}: {}",
            status, error_text
        )));
    }

    let post_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::ParseError(format!("Failed to parse post data: {}", e)))?;

    let uri = post_data["uri"]
        .as_str()
        .ok_or_else(|| AppError::ParseError("No URI in post data".to_string()))?
        .to_string();
    let cid = post_data["cid"]
        .as_str()
        .ok_or_else(|| AppError::ParseError("No CID in post data".to_string()))?
        .to_string();

    Ok((post_ref.did, post_ref.rkey, uri, cid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_react_args_parsing() {
        let args = json!({
            "reactAs": "test.bsky.social",
            "like": ["at://did:plc:abc/app.bsky.feed.post/123"],
            "unlike": [],
            "repost": [],
            "delete": []
        });

        let parsed: ReactArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.reactAs, "test.bsky.social");
        assert_eq!(parsed.like.len(), 1);
        assert_eq!(parsed.unlike.len(), 0);
        assert_eq!(parsed.repost.len(), 0);
        assert_eq!(parsed.delete.len(), 0);
    }

    #[tokio::test]
    async fn test_react_args_mixed_operations() {
        let args = json!({
            "reactAs": "test.bsky.social",
            "like": ["at://did:plc:abc/app.bsky.feed.post/1"],
            "unlike": ["at://did:plc:abc/app.bsky.feed.post/2"],
            "repost": ["at://did:plc:def/app.bsky.feed.post/3"],
            "delete": ["at://did:plc:ghi/app.bsky.feed.post/4"]
        });

        let parsed: ReactArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.reactAs, "test.bsky.social");
        assert_eq!(parsed.like.len(), 1);
        assert_eq!(parsed.unlike.len(), 1);
        assert_eq!(parsed.repost.len(), 1);
        assert_eq!(parsed.delete.len(), 1);
    }
}
