//! Session management for authenticated BlueSky sessions

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use crate::error::AppError;
use crate::auth::{AuthError, Credentials};

/// Session data from com.atproto.server.createSession
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Access JWT token
    pub access_jwt: String,
    
    /// Refresh JWT token
    pub refresh_jwt: String,
    
    /// User's handle
    pub handle: String,
    
    /// User's DID
    pub did: String,
    
    /// Service URL
    #[serde(default = "default_service")]
    pub service: String,
    
    /// Token expiration time (calculated from creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

fn default_service() -> String {
    crate::auth::DEFAULT_SERVICE.to_string()
}

impl Session {
    /// Check if the access token is expired or will expire soon (within 5 minutes)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            // Consider expired if within 5 minutes of expiry
            let now = Utc::now();
            now >= expires_at - Duration::minutes(5)
        } else {
            // If no expiry time set, assume 2 hour lifetime from AT Protocol spec
            false
        }
    }
}

/// Response from com.atproto.server.createSession
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionResponse {
    access_jwt: String,
    refresh_jwt: String,
    handle: String,
    did: String,
    #[serde(default)]
    did_doc: Option<serde_json::Value>,
}

/// Response from com.atproto.server.refreshSession
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshSessionResponse {
    access_jwt: String,
    refresh_jwt: String,
    handle: String,
    did: String,
    #[serde(default)]
    did_doc: Option<serde_json::Value>,
}

/// Manages authenticated sessions
pub struct SessionManager {
    client: reqwest::Client,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Result<Self, AppError> {
        let client = crate::http::client_with_timeout(std::time::Duration::from_secs(30));
        Ok(Self { client })
    }
    
    /// Authenticate using app password and create a new session
    pub async fn login(&self, credentials: &Credentials) -> Result<Session, AppError> {
        let url = format!("{}/xrpc/com.atproto.server.createSession", credentials.service);
        
        let body = serde_json::json!({
            "identifier": credentials.identifier,
            "password": credentials.password,
        });
        
        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Login request failed: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::AuthenticationFailed(format!(
                "Login failed with status {}: {}",
                status, error_text
            )).into());
        }
        
        let session_response: CreateSessionResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse session response: {}", e)))?;
        
        // Calculate expiration (2 hours from now as per AT Protocol spec)
        let expires_at = Utc::now() + Duration::hours(2);
        
        Ok(Session {
            access_jwt: session_response.access_jwt,
            refresh_jwt: session_response.refresh_jwt,
            handle: session_response.handle,
            did: session_response.did,
            service: credentials.service.clone(),
            expires_at: Some(expires_at),
        })
    }
    
    /// Refresh an existing session using the refresh token
    pub async fn refresh(&self, session: &Session) -> Result<Session, AppError> {
        let url = format!("{}/xrpc/com.atproto.server.refreshSession", session.service);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", session.refresh_jwt))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Refresh request failed: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::RefreshFailed(format!(
                "Token refresh failed with status {}: {}",
                status, error_text
            )).into());
        }
        
        let refresh_response: RefreshSessionResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse refresh response: {}", e)))?;
        
        // Calculate new expiration
        let expires_at = Utc::now() + Duration::hours(2);
        
        Ok(Session {
            access_jwt: refresh_response.access_jwt,
            refresh_jwt: refresh_response.refresh_jwt,
            handle: refresh_response.handle,
            did: refresh_response.did,
            service: session.service.clone(),
            expires_at: Some(expires_at),
        })
    }
    
    /// Get a valid session, refreshing if necessary
    pub async fn get_valid_session(&self, session: &Session) -> Result<Session, AppError> {
        if session.is_expired() {
            self.refresh(session).await
        } else {
            Ok(session.clone())
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new().expect("Failed to create SessionManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_not_expired_without_expiry() {
        let session = Session {
            access_jwt: "token".to_string(),
            refresh_jwt: "refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: None,
        };
        
        assert!(!session.is_expired());
    }
    
    #[test]
    fn test_session_expired() {
        let session = Session {
            access_jwt: "token".to_string(),
            refresh_jwt: "refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() - Duration::hours(1)),
        };
        
        assert!(session.is_expired());
    }
    
    #[test]
    fn test_session_not_expired() {
        let session = Session {
            access_jwt: "token".to_string(),
            refresh_jwt: "refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
        };
        
        assert!(!session.is_expired());
    }
    
    #[test]
    fn test_session_expiring_soon() {
        let session = Session {
            access_jwt: "token".to_string(),
            refresh_jwt: "refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() + Duration::minutes(3)),
        };
        
        // Should be considered expired if within 5 minutes
        assert!(session.is_expired());
    }
    
    #[test]
    fn test_session_serialization() {
        let session = Session {
            access_jwt: "access".to_string(),
            refresh_jwt: "refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now()),
        };
        
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session.handle, deserialized.handle);
        assert_eq!(session.did, deserialized.did);
    }
}
