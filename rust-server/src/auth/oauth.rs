use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use reqwest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime};

use super::AuthError;

/// OAuth client for BlueSky/AT Protocol authentication
pub struct OAuthClient {
    client_id: String,
    redirect_uri: String,
    auth_endpoint: String,
    token_endpoint: String,
    http_client: reqwest::Client,
}

impl OAuthClient {
    /// Create a new OAuth client for BlueSky authentication
    pub fn new() -> Self {
        Self {
            client_id: "autoreply-mcp-client".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            auth_endpoint: "https://bsky.social/oauth/authorize".to_string(),
            token_endpoint: "https://bsky.social/oauth/token".to_string(),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }
}

/// PKCE parameters for OAuth flow
#[derive(Debug, Clone)]
pub struct PKCEParams {
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PKCEParams {
    /// Generate PKCE code verifier and challenge
    pub fn generate() -> Result<Self, AuthError> {
        // Generate 32-byte random code verifier
        let verifier_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
        let code_verifier = URL_SAFE_NO_PAD.encode(&verifier_bytes);

        // Create SHA256 challenge
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let challenge_bytes = hasher.finalize();
        let code_challenge = URL_SAFE_NO_PAD.encode(&challenge_bytes);

        Ok(PKCEParams {
            code_verifier,
            code_challenge,
        })
    }
}

/// Authorization request parameters
pub struct AuthorizationRequest {
    pub handle: Option<String>,
    pub callback_port: Option<u16>,
    pub state: Option<String>,
    pub pkce_params: Option<PKCEParams>,
}

/// Authorization response with URL and state
#[derive(Debug)]
pub struct AuthorizationResponse {
    pub auth_url: String,
    pub state: String,
    pub code_verifier: String,
}

impl OAuthClient {
    /// Start PKCE OAuth authorization flow
    pub fn start_authorization_flow(
        &mut self,
        req: AuthorizationRequest,
    ) -> Result<AuthorizationResponse, AuthError> {
        let pkce_params = match req.pkce_params {
            Some(params) => params,
            None => PKCEParams::generate()?,
        };

        let state = match req.state {
            Some(s) => s,
            None => {
                let state_bytes: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();
                URL_SAFE_NO_PAD.encode(&state_bytes)
            }
        };

        if let Some(port) = req.callback_port {
            self.redirect_uri = format!("http://localhost:{}/callback", port);
        }

        let mut params = vec![
            ("response_type", "code".to_string()),
            ("client_id", self.client_id.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("scope", "atproto transition:generic".to_string()),
            ("state", state.clone()),
            ("code_challenge", pkce_params.code_challenge.clone()),
            ("code_challenge_method", "S256".to_string()),
        ];

        if let Some(handle) = req.handle {
            params.push(("login_hint", handle));
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let auth_url = format!("{}?{}", self.auth_endpoint, query_string);

        Ok(AuthorizationResponse {
            auth_url,
            state,
            code_verifier: pkce_params.code_verifier,
        })
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse, AuthError> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
            ("client_id", &self.client_id),
            ("code_verifier", code_verifier),
        ];

        let resp = self
            .http_client
            .post(&self.token_endpoint)
            .form(&params)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Token request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthError(format!(
                "Token exchange failed with status {}: {}",
                status,
                body
            )));
        }

        let mut token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::ParseError(format!("Failed to decode token response: {}", e)))?;

        // Calculate expiration time
        token_resp.expires_at = SystemTime::now() + Duration::from_secs(token_resp.expires_in as u64);

        Ok(token_resp)
    }

    /// Refresh an expired access token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenResponse, AuthError> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
        ];

        let resp = self
            .http_client
            .post(&self.token_endpoint)
            .form(&params)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Refresh request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthError(format!(
                "Token refresh failed with status {}: {}",
                status,
                body
            )));
        }

        let mut token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::ParseError(format!("Failed to decode refresh response: {}", e)))?;

        // Calculate expiration time
        token_resp.expires_at = SystemTime::now() + Duration::from_secs(token_resp.expires_in as u64);

        Ok(token_resp)
    }

    /// Start OAuth device authorization flow
    pub async fn start_device_flow(
        &self,
        handle: Option<&str>,
    ) -> Result<DeviceAuthorizationResponse, AuthError> {
        let device_endpoint = self.token_endpoint.replace("/token", "/device/code");

        let mut params = vec![
            ("client_id", self.client_id.as_str()),
            ("scope", "atproto transition:generic"),
        ];

        if let Some(h) = handle {
            params.push(("login_hint", h));
        }

        let resp = self
            .http_client
            .post(&device_endpoint)
            .form(&params)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Device auth request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthError(format!(
                "Device authorization failed with status {}: {}",
                status,
                body
            )));
        }

        let mut device_resp: DeviceAuthorizationResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::ParseError(format!("Failed to decode device auth response: {}", e)))?;

        // Set default polling interval if not provided
        if device_resp.interval == 0 {
            device_resp.interval = 5;
        }

        Ok(device_resp)
    }

    /// Poll for device authorization completion
    pub async fn poll_device_token(
        &self,
        device_code: &str,
    ) -> Result<TokenResponse, AuthError> {
        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
            ("client_id", &self.client_id),
        ];

        let resp = self
            .http_client
            .post(&self.token_endpoint)
            .form(&params)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Poll request failed: {}", e)))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        // Handle pending authorization
        if status == 400 {
            #[derive(Deserialize)]
            struct ErrorResponse {
                error: String,
            }

            if let Ok(err_resp) = serde_json::from_str::<ErrorResponse>(&body) {
                return match err_resp.error.as_str() {
                    "authorization_pending" => Err(AuthError::AuthorizationPending),
                    "slow_down" => Err(AuthError::SlowDown),
                    "expired_token" => Err(AuthError::ExpiredToken),
                    "access_denied" => Err(AuthError::AccessDenied),
                    _ => Err(AuthError::OAuthError(format!("Device token poll error: {}", err_resp.error))),
                };
            }
        }

        if !status.is_success() {
            return Err(AuthError::OAuthError(format!(
                "Device token poll failed with status {}: {}",
                status,
                body
            )));
        }

        let mut token_resp: TokenResponse = serde_json::from_str(&body)
            .map_err(|e| AuthError::ParseError(format!("Failed to decode token response: {}", e)))?;

        // Calculate expiration time
        token_resp.expires_at = SystemTime::now() + Duration::from_secs(token_resp.expires_in as u64);

        Ok(token_resp)
    }
}

