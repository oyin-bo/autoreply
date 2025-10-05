use crate::auth::{LoginManager, LoginRequest};
use crate::cli::LoginCommand;
use crate::error::AppError;
use crate::mcp::{ContentItem, McpResponse, ServerContext, ToolResult};
use serde_json::{json, Value};

pub async fn handle_login(id: Option<Value>, args: Value, context: &ServerContext) -> McpResponse {
    match handle_login_impl(args, context).await {
        Ok(result) => McpResponse::success(id, serde_json::to_value(result).unwrap()),
        Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
    }
}

async fn handle_login_impl(args: Value, context: &ServerContext) -> Result<ToolResult, AppError> {
    let command: LoginCommand = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    let manager = LoginManager::new()?;
    let request = LoginRequest {
        payload: command,
        interactive: true,
    };

    let outcome = manager.execute(request).await?;

    if let Some(elicitation) = outcome.elicitation {
        // Check if client supports elicitation
        if context.supports_elicitation() {
            // Use standard MCP elicitation (currently returns input_text for compatibility)
            // In the future, this would use elicitation/create RPC
            let mut content = Vec::new();
            if !outcome.message.is_empty() {
                content.push(ContentItem::text(outcome.message));
            }
            content.push(ContentItem::input_text(
                elicitation.message,
                json!({
                    "prompt_id": elicitation.prompt_id,
                    "field": elicitation.field,
                }),
            ));
            return Ok(ToolResult::from_items(content));
        } else {
            // Client doesn't support elicitation - return fallback error
            return Ok(create_elicitation_unavailable_error(context, &elicitation.field));
        }
    }

    Ok(ToolResult::text(outcome.message))
}

/// Create error message when elicitation is unavailable
fn create_elicitation_unavailable_error(context: &ServerContext, field: &str) -> ToolResult {
    let client_name = context.get_client_name();
    
    let message = if field == "password" {
        format!(
            r#"# Login via app password failed: **{} does not support interactive prompts** (MCP elicitation). Please choose one of these options:

1. **Use OAuth (strongly recommended):** Call login with your handle:
   {{"handle": "your.handle.bsky.social"}}

2. **Provide app password up-front:** Call login with password:
   {{"handle": "your.handle.bsky.social", "password": "your-app-password"}}

**IMPORTANT Security Warning:**
- Do NOT use your main BlueSky account password
- Create an app password at: https://bsky.app/settings/app-passwords
- OAuth is the most secure option and is strongly preferred
"#,
            client_name
        )
    } else {
        format!(
            r#"# Login requires {} **{} does not support interactive prompts** (MCP elicitation). To complete login, please:

1. **Use OAuth (recommended):** Call login with your handle:
   {{"handle": "your.handle.bsky.social"}}

2. **Or provide credentials up-front:** Call login with both handle and password:
   {{"handle": "your.handle.bsky.social", "password": "your-app-password"}}

**Security Note:** Do NOT use your main BlueSky password. Create an app password at:
https://bsky.app/settings/app-passwords
"#,
            field, client_name
        )
    };

    ToolResult::text(message)
}
