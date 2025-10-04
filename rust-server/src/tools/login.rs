use crate::auth::{LoginManager, LoginRequest};
use crate::cli::LoginCommand;
use crate::error::AppError;
use crate::mcp::{ContentItem, McpResponse, ToolResult};
use serde_json::{json, Value};

pub async fn handle_login(id: Option<Value>, args: Value) -> McpResponse {
    match handle_login_impl(args).await {
        Ok(result) => McpResponse::success(id, serde_json::to_value(result).unwrap()),
        Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
    }
}

async fn handle_login_impl(args: Value) -> Result<ToolResult, AppError> {
    let command: LoginCommand = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    let manager = LoginManager::new()?;
    let request = LoginRequest {
        payload: command,
        interactive: true,
    };

    let outcome = manager.execute(request).await?;

    if let Some(elicitation) = outcome.elicitation {
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
    }

    Ok(ToolResult::text(outcome.message))
}
