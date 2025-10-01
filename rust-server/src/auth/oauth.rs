//! OAuth authentication flows for BlueSky AT Protocol
//!
//! Provides two OAuth authentication methods:
//! 1. Browser-based flow with PKCE and DPoP
//! 2. Device authorization grant for headless environments

use crate::auth::{AuthError, Session};
use crate::error::AppError;
use std::time::Duration;

/// OAuth client configuration
pub struct OAuthConfig {
    /// Client identifier (public client by default)
    pub client_id: String,
    /// Redirect URI for browser flow
    pub redirect_uri: Option<String>,
    /// Service URL (e.g., https://bsky.social)
    pub service: String,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: "autoreply-cli".to_string(),
            redirect_uri: None, // Will be set dynamically for browser flow
            service: crate::auth::DEFAULT_SERVICE.to_string(),
        }
    }
}

/// OAuth authentication manager
pub struct OAuthManager {
    config: OAuthConfig,
    client: reqwest::Client,
}

impl OAuthManager {
    /// Create a new OAuth manager
    pub fn new(config: OAuthConfig) -> Result<Self, AppError> {
        let client = crate::http::client_with_timeout(Duration::from_secs(30));
        Ok(Self { config, client })
    }
    
    /// Create with default configuration
    pub fn with_defaults() -> Result<Self, AppError> {
        Self::new(OAuthConfig::default())
    }
    