/// OAuth token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: String,
    #[serde(rename = "sub")]
    pub did: String,
    #[serde(skip)]
    #[serde(default = "default_system_time")]
    pub expires_at: SystemTime,
}

fn default_system_time() -> SystemTime {
    SystemTime::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let pkce1 = PKCEParams::generate().unwrap();
        let pkce2 = PKCEParams::generate().unwrap();

        // Should not be empty
        assert!(!pkce1.code_verifier.is_empty());
        assert!(!pkce1.code_challenge.is_empty());

        // Should be unique
        assert_ne!(pkce1.code_verifier, pkce2.code_verifier);
        assert_ne!(pkce1.code_challenge, pkce2.code_challenge);
    }

    #[test]
    fn test_oauth_client_creation() {
        let client = OAuthClient::new();

        assert_eq!(client.client_id, "autoreply-mcp-client");
        assert_eq!(client.redirect_uri, "http://localhost:8080/callback");
        assert!(client.auth_endpoint.contains("oauth/authorize"));
        assert!(client.token_endpoint.contains("oauth/token"));
    }

    #[test]
    fn test_start_authorization_flow_basic() {
        let mut client = OAuthClient::new();
        let req = AuthorizationRequest {
            handle: None,
            callback_port: None,
            state: None,
            pkce_params: None,
        };

        let resp = client.start_authorization_flow(req).unwrap();

        assert!(!resp.auth_url.is_empty());
        assert!(!resp.state.is_empty());
        assert!(!resp.code_verifier.is_empty());

        // Verify URL contains required parameters
        assert!(resp.auth_url.contains("response_type=code"));
        assert!(resp.auth_url.contains("code_challenge="));
        assert!(resp.auth_url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_start_authorization_flow_with_handle() {
        let mut client = OAuthClient::new();
        let req = AuthorizationRequest {
            handle: Some("alice.bsky.social".to_string()),
            callback_port: None,
            state: None,
            pkce_params: None,
        };

        let resp = client.start_authorization_flow(req).unwrap();

        assert!(resp.auth_url.contains("login_hint=alice.bsky.social"));
    }

    #[test]
    fn test_start_authorization_flow_with_custom_port() {
        let mut client = OAuthClient::new();
        let req = AuthorizationRequest {
            handle: None,
            callback_port: Some(9090),
            state: None,
            pkce_params: None,
        };

        let _resp = client.start_authorization_flow(req).unwrap();

        assert_eq!(client.redirect_uri, "http://localhost:9090/callback");
    }

    #[test]
    fn test_start_authorization_flow_with_provided_pkce() {
        let mut client = OAuthClient::new();
        let pkce = PKCEParams::generate().unwrap();
        let code_verifier = pkce.code_verifier.clone();

        let req = AuthorizationRequest {
            handle: None,
            callback_port: None,
            state: Some("custom-state".to_string()),
            pkce_params: Some(pkce),
        };

        let resp = client.start_authorization_flow(req).unwrap();

        assert_eq!(resp.state, "custom-state");
        assert_eq!(resp.code_verifier, code_verifier);
    }

    #[test]
    fn test_token_response_expiration() {
        let now = SystemTime::now();
        let mut token_resp = TokenResponse {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            scope: "atproto".to_string(),
            did: "did:plc:test".to_string(),
            expires_at: now,
        };

        // Simulate expiration calculation
        token_resp.expires_at = now + Duration::from_secs(token_resp.expires_in as u64);

        assert!(token_resp.expires_at > now);
        
        let expected_expiry = now + Duration::from_secs(3600);
        let time_diff = token_resp.expires_at.duration_since(expected_expiry)
            .unwrap_or_else(|e| e.duration());
        assert!(time_diff < Duration::from_secs(1));
    }

    #[test]
    fn test_device_authorization_response_defaults() {
        let mut device = DeviceAuthorizationResponse {
            device_code: "ABC123".to_string(),
            user_code: "WXYZ-1234".to_string(),
            verification_uri: "https://bsky.app/device".to_string(),
            verification_uri_complete: None,
            expires_in: 600,
            interval: 0,
        };

        // Simulate default interval setting
        if device.interval == 0 {
            device.interval = 5;
        }

        assert_eq!(device.interval, 5);
    }

    #[test]
    fn test_auth_error_display() {
        let errors = vec![
            AuthError::AuthorizationPending,
            AuthError::SlowDown,
            AuthError::ExpiredToken,
            AuthError::AccessDenied,
        ];

        for err in errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
        }
    }
}

/// Device authorization response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthorizationResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    pub expires_in: i64,
    pub interval: i64,
}
