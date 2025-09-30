//! Profile tool implementation
//!
//! Implements the `profile(account)` MCP tool

use crate::bluesky::car::CarProcessor;
use crate::bluesky::did::DidResolver;
use crate::cli::ProfileArgs;
use crate::error::{validate_account, AppError};
use crate::mcp::{McpResponse, ToolResult};
use anyhow::Result;
use serde_json::Value;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

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
        profile_args.account.strip_prefix('@').unwrap_or(&profile_args.account).to_string()
    };

    debug!("Resolved {} to DID: {}", profile_args.account, did);

    // Fetch and process repository
    let car_processor = CarProcessor::new()?;
    let car_data = car_processor.fetch_repo(&did).await?;
    
    debug!("Fetched CAR data: {} bytes", car_data.len());

    // Extract profile record
    let profile_record = car_processor.extract_profile(&car_data).await?;
    
    let profile = match profile_record {
        Some(profile) => profile,
        None => {
            return Err(AppError::NotFound(format!(
                "No profile found for account: {}",
                profile_args.account
            )));
        }
    };

    debug!("Found profile record");

    // Convert to markdown
    let markdown = profile.to_markdown(&display_handle, &did);
    
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