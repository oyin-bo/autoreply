//! React tool implementation
//!
//! Implements the `react(reactAs, like, unlike, repost, delete)` MCP tool with batch operations

use crate::auth::SessionManager;
use crate::bluesky::did::DidResolver;
use crate::cli::ReactArgs;
use crate::error::AppError;
use crate::mcp::{McpResponse, ToolResult};
use crate::utils::PostRef;
use anyhow::Result;
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

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
    info!("Processing react operations");

    // Get session for the specified account (or default)
    let session_manager = SessionManager::new()?;
    let session = if let Some(account) = &react_args.react_as {
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
    let resolver = DidResolver::new();

    let mut results = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    // Process likes
    if let Some(like_uris) = &react_args.like {
        for uri in like_uris {
            match process_like(&client, &session, &resolver, uri).await {
                Ok(msg) => {
                    results.push(format!("✅ {}", msg));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ Like failed for {}: {}", uri, e));
                    error_count += 1;
                    warn!("Like failed for {}: {}", uri, e);
                }
            }
        }
    }

    // Process unlikes
    if let Some(unlike_uris) = &react_args.unlike {
        for uri in unlike_uris {
            match process_unlike(&client, &session, &resolver, uri).await {
                Ok(msg) => {
                    results.push(format!("✅ {}", msg));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ Unlike failed for {}: {}", uri, e));
                    error_count += 1;
                    warn!("Unlike failed for {}: {}", uri, e);
                }
            }
        }
    }

    // Process reposts
    if let Some(repost_uris) = &react_args.repost {
        for uri in repost_uris {
            match process_repost(&client, &session, &resolver, uri).await {
                Ok(msg) => {
                    results.push(format!("✅ {}", msg));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ Repost failed for {}: {}", uri, e));
                    error_count += 1;
                    warn!("Repost failed for {}: {}", uri, e);
                }
            }
        }
    }

    // Process deletes
    if let Some(delete_uris) = &react_args.delete {
        for uri in delete_uris {
            match process_delete(&client, &session, uri).await {
                Ok(msg) => {
                    results.push(format!("✅ {}", msg));
                    success_count += 1;
                }
                Err(e) => {
                    results.push(format!("❌ Delete failed for {}: {}", uri, e));
                    error_count += 1;
                    warn!("Delete failed for {}: {}", uri, e);
                }
            }
        }
    }

    if results.is_empty() {
        return Err(AppError::InvalidInput(
            "No operations specified. Provide at least one of: like, unlike, repost, delete".to_string()
        ));
    }

    // Build markdown response
    let mut markdown = format!("# React Operations Complete\n\n");
    markdown.push_str(&format!("**Total:** {} operations\n", success_count + error_count));
    markdown.push_str(&format!("**Successful:** {}\n", success_count));
    markdown.push_str(&format!("**Failed:** {}\n\n", error_count));
    
    markdown.push_str("## Results\n\n");
    for result in results {
        markdown.push_str(&format!("- {}\n", result));
    }

    info!("React operations completed: {} success, {} errors", success_count, error_count);

    Ok(ToolResult::text(markdown))
}

/// Process a like operation
async fn process_like(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    resolver: &DidResolver,
    uri: &str,
) -> Result<String, AppError> {
    let mut post_ref = PostRef::parse(uri)?;
    
    // Resolve handle to DID if needed
    if post_ref.needs_did_resolution() {
        post_ref.did = resolver.resolve_handle(&post_ref.did).await?
            .ok_or_else(|| AppError::DidResolveFailed(format!("Failed to resolve handle: {}", post_ref.did)))?;
    }

    // Get the post to obtain its CID
    let get_record_url = format!("{}/xrpc/com.atproto.repo.getRecord", session.service);
    
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
            "Failed to fetch post: {}",
            response.status()
        )));
    }

    let post: Value = response.json().await?;
    let post_uri = post["uri"].as_str().unwrap_or(uri);
    let post_cid = post["cid"].as_str().ok_or_else(|| {
        AppError::ParseError("Post CID not found".to_string())
    })?;

    // Create like record
    let create_url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);
    
    let create_body = json!({
        "repo": session.did,
        "collection": "app.bsky.feed.like",
        "record": {
            "$type": "app.bsky.feed.like",
            "subject": {
                "uri": post_uri,
                "cid": post_cid
            },
            "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        }
    });

    let response = client
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/json")
        .json(&create_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to create like: {}",
            response.status()
        )));
    }

    Ok(format!("Liked post: {}", post_uri))
}

