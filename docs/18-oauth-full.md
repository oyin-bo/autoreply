# 18. Asynchronous Browser-Based OAuth Flow for MCP

This document outlines the required behavior and implementation plan for a non-blocking, asynchronous browser-based OAuth 2.0 flow within the MCP (Model Context Protocol) server.

## 1. Conceptual Behavior

The primary goal is to provide a seamless, non-blocking login experience for users of MCP clients (like Gemini CLI) that do not support interactive elicitation. The server should not attempt to open a web browser automatically. The responsibility of presenting the authorization link to the user falls entirely on the MCP client.

### User Experience Flow

1.  **Initiation**: The user invokes the `login` tool via an MCP client. They may optionally provide a `handle` to suggest an account, but no password.
2.  **Immediate Response**: The MCP server immediately performs the following actions:
    *   Generates a unique OAuth state parameter and PKCE challenge.
    *   Constructs the full authorization URL for the AT Protocol service.
    *   Starts a local HTTP server on a free port (e.g., 8080) to listen for the OAuth callback.
    *   Spawns a background task (e.g., a `tokio::spawn` in Rust, a goroutine in Go) to manage the rest of the flow.
    *   **Immediately** returns a `ToolResult` to the MCP client. This result contains a markdown-formatted message with the authorization URL.
3.  **User Action**: The MCP client displays the markdown. The user manually copies the URL and pastes it into their web browser.
4.  **Authorization**: The user authenticates with the service (e.g., BlueSky) and authorizes the application.
5.  **Callback**: The service redirects the user's browser to the local callback URL (e.g., `http://127.0.0.1:8080/callback`) with the authorization `code` and `state` in the query parameters.
6.  **Background Completion**: The background task running on the MCP server handles the callback request:
    *   It validates the received `state` parameter against the one generated at the start.
    *   It exchanges the authorization `code` for an access token and refresh token.
    *   It securely stores the new credentials (session and refresh tokens).
    *   It shuts down the local callback server.
7.  **Completion**: The entire flow completes silently in the background. The original MCP tool call has already finished. There is **no second MCP message** sent to the client upon completion. The user can verify the success of the login by running a command like `login --command=list`.

### Key Principles

*   **Asynchronous**: The initial tool call must not block waiting for user action.
*   **Client-Driven UI**: The server is only responsible for providing the URL. The client is responsible for presenting it. No automatic browser opening.
*   **Stateless from Client's View**: The client's interaction is a single request-response. The state is managed entirely by the server's background process.
*   **Silent Completion**: Success or failure of the background task does not trigger a new message to the client.

---

## 2. Rust Implementation Plan

The changes will be centered in `rust-server/src/auth/login_flow.rs` and `rust-server/src/tools/login.rs`.

### `src/auth/login_flow.rs`

In `LoginManager::authenticate_with_oauth`:

1.  **Signature Change**: The function will no longer return a `Result<String, AppError>` containing a final success message. It will instead return a `Result<String, AppError>` containing the initial markdown message with the URL.
2.  **Remove Browser Opening**: Delete the `webbrowser::open(...)` call.
3.  **Spawn Background Task**: The core logic following the URL generation will be moved into a `tokio::spawn` block.
    *   The `CallbackServer` and `AtProtoOAuthManager` instances (or the necessary parts of them) will need to be moved into the async block.
    *   The `wait_for_callback`, `complete_flow`, and credential storage logic will all run inside this background task.
    *   Error handling within the task should log to `stderr` or a log file, as it can no longer return an `AppError` to the user.
4.  **Immediate Return**: The function will construct and return the markdown string immediately after spawning the background task.

**Example (Conceptual):**

```rust
// in src/auth/login_flow.rs

async fn authenticate_with_oauth(&self, ...) -> Result<String, AppError> {
    // ... setup callback_server and oauth_manager ...
    let flow_state = oauth_manager.start_browser_flow(handle).await?;
    let auth_url = flow_state.auth_url.clone();
    let port = callback_server.port();

    // Move ownership of required components into the background task
    let storage = self.storage.clone();
    tokio::spawn(async move {
        let callback_result = callback_server
            .wait_for_callback(Duration::from_secs(300))
            .await;
        
        // Handle callback, exchange code, store credentials...
        // Log any errors that occur here.
    });

    // Immediately return the markdown for the MCP client
    Ok(format!(
        "# OAuth Login Initiated\n\n1. Open this URL in your browser:\n   {}\n\n2. Authorize the application.\n\nWaiting for authorization on port {}...",
        auth_url, port
    ))
}
```

### `src/tools/login.rs`

In `LoginTool::perform_login`:

*   The logic that calls `login_manager.execute()` for the OAuth path will now receive the markdown string directly and can wrap it in a `ToolResult` to send back to the client. The `is_error` flag will be `false`.

---

## 3. Go Implementation Plan

The changes will be focused in `go-server/internal/tools/login.go`.

### `internal/tools/login.go`

In `LoginTool.loginWithOAuth`:

1.  **Signature Change**: The function will continue to return `(*mcp.ToolResult, error)`, but its behavior will change.
2.  **Remove Stderr Printing**: Delete the `fmt.Fprint(os.Stderr, message)` call.
3.  **Spawn Goroutine**: The blocking logic will be moved into a new goroutine.
    *   The call to `callbackServer.WaitForCallback(ctx)` and all subsequent steps (exchanging the code, resolving the DID, storing credentials) will run in this goroutine.
    *   The `context` passed to the goroutine should probably be `context.Background()` to ensure it outlives the original request context.
    *   Error handling within the goroutine should use `log.Printf` or a similar logging mechanism.
4.  **Immediate Return**: The function will construct the `*mcp.ToolResult` with the markdown message and return it immediately after launching the goroutine.

**Example (Conceptual):**

```go
// in go-server/internal/tools/login.go

func (t *LoginTool) loginWithOAuth(ctx context.Context, handle string, port int) (*mcp.ToolResult, error) {
    // ... setup config, flow, requestURI, authURL ...
    
    callbackServer := auth.NewCallbackServer(port)
    // ... start callback server ...

    // Launch background goroutine to wait for callback and finish the flow
    go func() {
        // Use a new background context for the long-running task
        bgCtx := context.Background() 
        
        callbackResult, err := callbackServer.WaitForCallback(bgCtx)
        if err != nil {
            log.Printf("OAuth background error: %v", err)
            return
        }

        // ... handle callback, exchange code, store credentials ...
        // ... log any further errors ...
    }()

    // Immediately return the markdown result to the MCP client
    message := fmt.Sprintf(
        "# OAuth Login Initiated\n\n1. Open this URL in your browser:\n   %s\n\n2. Authorize the application.\n\nWaiting for authorization on port %d...",
        authURL, port,
    )
    return &mcp.ToolResult{
        Content: []mcp.ContentItem{{
            Type: "text",
            Text: message,
        }},
    }, nil
}
```
