//! MCP (Model Context Protocol) handling module
//!
//! This module implements the JSON-RPC 2.0 protocol for MCP communication.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

/// Server context for tracking client information and bidirectional RPC
#[derive(Clone)]
pub struct ServerContext {
    pub client_info: Option<ClientInfo>,
    pub client_capabilities: Option<ClientCapabilities>,
    pub rpc_sender: Option<Arc<RpcSender>>,
    #[cfg(test)]
    pub test_elicitation_hook: Option<Arc<dyn Fn(String, Value) -> anyhow::Result<ElicitationResponse> + Send + Sync>>, 
}

/// RPC sender for server-to-client requests
pub struct RpcSender {
    next_id: AtomicI64,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
    pending_responses: Arc<Mutex<HashMap<i64, mpsc::Sender<McpResponse>>>>,
}

impl RpcSender {
    pub fn new(stdout: tokio::io::Stdout) -> Self {
        Self {
            next_id: AtomicI64::new(1),
            stdout: Arc::new(Mutex::new(stdout)),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send an elicitation/create request and wait for response
    pub async fn request_elicitation(
        &self,
        message: String,
        requested_schema: Value,
    ) -> Result<ElicitationResponse> {
        let request_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Create response channel
        let (tx, mut rx) = mpsc::channel(1);

        // Register pending response
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(request_id, tx);
        }

        // Build and send request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "elicitation/create",
            "params": {
                "message": message,
                "requestedSchema": requested_schema
            }
        });

        let request_json = serde_json::to_string(&request)?;
        debug!(
            "Sending elicitation/create request ID={}: {}",
            request_id, request_json
        );

        {
            let mut stdout = self.stdout.lock().await;
            stdout.write_all(request_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        // Wait for response
        let response = rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Elicitation response channel closed"))?;

        // Cleanup
        {
            let mut pending = self.pending_responses.lock().await;
            pending.remove(&request_id);
        }

        // Parse response
        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Elicitation error: {}", error.message));
        }

        let result = response
            .result
            .ok_or_else(|| anyhow::anyhow!("Elicitation response missing result"))?;

        let elicitation_response: ElicitationResponse = serde_json::from_value(result)?;
        Ok(elicitation_response)
    }

    /// Handle incoming response from client
    pub async fn handle_response(&self, response: McpResponse) {
        if let Some(Value::Number(id_num)) = &response.id {
            if let Some(id) = id_num.as_i64() {
                let pending = self.pending_responses.lock().await;
                if let Some(tx) = pending.get(&id) {
                    let _ = tx.send(response).await;
                    debug!("Delivered response for request ID={}", id);
                } else {
                    debug!("Warning: Received response for unknown request ID={}", id);
                }
            }
        }
    }
}

impl ServerContext {
    pub fn new(rpc_sender: Option<Arc<RpcSender>>) -> Self {
        Self {
            client_info: None,
            client_capabilities: None,
            rpc_sender,
            #[cfg(test)]
            test_elicitation_hook: None,
        }
    }

    pub fn supports_elicitation(&self) -> bool {
        self.client_capabilities
            .as_ref()
            .and_then(|c| c.elicitation.as_ref())
            .is_some()
    }

    pub fn get_client_name(&self) -> String {
        self.client_info
            .as_ref()
            .and_then(|info| info.name.as_ref())
            .cloned()
            .unwrap_or_else(|| "Unknown Client".to_string())
    }

    /// Request elicitation from client (if supported)
    pub async fn request_elicitation(
        &self,
        message: String,
        requested_schema: Value,
    ) -> Result<ElicitationResponse> {
        // In tests, allow injecting a synthetic elicitation response without stdio
        #[cfg(test)]
        if let Some(hook) = &self.test_elicitation_hook {
            return hook(message, requested_schema);
        }

        let rpc_sender = self
            .rpc_sender
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RPC sender not initialized"))?;

        if !self.supports_elicitation() {
            return Err(anyhow::anyhow!("Client does not support elicitation"));
        }

        rpc_sender
            .request_elicitation(message, requested_schema)
            .await
    }
}

