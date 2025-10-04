//! Profile tool implementation
//!
//! Implements the `profile(account)` MCP tool

use crate::bluesky::did::DidResolver;
use crate::bluesky::provider::RepositoryProvider;
use crate::cli::ProfileArgs;
use crate::error::{validate_account, AppError};
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

/// Helper function to extract string field from CBOR map without allocation
fn get_cbor_string_field(
    map: &std::collections::BTreeMap<serde_cbor::Value, serde_cbor::Value>,
    key: &str,
) -> Option<String> {
    for (k, v) in map.iter() {
        if let serde_cbor::Value::Text(text_key) = k {
            if text_key == key {
                if let serde_cbor::Value::Text(text_value) = v {
                    return Some(text_value.clone());
                }
            }
        }
    }
    None
}

/// Handle profile tool call
pub async fn handle_profile(id: Option<Value>, args: Value) -> McpResponse {
    // Set total timeout to 120 seconds as specified
    match timeout(Duration::from_secs(120), handle_profile_impl(args)).await {
        Ok(result) => match result {
            Ok(content) => McpResponse::success(id, serde_json::to_value(content).unwrap()),
            Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
        },
        Err(_) => McpResponse::error(id, "timeout", "Profile request exceeded 120 second timeout"),
    }
}

async fn handle_profile_impl(args: Value) -> Result<ToolResult, AppError> {
    // Parse arguments
    let profile_args: ProfileArgs = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    // Execute using shared implementation
    execute_profile(profile_args).await
}

/// Execute profile tool (shared implementation for MCP and CLI)
pub async fn execute_profile(profile_args: ProfileArgs) -> Result<ToolResult, AppError> {
    // Validate account parameter
    validate_account(&profile_args.account)?;

    info!("Profile request for account: {}", profile_args.account);

    // Resolve handle to DID
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(&profile_args.account).await?;

    // Determine the handle for display
    let display_handle = if profile_args.account.starts_with("did:plc:") {
        // If input was a DID, we might not have the handle - use DID for now
        profile_args.account.clone()
    } else {
        profile_args
            .account
            .strip_prefix('@')
            .unwrap_or(&profile_args.account)
            .to_string()
    };

    debug!("Resolved {} to DID: {:?}", profile_args.account, did);

    // Use true streaming to process CAR blocks one by one (like Go version)
    let provider = RepositoryProvider::new()?;

    debug!("Starting streaming CAR block processing for {:?}", did);
    use crate::bluesky::records::ProfileRecord;

    // Use the new iterator-based streaming approach
    let mut records = provider
        .records(
            did.as_ref()
                .ok_or_else(|| AppError::DidResolveFailed("DID resolution failed".to_string()))?,
        )
        .await?;
    let profile = records.find_map(|record_result| {
        let (record_type, cbor_data) = record_result.ok()?;
        debug!("Processing record of type: {}", record_type);

        // Check if this is a profile record
        if record_type == "app.bsky.actor.profile" {
            debug!("Found profile record!");

            // Decode CBOR data to ProfileRecord
            if let Ok(serde_cbor::Value::Map(profile_map)) =
                serde_cbor::from_slice::<serde_cbor::Value>(&cbor_data)
            {
                // Use helper function to avoid string allocations
                let display_name = get_cbor_string_field(&profile_map, "displayName");
                let description = get_cbor_string_field(&profile_map, "description");
                let avatar = get_cbor_string_field(&profile_map, "avatar");
                let banner = get_cbor_string_field(&profile_map, "banner");
                let created_at = get_cbor_string_field(&profile_map, "createdAt")
                    .unwrap_or_else(|| "unknown".to_string());

                return Some(ProfileRecord {
                    display_name,
                    description,
                    avatar,
                    banner,
                    created_at,
                });
            }
        }
        None
    });

    let profile = match profile {
        Some(profile_data) => profile_data,
        None => {
            return Err(AppError::NotFound(format!(
                "No profile found for account: {}",
                profile_args.account
            )));
        }
    };

    debug!("Found profile record");

    // Convert to markdown
    let markdown = profile.to_markdown(
        &display_handle,
        did.as_ref()
            .ok_or_else(|| AppError::DidResolveFailed("DID resolution failed".to_string()))?,
    );

    info!("Profile request completed for: {}", profile_args.account);

    Ok(ToolResult::text(markdown))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_profile_args_parsing() {
        let args = json!({
            "account": "test.bsky.social"
        });

        let parsed: ProfileArgs = serde_json::from_value(args).unwrap();
        assert_eq!(parsed.account, "test.bsky.social");
    }

    #[test]
    fn test_invalid_account_validation() {
        let result = validate_account("");
        assert!(result.is_err());

        let result = validate_account("invalid");
        assert!(result.is_err());

        let result = validate_account("test.bsky.social");
        assert!(result.is_ok());

        let result = validate_account("did:plc:abc123xyz789012345678901");
        assert!(result.is_ok());
    }
}