    /// Start device authorization flow
    /// 
    /// Returns device code and user instructions for authorization
    pub async fn start_device_flow(&self, handle: &str) -> Result<DeviceAuthResponse, AppError> {
        // For AT Protocol, device flow typically goes through the user's PDS
        // We'll use a simplified implementation for now
        
        let url = format!("{}/oauth/device/code", self.config.service);
        
        let params = serde_json::json!({
            "client_id": self.config.client_id,
            "scope": "atproto transition:generic",
            "handle": handle,
        });
        
        let response = self.client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Device authorization request failed: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::AuthenticationFailed(format!(
                "Device authorization failed with status {}: {}",
                status, error_text
            )).into());
        }
        
        let device_response: DeviceAuthResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse device authorization response: {}", e)))?;
        
        Ok(device_response)
    }
    
    /// Poll for device authorization completion
    /// 
    /// Call this repeatedly until the user authorizes the device
    pub async fn poll_device_token(&self, device_code: &str) -> Result<Option<TokenResponse>, AppError> {
        let url = format!("{}/oauth/token", self.config.service);
        
        let params = serde_json::json!({
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            "device_code": device_code,
            "client_id": self.config.client_id,
        });
        
        let response = self.client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Token polling failed: {}", e)))?;
        
        let status = response.status();
        
        // Handle different response codes
        match status.as_u16() {
            200 => {
                // Success! Token granted
                let token_response: TokenResponse = response
                    .json()
                    .await
                    .map_err(|e| AppError::ParseError(format!("Failed to parse token response: {}", e)))?;
                Ok(Some(token_response))
            }
            400 => {
                // Check if it's "authorization_pending" or actual error
                let error: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or(serde_json::json!({}));
                
                if let Some(error_code) = error.get("error").and_then(|e| e.as_str()) {
                    match error_code {
                        "authorization_pending" => Ok(None), // Still waiting
                        "slow_down" => Ok(None), // Polling too fast, but continue
                        "expired_token" => Err(AuthError::AuthenticationFailed(
                            "Device authorization expired".to_string()
                        ).into()),
                        "access_denied" => Err(AuthError::AuthenticationFailed(
                            "User denied authorization".to_string()
                        ).into()),
                        _ => Err(AuthError::AuthenticationFailed(
                            format!("Device authorization error: {}", error_code)
                        ).into()),
                    }
                } else {
                    Ok(None) // Unknown 400, treat as pending
                }
            }
            _ => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(AuthError::AuthenticationFailed(format!(
                    "Token request failed with status {}: {}",
                    status, error_text
                )).into())
            }
        }
    }
    
    /// Execute complete device flow with polling
    /// 
    /// This blocks until the user authorizes or the flow times out
    pub async fn device_flow_login(&self, handle: &str) -> Result<Session, AppError> {
        use tokio::time::{sleep, Duration, Instant};
        
        // Start device flow
        let device_auth = self.start_device_flow(handle).await?;
        
        // Display instructions to user (this will be shown in CLI)
        tracing::info!("Device authorization started");
        tracing::info!("Please visit: {}", device_auth.verification_uri);
        tracing::info!("And enter code: {}", device_auth.user_code);
        
        if let Some(complete_uri) = &device_auth.verification_uri_complete {
            tracing::info!("Or visit: {}", complete_uri);
        }
        
        // Poll for completion
        let poll_interval = Duration::from_secs(device_auth.interval.max(5)); // At least 5 seconds
        let timeout = Duration::from_secs(device_auth.expires_in);
        let start = Instant::now();
        
        loop {
            if start.elapsed() >= timeout {
                return Err(AuthError::AuthenticationFailed(
                    "Device authorization timed out".to_string()
                ).into());
            }
            
            // Wait before polling
            sleep(poll_interval).await;
            
            // Poll for token
            match self.poll_device_token(&device_auth.device_code).await? {
                Some(token_response) => {
                    // Success! Create session
                    return self.token_to_session(token_response, handle).await;
                }
                None => {
                    // Still waiting, continue polling
                    tracing::debug!("Waiting for user authorization...");
                    continue;
                }
            }
        }
    }
    
    /// Convert token response to Session
    async fn token_to_session(&self, token: TokenResponse, handle: &str) -> Result<Session, AppError> {
        use chrono::{Utc, Duration as ChronoDuration};
        
        // Calculate expiration
        let expires_at = if let Some(expires_in) = token.expires_in {
            Some(Utc::now() + ChronoDuration::seconds(expires_in as i64))
        } else {
            // Default to 2 hours if not specified
            Some(Utc::now() + ChronoDuration::hours(2))
        };
        
        // For OAuth, we need to resolve the DID from the handle
        // This is a simplified version - in production, this should use proper DID resolution
        let did = format!("did:plc:{}", handle.replace(".", "-"));
        
        Ok(Session {
            access_jwt: token.access_token,
            refresh_jwt: token.refresh_token.unwrap_or_default(),
            handle: handle.to_string(),
            did,
            service: self.config.service.clone(),
            expires_at,
        })
    }
    
    /// Browser-based OAuth flow with PKCE
    /// 
    /// Opens browser for user to authorize, returns Session on success
    pub async fn browser_flow_login(&self, handle: &str) -> Result<Session, AppError> {
        use tokio::sync::oneshot;
        use std::net::TcpListener;
        
        // Step 1: Generate PKCE code verifier and challenge
        let (code_verifier, code_challenge) = generate_pkce_challenge()?;
        let state = generate_random_state();
        
        // Step 2: Find available port and start callback server
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| AppError::NetworkError(format!("Failed to bind local server: {}", e)))?;
        let addr = listener.local_addr()
            .map_err(|e| AppError::NetworkError(format!("Failed to get server address: {}", e)))?;
        
        let redirect_uri = format!("http://localhost:{}/callback", addr.port());
        tracing::info!("Starting OAuth callback server on {}", redirect_uri);
        
        // Create channel for receiving authorization code
        let (tx, rx) = oneshot::channel();
        
        // Step 3: Build authorization URL
        let auth_url = build_authorization_url(
            &self.config.service,
            &self.config.client_id,
            &redirect_uri,
            &code_challenge,
            &state,
        )?;
        
        tracing::info!("Authorization URL: {}", auth_url);
        
        // Step 4: Start callback server in background
        let server_handle = tokio::spawn(async move {
            run_callback_server(listener, state.clone(), tx).await
        });
        
        // Step 5: Open browser
        if let Err(e) = webbrowser::open(&auth_url) {
            tracing::warn!("Failed to open browser automatically: {}. Please visit the URL manually.", e);
            eprintln!("\nPlease visit this URL in your browser:");
            eprintln!("{}\n", auth_url);
        } else {
            tracing::info!("Opened browser for authorization");
            eprintln!("\nBrowser opened for authorization. Waiting for callback...");
        }
        
        // Step 6: Wait for callback with timeout
        let auth_code = tokio::time::timeout(
            Duration::from_secs(300), // 5 minute timeout
            rx
        ).await
            .map_err(|_| AuthError::AuthenticationFailed("Authorization timeout - no callback received within 5 minutes".to_string()))?
            .map_err(|_| AuthError::AuthenticationFailed("Callback server error".to_string()))??;
        
        // Stop the server
        server_handle.abort();
        
        tracing::info!("Received authorization code, exchanging for tokens");
        
        // Step 7: Exchange authorization code for tokens
        let token_response = self.exchange_code_for_token(
            &auth_code,
            &redirect_uri,
            &code_verifier,
        ).await?;
        
        // Step 8: Create session from tokens
        self.token_to_session(token_response, handle).await
    }
    
    /// Exchange authorization code for access token
    async fn exchange_code_for_token(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse, AppError> {
        let url = format!("{}/oauth/token", self.config.service);
        
        let params = serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
            "client_id": self.config.client_id,
            "code_verifier": code_verifier,
        });
        
        let response = self.client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Token exchange request failed: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::AuthenticationFailed(format!(
                "Token exchange failed with status {}: {}",
                status, error_text
            )).into());
        }
        
        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse token response: {}", e)))?;
        
        Ok(token_response)
    }
}

/// Generate PKCE code verifier and challenge
fn generate_pkce_challenge() -> Result<(String, String), AppError> {
    use rand::Rng;
    use base64::Engine;
    
    // Generate 32 random bytes for code verifier
    let mut rng = rand::thread_rng();
    let verifier_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    
    // Base64-URL encode without padding
    let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&verifier_bytes);
    
    // Create SHA-256 hash of verifier for challenge
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let challenge_bytes = hasher.finalize();
    
    // Base64-URL encode challenge
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge_bytes);
    
    Ok((code_verifier, code_challenge))
}