/// Process an unlike operation
async fn process_unlike(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    resolver: &DidResolver,
    uri: &str,
) -> Result<String, AppError> {
    let mut post_ref = PostRef::parse(uri)?;
    
    // Resolve handle to DID if needed
    if post_ref.needs_did_resolution() {
        post_ref.did = resolver.resolve_handle(&post_ref.did).await?
            .ok_or_else(|| AppError::DidResolveFailed(format!("Failed to resolve handle: {}", post_ref.did)))?;
    }

    let post_uri = crate::utils::make_at_uri(&post_ref.did, "app.bsky.feed.post", &post_ref.rkey);

    // List likes to find the like record
    let list_url = format!("{}/xrpc/com.atproto.repo.listRecords", session.service);
    
    let response = client
        .get(&list_url)
        .query(&[
            ("repo", session.did.as_str()),
            ("collection", "app.bsky.feed.like"),
            ("limit", "100"),
        ])
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to list likes: {}",
            response.status()
        )));
    }

    let likes: Value = response.json().await?;
    
    // Find the like record for this post
    let like_rkey = likes["records"]
        .as_array()
        .and_then(|records| {
            records.iter().find_map(|record| {
                if record["value"]["subject"]["uri"].as_str() == Some(&post_uri) {
                    record["uri"].as_str().and_then(|uri| {
                        uri.split('/').last().map(|s| s.to_string())
                    })
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| AppError::NotFound(format!("Like not found for post: {}", post_uri)))?;

    // Delete the like record
    let delete_url = format!("{}/xrpc/com.atproto.repo.deleteRecord", session.service);
    
    let delete_body = json!({
        "repo": session.did,
        "collection": "app.bsky.feed.like",
        "rkey": like_rkey
    });

    let response = client
        .post(&delete_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/json")
        .json(&delete_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to delete like: {}",
            response.status()
        )));
    }

    Ok(format!("Unliked post: {}", post_uri))
}

/// Process a repost operation
async fn process_repost(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    resolver: &DidResolver,
    uri: &str,
) -> Result<String, AppError> {
    let mut post_ref = PostRef::parse(uri)?;
    
    // Resolve handle to DID if needed
    if post_ref.needs_did_resolution() {
        post_ref.did = resolver.resolve_handle(&post_ref.did).await?
            .ok_or_else(|| AppError::DidResolveFailed(format!("Failed to resolve handle: {}", post_ref.did)))?;
    }

    // Get the post to obtain its CID
    let get_record_url = format!("{}/xrpc/com.atproto.repo.getRecord", session.service);
    
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
            "Failed to fetch post: {}",
            response.status()
        )));
    }

    let post: Value = response.json().await?;
    let post_uri = post["uri"].as_str().unwrap_or(uri);
    let post_cid = post["cid"].as_str().ok_or_else(|| {
        AppError::ParseError("Post CID not found".to_string())
    })?;

    // Create repost record
    let create_url = format!("{}/xrpc/com.atproto.repo.createRecord", session.service);
    
    let create_body = json!({
        "repo": session.did,
        "collection": "app.bsky.feed.repost",
        "record": {
            "$type": "app.bsky.feed.repost",
            "subject": {
                "uri": post_uri,
                "cid": post_cid
            },
            "createdAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        }
    });

    let response = client
        .post(&create_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/json")
        .json(&create_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to create repost: {}",
            response.status()
        )));
    }

    Ok(format!("Reposted: {}", post_uri))
}

/// Process a delete operation
async fn process_delete(
    client: &reqwest::Client,
    session: &crate::auth::Session,
    uri: &str,
) -> Result<String, AppError> {
    let post_ref = PostRef::parse(uri)?;
    
    // For delete, the post must belong to the authenticated user
    // We don't need to resolve handles since we're deleting from our own repo
    
    // Delete the post record
    let delete_url = format!("{}/xrpc/com.atproto.repo.deleteRecord", session.service);
    
    let delete_body = json!({
        "repo": session.did,
        "collection": "app.bsky.feed.post",
        "rkey": post_ref.rkey
    });

    let response = client
        .post(&delete_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/json")
        .json(&delete_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to delete post: {}",
            response.status()
        )));
    }

    Ok(format!("Deleted post: {}", uri))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_react_args_parsing() {
        let args = json!({
            "like": ["at://did:plc:test/app.bsky.feed.post/abc123"]
        });

        let parsed: ReactArgs = serde_json::from_value(args).unwrap();
        assert!(parsed.like.is_some());
        assert_eq!(parsed.like.as_ref().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_react_args_batch() {
        let args = json!({
            "like": ["at://did:plc:test1/app.bsky.feed.post/abc"],
            "repost": ["at://did:plc:test2/app.bsky.feed.post/def"],
            "delete": ["at://did:plc:test3/app.bsky.feed.post/ghi"]
        });

        let parsed: ReactArgs = serde_json::from_value(args).unwrap();
        assert!(parsed.like.is_some());
        assert!(parsed.repost.is_some());
        assert!(parsed.delete.is_some());
    }
}
