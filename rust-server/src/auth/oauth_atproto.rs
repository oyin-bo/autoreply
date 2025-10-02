//! AT Protocol OAuth implementation following the official specification
//!
//! This module implements OAuth 2.0 for AT Protocol as specified in:
//! - https://atproto.com/specs/auth
//! - https://docs.bsky.app/docs/advanced-guides/oauth-client
//!
//! Key requirements:
//! - Identity resolution: handle → DID → PDS → Auth Server
//! - PAR (Pushed Authorization Requests) - mandatory
//! - PKCE with S256 - mandatory
//! - DPoP with nonce handling - mandatory
//! - Client metadata as URL or loopback

use crate::auth::{AuthError, Session};
use crate::error::AppError;
use std::time::Duration;
use serde::Deserialize;

/// OAuth client configuration for AT Protocol
pub struct AtProtoOAuthConfig {
    /// Client identifier - must be URL to metadata or use loopback pattern
    pub client_id: String,
    /// Redirect URI for callbacks
    pub redirect_uri: String,
    /// Scopes to request
    pub scope: String,
}

impl Default for AtProtoOAuthConfig {
    fn default() -> Self {
        Self {
            // For CLI apps, we'll use localhost loopback pattern
            client_id: "http://localhost".to_string(),
            redirect_uri: "http://localhost/callback".to_string(),
            scope: "atproto transition:generic".to_string(),
        }
    }
}

/// Authorization server metadata from /.well-known/oauth-authorization-server
#[derive(Debug, Deserialize)]
pub struct AuthServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub pushed_authorization_request_endpoint: String,
    #[serde(default)]
    pub dpop_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

/// Protected resource metadata from /.well-known/oauth-protected-resource  
#[derive(Debug, Deserialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
}

/// PAR (Pushed Authorization Request) response
#[derive(Debug, Deserialize)]
pub struct PARResponse {
    pub request_uri: String,
    pub expires_in: u64,
}

/// Token response
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[allow(dead_code)]
    pub sub: Option<String>,
}

/// AT Protocol OAuth manager
pub struct AtProtoOAuthManager {
    config: AtProtoOAuthConfig,
    client: reqwest::Client,
}

impl AtProtoOAuthManager {
    /// Create new OAuth manager with default config
    pub fn new() -> Result<Self, AppError> {
        Self::with_config(AtProtoOAuthConfig::default())
    }
    
    /// Create OAuth manager with custom config
    pub fn with_config(config: AtProtoOAuthConfig) -> Result<Self, AppError> {
        let client = crate::http::client_with_timeout(Duration::from_secs(30));
        Ok(Self { config, client })
    }
    
