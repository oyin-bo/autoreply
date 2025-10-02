//! Authentication module for BlueSky AT Protocol
//!
//! Provides authentication functionality including:
//! - App password authentication via com.atproto.server.createSession
//! - OAuth 2.0 with PKCE and DPoP (browser-based and device flows)
//! - Credential storage with OS keyring and file fallback
//! - Token refresh and lifecycle management
//! - Multi-account support

pub mod credentials;
pub mod session;
pub mod storage;
pub mod oauth;
pub mod oauth_atproto;
pub mod callback_server;

pub use credentials::Credentials;
pub use session::{Session, SessionManager};
pub use storage::{CredentialStorage, StorageBackend};
pub use oauth::{OAuthManager, OAuthConfig};
pub use oauth_atproto::{AtProtoOAuthManager, AtProtoOAuthConfig, BrowserFlowState};
pub use callback_server::{CallbackServer, CallbackResult};

use crate::error::AppError;

/// Service URL for BlueSky
pub const DEFAULT_SERVICE: &str = "https://bsky.social";

/// Errors specific to authentication
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("No credentials found for account: {0}")]
    NoCredentials(String),
    
    // Token refresh functionality - will be used when OAuth is enabled
    #[allow(dead_code)]
    #[error("Token expired")]
    TokenExpired,
    
    #[allow(dead_code)]
    #[error("Failed to refresh token: {0}")]
    RefreshFailed(String),
    
    #[allow(dead_code)]
    #[error("Invalid session data: {0}")]
    InvalidSession(String),
}

impl From<AuthError> for AppError {
    fn from(err: AuthError) -> Self {
        AppError::Authentication(err.to_string())
    }
}