#[cfg(test)]
impl ServerContext {
    pub fn set_test_elicitation_hook<F>(&mut self, f: F)
    where
        F: Fn(String, Value) -> anyhow::Result<ElicitationResponse> + Send + Sync + 'static,
    {
        self.test_elicitation_hook = Some(Arc::new(f));
    }
}

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

/// Initialize request parameters
#[derive(Debug, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "clientInfo")]
    pub client_info: Option<ClientInfo>,
    pub capabilities: Option<ClientCapabilities>,
}

/// Client information
#[derive(Debug, Deserialize, Clone)]
pub struct ClientInfo {
    pub name: Option<String>,
    #[allow(dead_code)]
    pub version: Option<String>,
}

/// Client capabilities
#[derive(Debug, Deserialize, Clone)]
pub struct ClientCapabilities {
    pub elicitation: Option<ElicitationCapability>,
}

/// Elicitation capability (presence indicates support)
#[derive(Debug, Deserialize, Clone)]
pub struct ElicitationCapability {
    // Empty struct - presence indicates support
}

/// Elicitation request (server -> client)
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct ElicitationRequest {
    pub message: String,
    #[serde(rename = "requestedSchema")]
    pub requested_schema: Value,
}

/// Elicitation response (client -> server)
#[derive(Debug, Deserialize)]
pub struct ElicitationResponse {
    pub action: String, // "accept", "decline", "cancel"
    pub content: Option<Value>,
}

/// MCP JSON-RPC 2.0 response structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP Error structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpError {
    // JSON-RPC uses numeric error codes; use i64 so clients expecting numbers validate correctly
    pub code: i64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// MCP Tool result
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: Vec<ContentItem>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "isError")]
    pub is_error: Option<bool>,
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
                code: map_error_code(code),
                message: message.to_string(),
            }),
        }
    }
}

/// Map string error identifiers used internally to JSON-RPC numeric error codes.
/// This keeps existing callsites using string codes unchanged while producing
/// the numeric `error.code` that MCP clients expect.
fn map_error_code(code: &str) -> i64 {
    match code {
        "parse_error" => -32700,
        "invalid_request" => -32600,
        "method_not_found" => -32601,
        "invalid_params" => -32602,
        "internal_error" => -32603,
        // Application/tool-specific errors go in the server error range
        "tool_not_found" => -32001,
        "tool_error" => -32002,
        _ => -32000,
    }
}

impl ToolResult {
    /// Create a text result
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ContentItem::text(content)],
            is_error: None,
        }
    }

    /// Create a result from explicit content items
    pub fn from_items(content: Vec<ContentItem>) -> Self {
        Self { content, is_error: None }
    }

    /// Mark this result as an error with user-facing guidance
    pub fn with_error_flag(mut self) -> Self {
        self.is_error = Some(true);
        self
    }
}

