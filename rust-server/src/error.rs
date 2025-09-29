//! Error types and handling for the Bluesky MCP server

use anyhow::Result;
use serde::Serialize;
use std::fmt;

/// Application error types as specified in docs/7.1-rust.md
#[derive(Debug, Serialize)]
pub enum AppError {
    InvalidInput(String),
    DidResolveFailed(String),
    RepoFetchFailed(String),
    RepoParseFailed(String),
    NotFound(String),
    Timeout(String),
    CacheError(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::DidResolveFailed(msg) => write!(f, "DID resolution failed: {}", msg),
            AppError::RepoFetchFailed(msg) => write!(f, "Repository fetch failed: {}", msg),
            AppError::RepoParseFailed(msg) => write!(f, "Repository parse failed: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            AppError::CacheError(msg) => write!(f, "Cache error: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl AppError {
    /// Get the error code for MCP responses
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::InvalidInput(_) => "invalid_input",
            AppError::DidResolveFailed(_) => "did_resolve_failed",
            AppError::RepoFetchFailed(_) => "repo_fetch_failed",
            AppError::RepoParseFailed(_) => "repo_parse_failed",
            AppError::NotFound(_) => "not_found",
            AppError::Timeout(_) => "timeout",
            AppError::CacheError(_) => "cache_error",
            AppError::Internal(_) => "internal_error",
        }
    }

    /// Get the error message
    pub fn message(&self) -> String {
        self.to_string()
    }
}

/// Convert anyhow::Error to AppError
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

/// Convert reqwest::Error to AppError
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AppError::Timeout(err.to_string())
        } else if err.is_connect() || err.is_request() {
            AppError::RepoFetchFailed(err.to_string())
        } else {
            AppError::Internal(err.to_string())
        }
    }
}

/// Convert serde_json::Error to AppError
impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::RepoParseFailed(err.to_string())
    }
}

/// Convert std::io::Error to AppError
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::CacheError(err.to_string())
    }
}

/// Validation functions
pub fn validate_account(account: &str) -> Result<(), AppError> {
    if account.is_empty() {
        return Err(AppError::InvalidInput("Account cannot be empty".to_string()));
    }

    // Check if it's a DID
    if account.starts_with("did:plc:") {
        if account.len() != 32 || !account[8..].chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(AppError::InvalidInput("Invalid DID format".to_string()));
        }
        return Ok(());
    }
    if account.starts_with("did:web:") {
        // Basic structural validation for did:web
        // did:web:<host>[:<path segments separated by ':'>]
        let rest = &account[8..];
        if rest.is_empty() {
            return Err(AppError::InvalidInput("Invalid did:web format".to_string()));
        }
        // Do not over-validate here; downstream resolution will attempt to fetch the DID document
        return Ok(());
    }

    // Check if it's a handle
    if !account.contains('.') {
        return Err(AppError::InvalidInput(
            "Invalid handle format, must contain domain".to_string(),
        ));
    }

    // Basic domain validation
    let parts: Vec<&str> = account.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(AppError::InvalidInput("Invalid handle format".to_string()));
    }

    Ok(())
}

pub fn validate_query(query: &str) -> Result<(), AppError> {
    if query.is_empty() {
        return Err(AppError::InvalidInput("Query cannot be empty".to_string()));
    }

    if query.len() > 500 {
        return Err(AppError::InvalidInput(
            "Query too long, maximum 500 characters".to_string(),
        ));
    }

    Ok(())
}

/// Normalize text using Unicode NFKC as specified
pub fn normalize_text(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfkc().collect::<String>().trim().to_string()
}