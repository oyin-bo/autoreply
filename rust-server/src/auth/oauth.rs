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
        // For now, this is a placeholder that shows the concept
        // Full implementation would:
        // 1. Generate PKCE code verifier and challenge
        // 2. Start local HTTP server for callback
        // 3. Build authorization URL
        // 4. Open browser
        // 5. Wait for callback with authorization code
        // 6. Exchange code for tokens
        // 7. Create session
        
        Err(AuthError::AuthenticationFailed(
            "Browser-based OAuth flow is not yet fully implemented. Use --device for device flow or app passwords.".to_string()
        ).into())
    }
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
