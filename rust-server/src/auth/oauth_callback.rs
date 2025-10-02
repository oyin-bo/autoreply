use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// OAuth callback result
#[derive(Debug, Clone)]
pub struct OAuthCallbackResult {
    pub code: String,
    pub state: String,
}

/// Simple HTTP server to handle OAuth callback
pub struct OAuthCallbackServer {
    port: u16,
    result_tx: Option<oneshot::Sender<OAuthCallbackResult>>,
}

impl OAuthCallbackServer {
    /// Create a new OAuth callback server
    pub fn new(port: u16) -> Self {
        Self {
            port,
            result_tx: None,
        }
    }
    
    /// Start the server and wait for callback
    pub async fn wait_for_callback(&mut self) -> Result<OAuthCallbackResult> {
        let (tx, rx) = oneshot::channel();
        self.result_tx = Some(tx);
        
        // Start a simple HTTP server using tokio
        let port = self.port;
        let result_tx = Arc::new(Mutex::new(self.result_tx.take()));
        
        let server = tokio::spawn(async move {
            Self::run_server(port, result_tx).await
        });
        
        // Wait for the callback
        let result = rx.await.context("Failed to receive callback")?;
        
        // Server will shut down after sending result
        server.abort();
        
        Ok(result)
    }
    
    /// Run the HTTP server
    async fn run_server(
        port: u16,
        result_tx: Arc<Mutex<Option<oneshot::Sender<OAuthCallbackResult>>>>,
    ) -> Result<()> {
        use tokio::net::TcpListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .context("Failed to bind to port")?;
        
        println!("✓ OAuth callback server listening on http://127.0.0.1:{}", port);
        
        loop {
            let (mut socket, _) = listener.accept().await?;
            let result_tx_clone = result_tx.clone();
            
            tokio::spawn(async move {
                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                
                if n > 0 {
                    let request = String::from_utf8_lossy(&buffer[..n]);
                    
                    // Parse the request line
                    if let Some(first_line) = request.lines().next() {
                        if let Some(path) = first_line.split_whitespace().nth(1) {
                            // Check if this is the callback
                            if path.starts_with("/callback") || path.starts_with("/?") {
                                // Parse query parameters
                                if let Some(query) = path.split('?').nth(1) {
                                    let mut code = None;
                                    let mut state = None;
                                    
                                    for param in query.split('&') {
                                        let parts: Vec<&str> = param.split('=').collect();
                                        if parts.len() == 2 {
                                            match parts[0] {
                                                "code" => code = Some(urlencoding::decode(parts[1]).unwrap_or_default().to_string()),
                                                "state" => state = Some(urlencoding::decode(parts[1]).unwrap_or_default().to_string()),
                                                _ => {}
                                            }
                                        }
                                    }
                                    
                                    if let (Some(code), Some(state)) = (code, state) {
                                        // Send success response
                                        let response = "HTTP/1.1 200 OK\r\n\
                                            Content-Type: text/html\r\n\
                                            Connection: close\r\n\
                                            \r\n\
                                            <html><body><h1>✓ Authentication Successful</h1>\
                                            <p>You can close this window and return to the terminal.</p>\
                                            </body></html>";
                                        
                                        let _ = socket.write_all(response.as_bytes()).await;
                                        
                                        // Send result
                                        if let Ok(mut tx) = result_tx_clone.lock() {
                                            if let Some(sender) = tx.take() {
                                                let _ = sender.send(OAuthCallbackResult { code, state });
                                            }
                                        }
                                        return;
                                    }
                                }
                                
                                // Send error response
                                let response = "HTTP/1.1 400 Bad Request\r\n\
                                    Content-Type: text/html\r\n\
                                    Connection: close\r\n\
                                    \r\n\
                                    <html><body><h1>❌ Authentication Failed</h1>\
                                    <p>Missing code or state parameter.</p>\
                                    </body></html>";
                                
                                let _ = socket.write_all(response.as_bytes()).await;
                            }
                        }
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_callback_server() {
        let server = OAuthCallbackServer::new(8080);
        assert_eq!(server.port, 8080);
    }
}
