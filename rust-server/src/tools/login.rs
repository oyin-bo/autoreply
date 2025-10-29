use crate::auth::{LoginManager, LoginRequest};
use crate::cli::LoginCommand;
use crate::error::AppError;
use crate::mcp::{McpResponse, ServerContext, ToolResult};
use serde_json::{json, Value};

pub async fn handle_login(id: Option<Value>, args: Value, context: &ServerContext) -> McpResponse {
    match handle_login_impl(args, context).await {
        Ok(result) => McpResponse::success(id, serde_json::to_value(result).unwrap()),
        Err(e) => McpResponse::error(id, e.error_code(), &e.message()),
    }
}

async fn handle_login_impl(args: Value, context: &ServerContext) -> Result<ToolResult, AppError> {
    let mut command: LoginCommand = serde_json::from_value(args)
        .map_err(|e| AppError::InvalidInput(format!("Invalid arguments: {}", e)))?;

    let manager = LoginManager::new()?;

    // Check if we need elicitation for missing fields
    let needs_handle = command.handle.is_none();
    let needs_password = command.password.is_none();

    // If client supports elicitation and we're missing fields, use it
    if context.supports_elicitation() {
        // Elicit handle if missing
        if needs_handle {
            let schema = json!({
                "type": "object",
                "properties": {
                    "handle": {
                        "type": "string",
                        "description": "Your BlueSky handle (e.g., user.bsky.social)"
                    }
                },
                "required": ["handle"]
            });

            match context
                .request_elicitation("Please provide your BlueSky handle".to_string(), schema)
                .await
            {
                Ok(response) => {
                    if response.action == "accept" {
                        if let Some(content) = response.content {
                            if let Some(handle) = content.get("handle").and_then(|v| v.as_str()) {
                                command.handle = Some(handle.to_string());
                            }
                        }
                    } else {
                        return Ok(ToolResult::text("Login cancelled"));
                    }
                }
                Err(e) => {
                    tracing::warn!("Elicitation failed despite client support: {}", e);
                    return Ok(create_elicitation_unavailable_error(context, "handle"));
                }
            }
        }

        // Elicit password if missing and not using OAuth
        if needs_password && command.handle.is_some() {
            let handle = command.handle.as_ref().unwrap();
            let schema = json!({
                "type": "object",
                "properties": {
                    "password": {
                        "type": "string",
                        "description": "BlueSky app password (create at https://bsky.app/settings/app-passwords)"
                    }
                },
                "required": ["password"]
            });

            let message = format!(
                "Please provide a BlueSky app password for @{} (NOT your main password).\n\n\
                Create an app password at: https://bsky.app/settings/app-passwords\n\n\
                Alternatively, cancel and use OAuth authentication instead.",
                handle
            );

            match context.request_elicitation(message, schema).await {
                Ok(response) => match response.action.as_str() {
                    "accept" => {
                        if let Some(content) = response.content {
                            if let Some(password) = content.get("password").and_then(|v| v.as_str())
                            {
                                command.password = Some(password.to_string());
                            }
                        }
                    }
                    "cancel" => {
                        return Ok(ToolResult::text(format!(
                                "Login cancelled. To use OAuth, call login with handle={} and omit the password parameter.",
                                handle
                            )));
                    }
                    _ => {
                        return Ok(ToolResult::text("Login declined"));
                    }
                },
                Err(e) => {
                    tracing::warn!("Password elicitation failed: {}", e);
                    return Ok(create_password_elicitation_unavailable_error(
                        context, handle,
                    ));
                }
            }
        }
    } else if needs_handle || needs_password {
        // Client doesn't support elicitation - return error with guidance
        let field = if needs_handle { "handle" } else { "password" };
        return Ok(create_elicitation_unavailable_error(context, field));
    }

    let request = LoginRequest {
        payload: command,
        interactive: true,
    };

    let outcome = manager.execute(request).await?;

    if let Some(_elicitation) = outcome.elicitation {
        // Elicitation should have been handled via MCP requests above; return message only.
        return Ok(ToolResult::text(outcome.message));
    }

    Ok(ToolResult::text(outcome.message))
}

/// Create error message when elicitation is unavailable
pub(crate) fn create_elicitation_unavailable_error(context: &ServerContext, field: &str) -> ToolResult {
    let client_name = context.get_client_name();

    let message = format!(
        r#"# Login requires {} - but **{} does not support interactive prompts** (MCP elicitation). To complete login, please:

1. **Use OAuth (recommended):** Call login with your handle:
   {{"handle": "your.handle.bsky.social"}}

2. **Or provide credentials up-front:** Call login with both handle and password:
   {{"handle": "your.handle.bsky.social", "password": "your-app-password"}}

**Security Note:** Do NOT use your main BlueSky password. Create an app password at:
https://bsky.app/settings/app-passwords
"#,
        field, client_name
    );

    ToolResult::text(message).with_error_flag()
}

/// Create password-specific error message when elicitation is unavailable
pub(crate) fn create_password_elicitation_unavailable_error(
    context: &ServerContext,
    handle: &str,
) -> ToolResult {
    let client_name = context.get_client_name();

    let message = format!(
        r#"# Login via app password failed: **{} does not support interactive prompts** (MCP elicitation). Please choose one of these options:

1. **Use OAuth (strongly recommended):** Call login with your handle:
   {{"handle": "{}"}}

2. **Provide app password up-front:** Call login with password:
   {{"handle": "{}", "password": "your-app-password"}}

**IMPORTANT Security Warning:**
- Do NOT use your main BlueSky account password
- Create an app password at: https://bsky.app/settings/app-passwords
- OAuth is the most secure option and is strongly preferred
"#,
        client_name, handle, handle
    );

    ToolResult::text(message).with_error_flag()
}