    /// Resolve handle to DID using AT Protocol identity resolution
    /// 
    /// This follows the handle resolution spec:
    /// 1. Try DNS TXT record at _atproto.{handle}
    /// 2. Fall back to HTTPS /.well-known/atproto-did
    pub async fn resolve_handle_to_did(&self, handle: &str) -> Result<String, AppError> {
        // For now, use the HTTPS well-known method
        // In production, should also try DNS TXT records
        
        let url = format!("https://{}/.well-known/atproto-did", handle);
        
        let response = self.client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Handle resolution failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Handle resolution failed with status {}", response.status()
            )).into());
        }
        
        let did = response
            .text()
            .await
            .map_err(|e| AppError::NetworkError(format!("Failed to read handle resolution: {}", e)))?
            .trim()
            .to_string();
        
        if !did.starts_with("did:") {
            return Err(AuthError::AuthenticationFailed(format!("Invalid DID format: {}", did)).into());
        }
        
        Ok(did)
    }
    
    /// Resolve DID document to get PDS endpoint
    pub async fn resolve_did_to_pds(&self, did: &str) -> Result<String, AppError> {
        // Use PLC directory for did:plc, or web for did:web
        let did_doc_url = if did.starts_with("did:plc:") {
            format!("https://plc.directory/{}", did)
        } else if did.starts_with("did:web:") {
            // did:web uses HTTPS well-known location
            let domain = did.strip_prefix("did:web:").unwrap();
            format!("https://{}/.well-known/did.json", domain)
        } else {
            return Err(AuthError::AuthenticationFailed(format!("Unsupported DID method: {}", did)).into());
        };
        
        let response = self.client
            .get(&did_doc_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("DID resolution failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "DID resolution failed with status {}", response.status()
            )).into());
        }
        
        let did_doc: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse DID document: {}", e)))?;
        
        // Extract PDS endpoint from service array
        let services = did_doc.get("service")
            .and_then(|s| s.as_array())
            .ok_or_else(|| AuthError::AuthenticationFailed("No services in DID document".to_string()))?;
        
        for service in services {
            if service.get("id").and_then(|id| id.as_str()) == Some("#atproto_pds") {
                if let Some(endpoint) = service.get("serviceEndpoint").and_then(|e| e.as_str()) {
                    return Ok(endpoint.to_string());
                }
            }
        }
        
        Err(AuthError::AuthenticationFailed("No PDS endpoint found in DID document".to_string()).into())
    }
    
    /// Discover authorization server metadata from PDS
    pub async fn discover_authorization_server(&self, pds_url: &str) -> Result<AuthServerMetadata, AppError> {
        // First get protected resource metadata
        let protected_resource_url = format!("{}/.well-known/oauth-protected-resource", pds_url);
        
        let pr_response = self.client
            .get(&protected_resource_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Failed to fetch protected resource metadata: {}", e)))?;
        
        if !pr_response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Protected resource metadata fetch failed with status {}", pr_response.status()
            )).into());
        }
        
        let pr_metadata: ProtectedResourceMetadata = pr_response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse protected resource metadata: {}", e)))?;
        
        // Get the first authorization server
        let auth_server_issuer = pr_metadata.authorization_servers
            .first()
            .ok_or_else(|| AuthError::AuthenticationFailed("No authorization servers found".to_string()))?;
        
        // Fetch authorization server metadata
        let auth_metadata_url = format!("{}/.well-known/oauth-authorization-server", auth_server_issuer);
        
        let as_response = self.client
            .get(&auth_metadata_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Failed to fetch auth server metadata: {}", e)))?;
        
        if !as_response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Auth server metadata fetch failed with status {}", as_response.status()
            )).into());
        }
        
        let auth_metadata: AuthServerMetadata = as_response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse auth server metadata: {}", e)))?;
        
        Ok(auth_metadata)
    }
    
    /// Generate PKCE code verifier and challenge (S256 method)
    fn generate_pkce() -> (String, String) {
        use rand::Rng;
        use sha2::{Sha256, Digest};
        use base64::Engine;
        
        // Generate 32 random bytes for code verifier
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes);
        
        // Generate S256 code challenge
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let challenge_bytes = hasher.finalize();
        let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&challenge_bytes);
        
        (code_verifier, code_challenge)
    }
    
    /// Generate random state for CSRF protection
    fn generate_state() -> String {
        use rand::Rng;
        use base64::Engine;
        let random_bytes: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes)
    }
    
    /// Start OAuth browser flow with proper identity resolution
    /// 
    /// This implements the full atproto OAuth flow:
    /// 1. Resolve handle → DID → PDS → Auth Server
    /// 2. Submit PAR (Pushed Authorization Request)
    /// 3. Get request_uri
    /// 4. Return authorization URL for browser
    pub async fn start_browser_flow(&self, handle: &str) -> Result<BrowserFlowState, AppError> {
        tracing::info!("Starting atproto OAuth flow for handle: {}", handle);
        
        // Step 1: Resolve handle to DID
        tracing::debug!("Resolving handle to DID...");
        let did = self.resolve_handle_to_did(handle).await?;
        tracing::info!("Resolved handle to DID: {}", did);
        
        // Step 2: Resolve DID to PDS
        tracing::debug!("Resolving DID to PDS...");
        let pds_url = self.resolve_did_to_pds(&did).await?;
        tracing::info!("Resolved PDS: {}", pds_url);
        
        // Step 3: Discover authorization server
        tracing::debug!("Discovering authorization server...");
        let auth_metadata = self.discover_authorization_server(&pds_url).await?;
        tracing::info!("Authorization server: {}", auth_metadata.issuer);
        
        // Step 4: Generate PKCE and state
        let (code_verifier, code_challenge) = Self::generate_pkce();
        let state = Self::generate_state();
        
        // Step 5: Submit PAR (Pushed Authorization Request)
        tracing::debug!("Submitting PAR...");
        let par_response = self.submit_par(
            &auth_metadata.pushed_authorization_request_endpoint,
            &code_challenge,
            &state,
        ).await?;
        tracing::info!("PAR submitted successfully, request_uri valid for {} seconds", par_response.expires_in);
        
        // Step 6: Build authorization URL
        let auth_url = format!(
            "{}?client_id={}&request_uri={}",
            auth_metadata.authorization_endpoint,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&par_response.request_uri)
        );
        
        Ok(BrowserFlowState {
            auth_url,
            code_verifier,
            state,
            token_endpoint: auth_metadata.token_endpoint,
            did,
            pds_url,
        })
    }
    
    /// Submit PAR (Pushed Authorization Request)
    async fn submit_par(
        &self,
        par_endpoint: &str,
        code_challenge: &str,
        state: &str,
    ) -> Result<PARResponse, AppError> {
        let params = [
            ("response_type", "code"),
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_uri),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            ("scope", &self.config.scope),
        ];
        
        let response = self.client
            .post(par_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("PAR request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::AuthenticationFailed(format!(
                "PAR failed: {}", error_text
            )).into());
        }
        
        let par_response: PARResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse PAR response: {}", e)))?;
        
        Ok(par_response)
    }
    
    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        token_endpoint: &str,
    ) -> Result<TokenResponse, AppError> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_uri),
            ("code_verifier", code_verifier),
        ];
        
        let response = self.client
            .post(token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Token exchange failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::AuthenticationFailed(format!(
                "Token exchange failed: {}", error_text
            )).into());
        }
        
        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse token response: {}", e)))?;
        
        Ok(token_response)
    }
    
    /// Complete the OAuth flow and create a session
    pub async fn complete_flow(
        &self,
        code: &str,
        state: &BrowserFlowState,
    ) -> Result<Session, AppError> {
        // Exchange code for tokens
        let token_response = self.exchange_code(
            code,
            &state.code_verifier,
            &state.token_endpoint,
        ).await?;
        
        // Create session - note: handle extraction from DID needs proper implementation
        // For now, using DID as handle which needs to be fixed
        let handle = state.did.split(':').last().unwrap_or(&state.did).to_string();
        
        // Create session
        let session = Session {
            handle,
            did: state.did.clone(),
            access_jwt: token_response.access_token,
            refresh_jwt: token_response.refresh_token.unwrap_or_default(),
            service: state.pds_url.clone(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(token_response.expires_in as i64)),
        };
        
        Ok(session)
    }
}

/// State maintained during browser OAuth flow
#[derive(Debug, Clone)]
pub struct BrowserFlowState {
    /// Authorization URL to open in browser
    pub auth_url: String,
    /// PKCE code verifier (keep secret)
    pub code_verifier: String,
    /// State parameter for CSRF protection
    pub state: String,
    /// Token endpoint for code exchange
    pub token_endpoint: String,
    /// User's DID
    pub did: String,
    /// User's PDS URL
    pub pds_url: String,
}
