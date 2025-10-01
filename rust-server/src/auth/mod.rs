//! Authentication module for BlueSky AT Protocol
//!
//! Provides authentication functionality including:
//! - App password authentication via com.atproto.server.createSession
//! - Credential storage with OS keyring and file fallback
//! - Token refresh and lifecycle management
//! - Multi-account support

pub mod credentials;
pub mod session;
pub mod storage;

pub use credentials::Credentials;
pub use session::{Session, SessionManager};
pub use storage::{CredentialStorage, StorageBackend};

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
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Failed to refresh token: {0}")]
    RefreshFailed(String),
    
    #[error("Invalid session data: {0}")]
    InvalidSession(String),
}

impl From<AuthError> for AppError {
    fn from(err: AuthError) -> Self {
        AppError::Authentication(err.to_string())
    }
}