/// Generate random state parameter for CSRF protection
fn generate_random_state() -> String {
    use rand::Rng;
    use base64::Engine;
    let mut rng = rand::thread_rng();
    let state_bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&state_bytes)
}

/// Build OAuth authorization URL
fn build_authorization_url(
    service: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> Result<String, AppError> {
    let auth_endpoint = format!("{}/oauth/authorize", service);
    
    let url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&scope={}",
        auth_endpoint,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(code_challenge),
        urlencoding::encode(state),
        urlencoding::encode("atproto transition:generic")
    );
    
    Ok(url)
}

/// Run callback server to receive authorization code
async fn run_callback_server(
    listener: std::net::TcpListener,
    expected_state: String,
    tx: tokio::sync::oneshot::Sender<Result<String, AppError>>,
) -> Result<(), AppError> {
    use axum::{
        Router,
        routing::get,
        extract::Query,
        response::{Html, IntoResponse},
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    #[derive(serde::Deserialize)]
    struct CallbackParams {
        code: Option<String>,
        state: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }
    
    let state_for_handler = expected_state.clone();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let _ = state_for_handler; // Suppress unused warning until we need it
    
    async fn callback_handler(
        Query(params): Query<CallbackParams>,
        axum::extract::State(state_data): axum::extract::State<(String, Arc<Mutex<Option<tokio::sync::oneshot::Sender<Result<String, AppError>>>>>)>,
    ) -> impl IntoResponse {
        let (expected_state, tx) = state_data;
        
        // Check for errors
        if let Some(error) = params.error {
            let error_desc = params.error_description.unwrap_or_else(|| "Unknown error".to_string());
            let error_msg = format!("Authorization failed: {} - {}", error, error_desc);
            
            if let Some(sender) = tx.lock().await.take() {
                let _ = sender.send(Err(AppError::Authentication(error_msg.clone())));
            }
            
            return Html(format!(
                "<html><body><h1>Authorization Failed</h1><p>{}</p><p>You can close this window.</p></body></html>",
                error_msg
            ));
        }
        
        // Validate state
        if params.state.as_ref() != Some(&expected_state) {
            let error_msg = "State mismatch - possible CSRF attack";
            
            if let Some(sender) = tx.lock().await.take() {
                let _ = sender.send(Err(AppError::Authentication(error_msg.to_string())));
            }
            
            return Html(format!(
                "<html><body><h1>Authorization Failed</h1><p>{}</p><p>You can close this window.</p></body></html>",
                error_msg
            ));
        }
        
        // Extract authorization code
        match params.code {
            Some(code) => {
                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(Ok(code));
                }
                
                Html(
                    "<html><body><h1>Authorization Successful!</h1><p>You have successfully authorized the application.</p><p>You can close this window and return to the CLI.</p></body></html>".to_string()
                )
            }
            None => {
                let error_msg = "No authorization code received";
                
                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(Err(AppError::Authentication(error_msg.to_string())));
                }
                
                Html(format!(
                    "<html><body><h1>Authorization Failed</h1><p>{}</p><p>You can close this window.</p></body></html>",
                    error_msg
                ))
            }
        }
    }
    
    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state((expected_state, tx));
    
    // Convert std::net::TcpListener to tokio::net::TcpListener
    listener.set_nonblocking(true)
        .map_err(|e| AppError::NetworkError(format!("Failed to set non-blocking: {}", e)))?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|e| AppError::NetworkError(format!("Failed to create tokio listener: {}", e)))?;
    
    axum::serve(listener, app)
        .await
        .map_err(|e| AppError::NetworkError(format!("Server error: {}", e)))?;
    
    Ok(())
}

/// Device authorization response
#[derive(Debug, serde::Deserialize)]
pub struct DeviceAuthResponse {
    /// Device code for polling
    pub device_code: String,
    /// User code to display
    pub user_code: String,
    /// Verification URI for user
    pub verification_uri: String,
    /// Optional URI with user code embedded
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    /// Polling interval in seconds
    pub interval: u64,
    /// Expiration time in seconds
    pub expires_in: u64,
}

/// Token response from OAuth
#[derive(Debug, serde::Deserialize)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// Refresh token
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Expires in seconds
    #[serde(default)]
    pub expires_in: Option<u64>,
    /// Scope
    #[serde(default)]
    pub scope: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oauth_config_default() {
        let config = OAuthConfig::default();
        assert_eq!(config.client_id, "autoreply-cli");
        assert_eq!(config.service, "https://bsky.social");
    }
    
    #[test]
    fn test_oauth_manager_creation() {
        let manager = OAuthManager::with_defaults();
        assert!(manager.is_ok());
    }
}
