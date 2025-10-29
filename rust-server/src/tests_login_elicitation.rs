use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn login_elicitation_handle_accept_password_cancel() {
    // Build context with elicitation support and a test hook that returns
    // accept for handle, then cancel for password
    let mut context = crate::mcp::ServerContext::new(None);
    context.client_capabilities = Some(crate::mcp::ClientCapabilities {
        elicitation: Some(crate::mcp::ElicitationCapability {}),
    });

    let call_count = Arc::new(Mutex::new(0usize));
    let cc = call_count.clone();
    context.set_test_elicitation_hook(move |_message: String, _schema: serde_json::Value| {
        let mut n = cc.lock().unwrap();
        *n += 1;
        match *n {
            1 => Ok(crate::mcp::ElicitationResponse {
                action: "accept".to_string(),
                content: Some(json!({"handle": "alice.bsky.social"})),
            }),
            _ => Ok(crate::mcp::ElicitationResponse {
                action: "cancel".to_string(),
                content: None,
            }),
        }
    });

    let args = json!({});
    let response = crate::tools::login::handle_login(None, args, &context).await;
    let result = response.result.expect("success result");
    let tr: crate::mcp::ToolResult = serde_json::from_value(result).expect("ToolResult");
    let text = &tr.content[0].text;
    assert!(text.contains("Login cancelled."), "unexpected text: {}", text);
    assert!(tr.is_error.is_none(), "cancel path should not set isError");
}

#[tokio::test]
async fn login_elicitation_handle_accept_password_decline() {
    // accept for handle, decline for password
    let mut context = crate::mcp::ServerContext::new(None);
    context.client_capabilities = Some(crate::mcp::ClientCapabilities {
        elicitation: Some(crate::mcp::ElicitationCapability {}),
    });

    let call_count = Arc::new(Mutex::new(0usize));
    let cc = call_count.clone();
    context.set_test_elicitation_hook(move |_message: String, _schema: serde_json::Value| {
        let mut n = cc.lock().unwrap();
        *n += 1;
        match *n {
            1 => Ok(crate::mcp::ElicitationResponse {
                action: "accept".to_string(),
                content: Some(json!({"handle": "alice.bsky.social"})),
            }),
            _ => Ok(crate::mcp::ElicitationResponse {
                action: "decline".to_string(),
                content: None,
            }),
        }
    });

    let args = json!({});
    let response = crate::tools::login::handle_login(None, args, &context).await;
    let result = response.result.expect("success result");
    let tr: crate::mcp::ToolResult = serde_json::from_value(result).expect("ToolResult");
    let text = &tr.content[0].text;
    assert!(text.contains("Login declined"), "unexpected text: {}", text);
    assert!(tr.is_error.is_none(), "decline path should not set isError");
}

#[tokio::test]
async fn login_elicitation_handle_decline() {
    // decline at handle prompt
    let mut context = crate::mcp::ServerContext::new(None);
    context.client_capabilities = Some(crate::mcp::ClientCapabilities {
        elicitation: Some(crate::mcp::ElicitationCapability {}),
    });

    context.set_test_elicitation_hook(move |_message: String, _schema: serde_json::Value| {
        Ok(crate::mcp::ElicitationResponse {
            action: "decline".to_string(),
            content: None,
        })
    });

    let args = json!({});
    let response = crate::tools::login::handle_login(None, args, &context).await;
    let result = response.result.expect("success result");
    let tr: crate::mcp::ToolResult = serde_json::from_value(result).expect("ToolResult");
    let text = &tr.content[0].text;
    assert!(text.contains("Login cancelled"), "unexpected text: {}", text);
    assert!(tr.is_error.is_none(), "cancel path should not set isError");
}
