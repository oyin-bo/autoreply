//! MCP (Model Context Protocol) handling module
//!
//! This module implements the JSON-RPC 2.0 protocol for MCP communication.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tracing::{error, info, debug};

/// MCP JSON-RPC 2.0 request structure
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version field - required by spec but not accessed in code
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// MCP JSON-RPC 2.0 response structure
#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP Error structure
#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: String,
    pub message: String,
}

/// MCP Tool call arguments
#[derive(Debug, Deserialize)]
pub struct ToolCallArgs {
    pub name: String,
    pub arguments: Value,
}

/// MCP Content item
#[derive(Debug, Serialize)]
pub struct ContentItem {
    pub r#type: String,
    pub text: String,
}

/// MCP Tool result
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: Vec<ContentItem>,
}

impl McpResponse {
    /// Create a successful response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, code: &str, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

impl ToolResult {
    /// Create a text result
    pub fn text(content: String) -> Self {
        Self {
            content: vec![ContentItem {
                r#type: "text".to_string(),
                text: content,
            }],
        }
    }
}

/// Parse MCP request from JSON string
pub fn parse_request(json: &str) -> Result<McpRequest> {
    let request: McpRequest = serde_json::from_str(json)?;
    Ok(request)
}

/// Serialize MCP response to JSON string
pub fn serialize_response(response: &McpResponse) -> Result<String> {
    Ok(serde_json::to_string(response)?)
}

/// Handle stdio MCP communication
pub async fn handle_stdio() -> Result<()> {
    info!("Starting autoreply MCP server on stdio");
    
    let stdin = tokio::io::stdin();
    let mut reader = AsyncBufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = reader.next_line().await? {
        debug!("Received request: {}", line);
        
        let response = match parse_request(&line) {
            Ok(request) => handle_request(request).await,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                McpResponse::error(None, "parse_error", &format!("Invalid JSON: {}", e))
            }
        };

        let response_json = serialize_response(&response)?;
        debug!("Sending response: {}", response_json);
        
        stdout.write_all(response_json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

/// Handle a single MCP request
async fn handle_request(request: McpRequest) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request).await,
        "tools/call" => handle_tool_call(request).await,
        "tools/list" => handle_tools_list(request).await,
        _ => McpResponse::error(
            request.id,
            "method_not_found",
            &format!("Method '{}' not found", request.method),
        ),
    }
}

/// Handle tools/call method
async fn handle_tool_call(request: McpRequest) -> McpResponse {
    let args: ToolCallArgs = match serde_json::from_value(request.params.unwrap_or_default()) {
        Ok(args) => args,
        Err(e) => {
            return McpResponse::error(
                request.id,
                "invalid_params",
                &format!("Invalid parameters: {}", e),
            )
        }
    };

    match args.name.as_str() {
        "profile" => crate::tools::profile::handle_profile(request.id, args.arguments).await,
        "search" => crate::tools::search::handle_search(request.id, args.arguments).await,
        _ => McpResponse::error(
            request.id,
            "tool_not_found",
            &format!("Tool '{}' not found", args.name),
        ),
    }
}

/// Handle tools/list method
async fn handle_tools_list(request: McpRequest) -> McpResponse {
    let tools = build_tools_array();

    McpResponse::success(request.id, serde_json::json!({ "tools": tools }))
}

/// Handle initialize method
async fn handle_initialize(request: McpRequest) -> McpResponse {
    let tools = build_tools_array();
    let result = serde_json::json!({
        "serverInfo": {
            "name": "autoreply",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": { "list": true, "call": true }
        },
        "tools": tools
    });
    McpResponse::success(request.id, result)
}

/// Build the tools array returned from tools/list and initialize
fn build_tools_array() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "profile",
            "description": "Retrieve user profile information",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": {
                        "type": "string",
                        "description": "Handle (alice.bsky.social) or DID (did:plc:...)"
                    }
                },
                "required": ["account"]
            }
        },
        {
            "name": "search",
            "description": "Search posts within a user's repository",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": {
                        "type": "string",
                        "description": "Handle or DID"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search terms (case-insensitive)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default 50, max 200)"
                    }
                },
                "required": ["account", "query"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_initialize_response_contains_fields() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "initialize".into(),
            params: None,
        };
        let resp = handle_request(req).await;
        assert!(resp.error.is_none());
        let result = resp.result.expect("result present");
        assert_eq!(result.get("serverInfo").and_then(|v| v.get("name")).and_then(|v| v.as_str()), Some("autoreply"));
        assert_eq!(result.get("capabilities").and_then(|v| v.get("tools")).and_then(|v| v.get("list")).and_then(|v| v.as_bool()), Some(true));
        assert!(result.get("tools").and_then(|v| v.as_array()).is_some());
    }

    #[tokio::test]
    async fn test_tools_list_contains_profile_and_search() {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(2)),
            method: "tools/list".into(),
            params: None,
        };
        let resp = handle_request(req).await;
        assert!(resp.error.is_none());
        let result = resp.result.expect("result present");
        let tools = result.get("tools").and_then(|v| v.as_array()).expect("tools array");
        let names: Vec<String> = tools.iter().filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())).collect();
        assert!(names.contains(&"profile".to_string()));
        assert!(names.contains(&"search".to_string()));
    }
}