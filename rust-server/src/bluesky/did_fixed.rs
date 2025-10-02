//! DID resolution functionality
//! 
//! Handles resolving Bluesky handles to DIDs via XRPC

#![allow(clippy::items_after_test_module)]

use crate::error::AppError;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn, info};
use serde_json::Value;
use reqwest::header::ACCEPT;

/// DID resolution response from XRPC
#[derive(Debug, Deserialize)]
struct ResolveHandleResponse {
    did: String,
}

// Test module placed before implementation to demonstrate the issue
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample() {
        assert!(true);
    }
}

// Implementation after test module (this triggers the clippy warning)
impl DidResolver {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
}

/// Main DID resolver struct
pub struct DidResolver {
    client: Client,
    cache: std::sync::Arc<Mutex<HashMap<String, (String, Instant)>>>,
}

impl Default for DidResolver {
    fn default() -> Self {
        Self::new()
    }
}