//! Local HTTP callback server for OAuth browser flow
//!
//! This module provides a minimal HTTP server that listens for OAuth callbacks
//! on localhost. It's designed for native/CLI applications following the OAuth
//! loopback redirect pattern.

use axum::{extract::Query, response::Html, routing::get, Router};
use serde::Deserialize;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use tokio::sync::oneshot;

/// OAuth callback parameters
#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Result of waiting for OAuth callback
#[derive(Debug)]
pub enum CallbackResult {
    Success {
        code: String,
        state: String,
    },
    Error {
        error: String,
        description: Option<String>,
    },
}

/// Local callback server for OAuth
pub struct CallbackServer {
    port: u16,
    addr: SocketAddr,
}

impl CallbackServer {
    /// Create a new callback server on a random available port
    pub fn new() -> Result<Self, std::io::Error> {
        // Bind to localhost on a random available port
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let port = addr.port();

        // Release the port so axum can bind to it
        drop(listener);

        Ok(Self { port, addr })
    }

    /// Get the callback URL
    #[allow(dead_code)] // Used in future DPoP implementation
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the callback URL (for loopback OAuth - no path component)
    pub fn callback_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Start the server and wait for a callback
    ///
    /// Returns when either:
    /// - OAuth callback is received
    /// - Timeout occurs
    /// - Server error occurs
    pub async fn wait_for_callback(
        self,
        timeout: std::time::Duration,
    ) -> Result<CallbackResult, String> {
        let (tx, rx) = oneshot::channel();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        // Create the callback handler
        let callback_tx = tx.clone();
        let callback_handler = move |Query(params): Query<CallbackParams>| async move {
            let result = if let Some(error) = params.error {
                CallbackResult::Error {
                    error,
                    description: params.error_description,
                }
            } else if let (Some(code), Some(state)) = (params.code, params.state) {
                CallbackResult::Success { code, state }
            } else {
                CallbackResult::Error {
                    error: "invalid_request".to_string(),
                    description: Some("Missing code or state parameter".to_string()),
                }
            };

            // Send the result through the channel
            if let Some(tx) = callback_tx.lock().await.take() {
                let _ = tx.send(result);
            }

            // Return success page
            Html(SUCCESS_PAGE)
        };

        // Build the router - handle OAuth callback at root path per loopback spec
        let app = Router::new().route("/", get(callback_handler));

        // Start the server
        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .map_err(|e| format!("Failed to bind callback server: {}", e))?;

        tracing::debug!("OAuth callback server listening on {}", self.addr);

        // Spawn the server in a separate task
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| format!("Callback server error: {}", e))
        });

        // Wait for either callback or timeout
        let result = tokio::select! {
            callback = rx => {
                callback.map_err(|_| "Callback channel closed".to_string())
            }
            _ = tokio::time::sleep(timeout) => {
                Err("Timeout waiting for OAuth callback".to_string())
            }
        };

        // Shutdown the server gracefully
        server_handle.abort();
        // Wait a bit for the server to actually shut down
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), server_handle).await;

        result
    }
}

const SUCCESS_PAGE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Authentication Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            background: white;
            padding: 3rem;
            border-radius: 1rem;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            text-align: center;
            max-width: 400px;
        }
        .success-icon {
            width: 80px;
            height: 80px;
            background: #10b981;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 1.5rem;
            font-size: 40px;
            color: white;
        }
        h1 {
            color: #1f2937;
            margin: 0 0 0.5rem;
            font-size: 1.75rem;
        }
        p {
            color: #6b7280;
            margin: 0;
            line-height: 1.6;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="success-icon">✓</div>
        <h1>Authentication Successful!</h1>
        <p>You can close this window and return to the CLI.</p>
    </div>
</body>
</html>"#;