impl ContentItem {
    /// Helper to create plain text content
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            r#type: "text".to_string(),
            text: content.into(),
            metadata: None,
        }
    }

    /// Helper to create an input prompt content item
    pub fn input_text(prompt: impl Into<String>, metadata: Value) -> Self {
        Self {
            r#type: "input_text".to_string(),
            text: prompt.into(),
            metadata: Some(metadata),
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
    let stdout = tokio::io::stdout();

    // Create RPC sender for bidirectional communication
    let rpc_sender = Arc::new(RpcSender::new(stdout));

    // Track server context with RPC sender
    let mut context = ServerContext::new(Some(rpc_sender.clone()));

    while let Some(line) = reader.next_line().await? {
        debug!("Received message: {}", line);

        // Try to parse as response first (has "result" or "error" but is response to our request)
        if let Ok(response) = serde_json::from_str::<McpResponse>(&line) {
            if response.id.is_some() && (response.result.is_some() || response.error.is_some()) {
                // Check if this is a response to one of our pending requests
                if let Some(Value::Number(id_num)) = &response.id {
                    if id_num.as_i64().is_some() {
                        // This looks like a response to our elicitation request
                        rpc_sender.handle_response(response).await;
                        continue;
                    }
                }
            }
        }

        // Otherwise parse as request from client
        let parsed = match parse_request(&line) {
            Ok(request) => request,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                let response = McpResponse::error(None, "parse_error", &format!("Invalid JSON: {}", e));
                let response_json = serialize_response(&response)?;
                debug!("Sending response: {}", response_json);
                let mut stdout = rpc_sender.stdout.lock().await;
                stdout.write_all(response_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };

        // Do not respond to JSON-RPC notifications (no id)
        if parsed.id.is_none() {
            // Known notification: "notifications/initialized"; silently ignore per spec
            continue;
        }

        let response = handle_request(parsed, &mut context).await;

        let response_json = serialize_response(&response)?;
        debug!("Sending response: {}", response_json);

        {
            let mut stdout = rpc_sender.stdout.lock().await;
            stdout.write_all(response_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}

/// Handle a single MCP request
async fn handle_request(request: McpRequest, context: &mut ServerContext) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request, context).await,
        "tools/call" => handle_tool_call(request, context).await,
        "tools/list" => handle_tools_list(request).await,
        _ => McpResponse::error(
            request.id,
            "method_not_found",
            &format!("Method '{}' not found", request.method),
        ),
    }
}

/// Handle tools/call method
async fn handle_tool_call(request: McpRequest, context: &ServerContext) -> McpResponse {
    let args: ToolCallArgs = match serde_json::from_value(request.params.unwrap_or_default()) {
        Ok(args) => args,
        Err(e) => {
            return McpResponse::error(
                request.id.clone(),
                "invalid_params",
                &format!("Invalid parameters: {}", e),
            )
        }
    };

    match args.name.as_str() {
        "profile" => crate::tools::profile::handle_profile(request.id, args.arguments).await,
        "search" => crate::tools::search::handle_search(request.id, args.arguments).await,
        "login" => crate::tools::login::handle_login(request.id, args.arguments, context).await,
        "feed" => crate::tools::feed::handle_feed(request.id, args.arguments).await,
        "thread" => crate::tools::thread::handle_thread(request.id, args.arguments).await,
        "post" => crate::tools::post::handle_post(request.id, args.arguments).await,
        "react" => crate::tools::react::handle_react(request.id, args.arguments).await,
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
async fn handle_initialize(request: McpRequest, context: &mut ServerContext) -> McpResponse {
    // Parse initialize params
    if let Some(params) = request.params {
        if let Ok(init_params) = serde_json::from_value::<InitializeParams>(params) {
            // Store client info and capabilities
            context.client_info = init_params.client_info;
            context.client_capabilities = init_params.capabilities;

            if context.supports_elicitation() {
                info!("Client supports elicitation");
            } else {
                info!("Client does not support elicitation - will use fallback error messages");
            }
        }
    }

    let tools = build_tools_array();
    let result = serde_json::json!({
        "protocolVersion": "2024-11-05",
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
pub(crate) fn build_tools_array() -> serde_json::Value {
    use crate::cli::{FeedArgs, LoginCommand, PostArgs, ProfileArgs, ReactArgs, SearchArgs, ThreadArgs};
    use schemars::schema_for;

    // Generate JSON schemas from the CLI argument structs
    let profile_schema = schema_for!(ProfileArgs);
    let search_schema = schema_for!(SearchArgs);
    let login_schema = schema_for!(LoginCommand);
    let feed_schema = schema_for!(FeedArgs);
    let thread_schema = schema_for!(ThreadArgs);
    let post_schema = schema_for!(PostArgs);
    let react_schema = schema_for!(ReactArgs);

    serde_json::json!([
        {
            "name": "profile",
            "description": "Retrieve user profile information",
            "inputSchema": profile_schema
        },
        {
            "name": "search",
            "description": "Search posts within a user's repository",
            "inputSchema": search_schema
        },
        {
            "name": "login",
            "description": "Authenticate accounts and manage stored credentials. Handle parameter is optional for OAuth (allows account selection in browser). Subcommands: list, default, delete",
            "inputSchema": login_schema
        },
        {
            "name": "feed",
            "description": "Get the latest feed from BlueSky. Returns a list of posts from a feed. If you want to see the latest posts from a specific feed, provide the feed URI or name. These feeds are paginated.",
            "inputSchema": feed_schema
        },
        {
            "name": "thread",
            "description": "Fetch a thread by post URI. Returns all the replies and replies to replies, the whole thread.",
            "inputSchema": thread_schema
        },
        {
            "name": "post",
            "description": "Create a new post or reply on BlueSky. Supports text content and replying to existing posts via at:// URI or https://bsky.app/... URL.",
            "inputSchema": post_schema
        },
        {
            "name": "react",
            "description": "Perform batch reactions on BlueSky posts (like, unlike, repost, delete). All operations support both at:// URIs and https://bsky.app/... URLs. Partial success is allowed - some operations may succeed while others fail.",
            "inputSchema": react_schema
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
        let mut context = ServerContext::new(None);
        let resp = handle_request(req, &mut context).await;
        assert!(resp.error.is_none());
        let result = resp.result.expect("result present");
        assert_eq!(
            result
                .get("protocolVersion")
                .and_then(|v| v.as_str()),
            Some("2024-11-05")
        );
        assert_eq!(
            result
                .get("serverInfo")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("autoreply")
        );
        assert_eq!(
            result
                .get("capabilities")
                .and_then(|v| v.get("tools"))
                .and_then(|v| v.get("list"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
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
        let mut context = ServerContext::new(None);
        let resp = handle_request(req, &mut context).await;
        assert!(resp.error.is_none());
        let result = resp.result.expect("result present");
        let tools = result
            .get("tools")
            .and_then(|v| v.as_array())
            .expect("tools array");
        let names: Vec<String> = tools
            .iter()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        assert!(names.contains(&"profile".to_string()));
        assert!(names.contains(&"search".to_string()));
        assert!(names.contains(&"feed".to_string()));
        assert!(names.contains(&"thread".to_string()));
    }

    #[tokio::test]
    async fn test_server_context_supports_elicitation() {
        // Test with elicitation support
        let context_with = ServerContext {
            client_info: None,
            client_capabilities: Some(ClientCapabilities {
                elicitation: Some(ElicitationCapability {}),
            }),
            rpc_sender: None,
            #[cfg(test)]
            test_elicitation_hook: None,
        };
        assert!(context_with.supports_elicitation());

        // Test without elicitation support
        let context_without = ServerContext {
            client_info: None,
            client_capabilities: Some(ClientCapabilities { elicitation: None }),
            rpc_sender: None,
            #[cfg(test)]
            test_elicitation_hook: None,
        };
        assert!(!context_without.supports_elicitation());

        // Test with no capabilities
        let context_none = ServerContext::new(None);
        assert!(!context_none.supports_elicitation());
    }

    #[tokio::test]
    async fn test_server_context_get_client_name() {
        // Test with client name
        let context_with_name = ServerContext {
            client_info: Some(ClientInfo {
                name: Some("Test Client".to_string()),
                version: None,
            }),
            client_capabilities: None,
            rpc_sender: None,
            #[cfg(test)]
            test_elicitation_hook: None,
        };
        assert_eq!(context_with_name.get_client_name(), "Test Client");

        // Test without client name
        let context_without = ServerContext::new(None);
        assert_eq!(context_without.get_client_name(), "Unknown Client");

        // Test with empty client name
        let context_empty = ServerContext {
            client_info: Some(ClientInfo {
                name: Some("".to_string()),
                version: None,
            }),
            client_capabilities: None,
            rpc_sender: None,
            #[cfg(test)]
            test_elicitation_hook: None,
        };
        assert_eq!(context_empty.get_client_name(), "");
    }

    #[tokio::test]
    async fn test_request_elicitation_no_sender() {
        // Context with capability but no RPC sender (checks sender first)
        let context = ServerContext {
            client_info: None,
            client_capabilities: Some(ClientCapabilities {
                elicitation: Some(ElicitationCapability {}),
            }),
            rpc_sender: None,
            #[cfg(test)]
            test_elicitation_hook: None,
        };

        let result = context
            .request_elicitation("Test".to_string(), json!({}))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("RPC sender not initialized"));
    }

    #[tokio::test]
    async fn test_request_elicitation_no_capability() {
        // Context with sender but no elicitation capability
        // Need actual stdout for sender, so we'll create context differently
        let stdout = tokio::io::stdout();
        let rpc_sender = Arc::new(RpcSender::new(stdout));

        let context = ServerContext {
            client_info: None,
            client_capabilities: Some(ClientCapabilities { elicitation: None }),
            rpc_sender: Some(rpc_sender),
            #[cfg(test)]
            test_elicitation_hook: None,
        };

        let result = context
            .request_elicitation("Test".to_string(), json!({}))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not support elicitation"));
    }

    #[tokio::test]
    async fn test_rpc_sender_construction() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Verify initial state
        assert_eq!(sender.next_id.load(Ordering::SeqCst), 1);

        // Verify pending responses is empty
        let pending = sender.pending_responses.lock().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_request_id_uniqueness() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Generate multiple IDs
        let id1 = sender.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = sender.next_id.fetch_add(1, Ordering::SeqCst);
        let id3 = sender.next_id.fetch_add(1, Ordering::SeqCst);

        // Verify uniqueness and ordering
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[tokio::test]
    async fn test_concurrent_request_id_generation() {
        use std::collections::HashSet;
        use tokio::task;

        let stdout = tokio::io::stdout();
        let sender = Arc::new(RpcSender::new(stdout));

        // Spawn 20 concurrent tasks generating IDs
        let mut handles = vec![];
        for _ in 0..20 {
            let sender_clone = sender.clone();
            let handle =
                task::spawn(async move { sender_clone.next_id.fetch_add(1, Ordering::SeqCst) });
            handles.push(handle);
        }

        // Collect all IDs
        let mut ids = HashSet::new();
        for handle in handles {
            let id = handle.await.unwrap();
            ids.insert(id);
        }

        // Verify all IDs are unique
        assert_eq!(ids.len(), 20, "All 20 IDs should be unique");

        // Verify IDs are in expected range (1-20)
        for id in &ids {
            assert!(*id >= 1 && *id <= 20, "ID {} should be in range 1-20", id);
        }
    }

    #[tokio::test]
    async fn test_handle_response_delivers_to_pending() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Create a pending response channel
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut pending = sender.pending_responses.lock().await;
            pending.insert(42, tx);
        }

        // Simulate incoming response
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(42)),
            result: Some(json!({"type": "response", "data": "test"})),
            error: None,
        };

        sender.handle_response(response.clone()).await;

        // Verify response was delivered
        let received = rx.recv().await.expect("Should receive response");
        assert_eq!(received.id, Some(json!(42)));
        assert!(received.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_response_unknown_id() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // No pending responses registered

        // Simulate incoming response with unknown ID
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(999)),
            result: Some(json!({"type": "response"})),
            error: None,
        };

        // Should not panic, just log warning
        sender.handle_response(response).await;

        // Verify pending is still empty
        let pending = sender.pending_responses.lock().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_handle_response_malformed_id_string() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Response with string ID (should be ignored - not a number)
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("not-a-number")),
            result: Some(json!({"data": "test"})),
            error: None,
        };

        sender.handle_response(response).await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_handle_response_null_id() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Response with null ID
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: Some(json!({"data": "test"})),
            error: None,
        };

        sender.handle_response(response).await;
        // Should not panic
    }

    #[tokio::test]
    async fn test_request_elicitation_error_response() {
        use std::sync::Arc;

        // Create an in-memory buffer to capture stdout
        let (_reader, _writer) = tokio::io::duplex(1024);
        let sender = Arc::new(RpcSender::new(tokio::io::stdout()));

        // We can't easily test the full cycle without refactoring, but we can test
        // error response handling by creating a mock response
        let error_response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            result: None,
            error: Some(McpError {
                code: "-32602".to_string(),
                message: "Invalid params".to_string(),
            }),
        };

        // Create channel to simulate pending request
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut pending = sender.pending_responses.lock().await;
            pending.insert(1, tx);
        }

        // Deliver error response
        sender.handle_response(error_response).await;

        // Verify error was delivered
        let received = rx.recv().await.expect("Should receive error response");
        assert!(received.error.is_some());
        assert_eq!(received.error.as_ref().unwrap().message, "Invalid params");
        assert!(received.result.is_none());
    }

    #[tokio::test]
    async fn test_pending_response_cleanup_after_delivery() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        // Create pending response
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut pending = sender.pending_responses.lock().await;
            pending.insert(100, tx);
        }

        // Verify it's registered
        {
            let pending = sender.pending_responses.lock().await;
            assert_eq!(pending.len(), 1);
        }

        // Deliver response
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(100)),
            result: Some(json!({"status": "ok"})),
            error: None,
        };
        sender.handle_response(response).await;

        // Receive it
        let _ = rx.recv().await;

        // Note: In current implementation, cleanup happens in request_elicitation
        // after receiving, not in handle_response. This test shows the response
        // is still in pending map after delivery.
        let pending = sender.pending_responses.lock().await;
        assert_eq!(
            pending.len(),
            1,
            "Cleanup happens in request_elicitation, not handle_response"
        );
    }

    #[tokio::test]
    async fn test_multiple_concurrent_pending_responses() {
        use std::collections::HashSet;

        let stdout = tokio::io::stdout();
        let sender = Arc::new(RpcSender::new(stdout));

        // Create 10 pending responses
        let mut receivers = vec![];
        for i in 1..=10 {
            let (tx, rx) = mpsc::channel(1);
            {
                let mut pending = sender.pending_responses.lock().await;
                pending.insert(i, tx);
            }
            receivers.push((i, rx));
        }

        // Verify all registered
        {
            let pending = sender.pending_responses.lock().await;
            assert_eq!(pending.len(), 10);
        }

        // Send responses out of order
        for i in [3, 7, 1, 9, 2, 5, 10, 4, 8, 6] {
            let response = McpResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(json!(i)),
                result: Some(json!({"request_id": i})),
                error: None,
            };
            sender.handle_response(response).await;
        }

        // Verify all responses received correctly
        let mut received_ids = HashSet::new();
        for (expected_id, mut rx) in receivers {
            let response = rx.recv().await.expect("Should receive response");
            if let Some(Value::Number(id)) = response.id {
                let id_val = id.as_i64().unwrap();
                assert_eq!(id_val, expected_id);
                received_ids.insert(id_val);
            }
        }

        assert_eq!(
            received_ids.len(),
            10,
            "All 10 responses should be received"
        );
    }

    #[tokio::test]
    async fn test_response_with_missing_result() {
        let stdout = tokio::io::stdout();
        let sender = RpcSender::new(stdout);

        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut pending = sender.pending_responses.lock().await;
            pending.insert(50, tx);
        }

        // Response with neither result nor error (malformed)
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(50)),
            result: None,
            error: None,
        };

        sender.handle_response(response).await;

        let received = rx.recv().await.expect("Should receive response");
        assert!(received.result.is_none());
        assert!(received.error.is_none());
        // This would cause an error in request_elicitation which checks for result
    }

    #[tokio::test]
    async fn test_elicitation_response_schema_parsing() {
        // Test that ElicitationResponse can be parsed from valid JSON
        let json_response = json!({
            "action": "accept",
            "content": {"field": "value"}
        });

        let parsed: Result<ElicitationResponse, _> = serde_json::from_value(json_response);
        assert!(parsed.is_ok());

        let response = parsed.unwrap();
        assert_eq!(response.action, "accept");
        assert!(response.content.is_some());
        assert!(response.content.unwrap().is_object());
    }

    #[tokio::test]
    async fn test_elicitation_response_schema_invalid() {
        // Test that invalid schema fails to parse
        let json_response = json!({
            // Missing action field
            "content": {"field": "value"}
        });

        let parsed: Result<ElicitationResponse, _> = serde_json::from_value(json_response);
        assert!(parsed.is_err(), "Should fail to parse invalid schema");
    }
}
