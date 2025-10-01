use crate::auth::CredentialManager;
use crate::error::AppError;
use crate::mcp::{ContentItem, McpResponse, ToolResult};
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::SystemTime;

/// Arguments for the auth_status tool
#[derive(JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct AuthStatusArgs {
    /// Optional: Check status for specific account handle
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional: Check status for specific account handle")]
    pub handle: Option<String>,
}

/// Handle auth_status tool call from MCP
pub async fn handle_auth_status(id: Option<Value>, arguments: Value) -> McpResponse {
    match serde_json::from_value::<AuthStatusArgs>(arguments) {
        Ok(args) => {
            match execute_auth_status(args).await {
                Ok(result) => McpResponse::success(id, serde_json::to_value(result).unwrap()),
                Err(err) => McpResponse::error(id, err.error_code(), &err.message()),
            }
        }
        Err(e) => McpResponse::error(
            id,
            "invalid_params",
            &format!("Invalid arguments: {}", e),
        ),
    }
}

/// Execute the auth_status tool
pub async fn execute_auth_status(args: AuthStatusArgs) -> Result<ToolResult, AppError> {
    let cm = CredentialManager::new()
        .map_err(|e| AppError::Internal(format!("Failed to create credential manager: {}", e)))?;
    
    let mut accounts = cm.list_accounts()
        .map_err(|e| AppError::Internal(format!("Failed to list accounts: {}", e)))?;
    
    let default_account = cm.get_default_account()
        .map_err(|e| AppError::Internal(format!("Failed to get default account: {}", e)))?;
    
    // If specific handle requested, filter to that account
    if let Some(handle) = &args.handle {
        accounts.retain(|acc| &acc.handle == handle);
    }
    
    // Format as markdown for text content
    let mut text = String::from("# Authentication Status\n\n");
    
    if accounts.is_empty() {
        text.push_str("No authenticated accounts found.\n");
        text.push_str("\nRun `autoreply login` to authenticate.\n");
    } else {
        text.push_str(&format!("**Authenticated Accounts:** {}\n\n", accounts.len()));
        
        for acc in &accounts {
            let marker = if default_account.as_ref() == Some(&acc.handle) {
                "✓"
            } else {
                " "
            };
            
            text.push_str(&format!("{} **@{}**\n", marker, acc.handle));
            
            if !acc.did.is_empty() {
                text.push_str(&format!("  - DID: `{}`\n", acc.did));
            }
            if !acc.pds.is_empty() {
                text.push_str(&format!("  - PDS: `{}`\n", acc.pds));
            }
            
            // Format timestamps
            if let Ok(duration) = acc.created_at.duration_since(SystemTime::UNIX_EPOCH) {
                let secs = duration.as_secs();
                text.push_str(&format!("  - Created: {}\n", format_timestamp(secs)));
            }
            if let Ok(duration) = acc.last_used.duration_since(SystemTime::UNIX_EPOCH) {
                let secs = duration.as_secs();
                text.push_str(&format!("  - Last used: {}\n", format_timestamp(secs)));
            }
            
            if default_account.as_ref() == Some(&acc.handle) {
                text.push_str("  - _(default)_\n");
            }
            text.push('\n');
        }
        
        if let Some(default) = &default_account {
            text.push_str(&format!("**Default Account:** @{}\n", default));
        }
    }
    
    Ok(ToolResult {
        content: vec![ContentItem {
            r#type: "text".to_string(),
            text,
        }],
    })
}

/// Format a Unix timestamp as a readable string
fn format_timestamp(secs: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    // Simple formatting - in a real app you'd use chrono
    format!("{:?}", dt).replace("SystemTime { tv_sec: ", "").replace(", tv_nsec: 0 }", "")
}
