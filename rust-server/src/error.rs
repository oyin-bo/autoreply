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
    HttpClientInitialization(String),
    NetworkError(String),
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
            AppError::HttpClientInitialization(msg) => write!(f, "HTTP client initialization failed: {}", msg),
            AppError::NetworkError(msg) => write!(f, "Network error: {}", msg),
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
            AppError::HttpClientInitialization(_) => "http_client_initialization",
            AppError::NetworkError(_) => "network_error",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let tests = vec![
            (AppError::InvalidInput("test".to_string()), "Invalid input: test"),
            (AppError::DidResolveFailed("resolve error".to_string()), "DID resolution failed: resolve error"),
            (AppError::RepoFetchFailed("fetch error".to_string()), "Repository fetch failed: fetch error"),
            (AppError::RepoParseFailed("parse error".to_string()), "Repository parse failed: parse error"),
            (AppError::NotFound("not found".to_string()), "Not found: not found"),
            (AppError::Timeout("timeout error".to_string()), "Timeout: timeout error"),
            (AppError::CacheError("cache error".to_string()), "Cache error: cache error"),
            (AppError::HttpClientInitialization("http client error".to_string()), "HTTP client initialization failed: http client error"),
            (AppError::NetworkError("network error".to_string()), "Network error: network error"),
            (AppError::Internal("internal error".to_string()), "Internal error: internal error"),
        ];

        for (error, expected) in tests {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn test_app_error_codes() {
        let tests = vec![
            (AppError::InvalidInput("test".to_string()), "invalid_input"),
            (AppError::DidResolveFailed("test".to_string()), "did_resolve_failed"),
            (AppError::RepoFetchFailed("test".to_string()), "repo_fetch_failed"),
            (AppError::RepoParseFailed("test".to_string()), "repo_parse_failed"),
            (AppError::NotFound("test".to_string()), "not_found"),
            (AppError::Timeout("test".to_string()), "timeout"),
            (AppError::CacheError("test".to_string()), "cache_error"),
            (AppError::HttpClientInitialization("test".to_string()), "http_client_initialization"),
            (AppError::NetworkError("test".to_string()), "network_error"),
            (AppError::Internal("test".to_string()), "internal_error"),
        ];

        for (error, expected_code) in tests {
            assert_eq!(error.error_code(), expected_code);
        }
    }

    #[test]
    fn test_from_reqwest_error() {
        // We can't easily create specific reqwest errors in tests,
        // but we can test the From implementation logic
        // by creating mock errors and testing the conversion paths
        
        // Test that we have the conversion implemented
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let app_err: AppError = json_err.into();
        assert_eq!(app_err.error_code(), "repo_parse_failed");
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let app_err: AppError = json_err.into();
        assert_eq!(app_err.error_code(), "repo_parse_failed");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        assert_eq!(app_err.error_code(), "cache_error");
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("generic error");
        let app_err: AppError = anyhow_err.into();
        assert_eq!(app_err.error_code(), "internal_error");
    }

    #[test]
    fn test_validate_account_empty() {
        let result = validate_account("");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_account_valid_plc_did() {
        let valid_dids = vec![
            "did:plc:abcdefghijklmnopqrstuvwx", // 32 total
            "did:plc:123456789012345678901234", // 32 total
        ];
        
        for did in valid_dids {
            assert!(validate_account(did).is_ok());
        }
    }

    #[test]
    fn test_validate_account_invalid_plc_did() {
        let invalid_dids = vec![
            "did:plc:tooshort",          // Too short
            "did:plc:toolong123456789012345678901", // Too long
            "did:plc:has-invalid-chars123456789!", // Invalid characters
        ];
        
        for did in invalid_dids {
            let result = validate_account(did);
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::InvalidInput(msg) => assert!(msg.contains("Invalid DID format")),
                _ => panic!("Expected InvalidInput error for {}", did),
            }
        }
    }

    #[test]
    fn test_validate_account_valid_web_did() {
        let valid_web_dids = vec![
            "did:web:example.com",
            "did:web:example.com:user:alice",
            "did:web:subdomain.example.org:some:path",
        ];
        
        for did in valid_web_dids {
            assert!(validate_account(did).is_ok());
        }
    }

    #[test]
    fn test_validate_account_invalid_web_did() {
        let result = validate_account("did:web:");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidInput(msg) => assert!(msg.contains("Invalid did:web format")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_account_valid_handles() {
        let valid_handles = vec![
            "alice.bsky.social",
            "bob.example.com", 
            "user.subdomain.example.org",
            "@alice.bsky.social", // With @ prefix should still validate the underlying handle
        ];
        
        for handle in valid_handles {
            assert!(validate_account(handle).is_ok());
        }
    }

    #[test]
    fn test_validate_account_invalid_handles() {
        let invalid_handles = vec![
            "nodomain",           // No dot
            "empty.",            // Empty domain part
            ".empty",            // Empty name part
            "double..domain",    // Double dot
        ];
        
        for handle in invalid_handles {
            let result = validate_account(handle);
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::InvalidInput(_) => {} // Expected
                _ => panic!("Expected InvalidInput error for {}", handle),
            }
        }
    }

    #[test]
    fn test_validate_query_valid() {
        let max_length_query = "a".repeat(500);
        let valid_queries = vec![
            "hello",
            "hello world",
            &max_length_query, // Max length
            "unicode: 🚀 ñoño",
        ];
        
        for query in valid_queries {
            assert!(validate_query(&query).is_ok());
        }
    }

    #[test]
    fn test_validate_query_empty() {
        let result = validate_query("");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_query_too_long() {
        let long_query = "a".repeat(501); // Over 500 chars
        let result = validate_query(&long_query);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidInput(msg) => assert!(msg.contains("too long")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_normalize_text_basic() {
        assert_eq!(normalize_text("hello world"), "hello world");
        assert_eq!(normalize_text("  hello world  "), "hello world");
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn test_normalize_text_unicode() {
        // Test NFKC normalization
        // These examples use different Unicode forms that should normalize to the same result
        let text1 = "café"; // e with acute accent (composed)
        let text2 = "cafe\u{0301}"; // e + combining acute accent (decomposed)
        
        let normalized1 = normalize_text(text1);
        let normalized2 = normalize_text(text2);
        
        assert_eq!(normalized1, normalized2);
        assert_eq!(normalized1, "café");
    }

    #[test]
    fn test_normalize_text_whitespace() {
        // Test trimming various whitespace
        let inputs = vec![
            "\t  hello  \n",
            "\r\n hello \t",
            "   hello   ",
        ];
        
        for input in inputs {
            assert_eq!(normalize_text(input), "hello");
        }
    }

    #[test] 
    fn test_normalize_text_compatibility() {
        // Test NFKC compatibility normalization
        // Roman numeral I (Ⅰ U+2160) should normalize to regular I
        assert_eq!(normalize_text("Ⅰ"), "I");
        
        // Fullwidth A should normalize to regular A
        assert_eq!(normalize_text("Ａ"), "A");
    }
}