//! AT Protocol OAuth implementation following the official specification
//!
//! This module implements OAuth 2.0 for AT Protocol as specified in:
//! - <https://atproto.com/specs/auth>
//! - <https://docs.bsky.app/docs/advanced-guides/oauth-client>
//!
//! Key requirements:
//! - Identity resolution: handle → DID → PDS → Auth Server
//! - PAR (Pushed Authorization Requests) - mandatory
//! - PKCE with S256 - mandatory
//! - DPoP with nonce handling - mandatory
//! - Client metadata as URL or loopback

use crate::auth::{AuthError, Session};
use crate::error::AppError;
use base64::Engine;
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// DPoP (Demonstrating Proof of Possession) key manager
pub struct DPoPManager {
    signing_key: SigningKey,
    public_jwk: serde_json::Value,
    nonce: Option<String>,
}

impl DPoPManager {
    /// Create a new DPoP manager with a fresh ES256 keypair
    pub fn new() -> Result<Self, AppError> {
        let signing_key = SigningKey::random(&mut OsRng);

        // Extract public key in JWK format
        let verifying_key = signing_key.verifying_key();
        let encoded_point = verifying_key.to_encoded_point(false);
        let x = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            encoded_point
                .x()
                .ok_or_else(|| AppError::ParseError("Failed to get x coordinate".to_string()))?,
        );
        let y = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            encoded_point
                .y()
                .ok_or_else(|| AppError::ParseError("Failed to get y coordinate".to_string()))?,
        );

        let public_jwk = serde_json::json!({
            "kty": "EC",
            "crv": "P-256",
            "x": x,
            "y": y,
        });

        Ok(Self {
            signing_key,
            public_jwk,
            nonce: None,
        })
    }

    /// Update the DPoP nonce from server response
    pub fn set_nonce(&mut self, nonce: String) {
        self.nonce = Some(nonce);
    }

    /// Generate a DPoP proof JWT for a token endpoint request
    pub fn create_proof(&self, http_method: &str, http_uri: &str) -> Result<String, AppError> {
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

        // Generate random jti (JWT ID)
        let jti = {
            use rand::Rng;
            let random_bytes: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes)
        };

        #[derive(Serialize)]
        struct DPoPClaims {
            jti: String,
            htm: String,
            htu: String,
            iat: i64,
            #[serde(skip_serializing_if = "Option::is_none")]
            nonce: Option<String>,
        }

        let claims = DPoPClaims {
            jti,
            htm: http_method.to_uppercase(),
            htu: http_uri.to_string(),
            iat: chrono::Utc::now().timestamp(),
            nonce: self.nonce.clone(),
        };

        // Create JWT header with embedded JWK
        let mut header = Header::new(Algorithm::ES256);
        header.typ = Some("dpop+jwt".to_string());

        // Convert serde_json::Value to jsonwebtoken::jwk::Jwk
        let jwk_str = serde_json::to_string(&self.public_jwk)
            .map_err(|e| AppError::ParseError(format!("Failed to serialize JWK: {}", e)))?;
        let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_str(&jwk_str)
            .map_err(|e| AppError::ParseError(format!("Failed to parse JWK: {}", e)))?;
        header.jwk = Some(jwk);

        // Sign the JWT - use SEC1 encoding for EC keys
        use p256::pkcs8::EncodePrivateKey;
        let key_der = self
            .signing_key
            .to_pkcs8_der()
            .map_err(|e| AppError::ParseError(format!("Failed to encode key: {}", e)))?;
        let encoding_key = EncodingKey::from_ec_der(key_der.as_bytes());

        encode(&header, &claims, &encoding_key)
            .map_err(|e| AppError::ParseError(format!("Failed to create DPoP proof: {}", e)))
    }
}

/// OAuth client configuration for AT Protocol
#[allow(dead_code)] // Fields used when configuring OAuth
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
            // For development/CLI apps, use http://localhost per atproto OAuth spec
            // This is a special exception for development that allows dynamic redirect_uri
            // The redirect_uri will use 127.0.0.1 per RFC 8252 (set dynamically)
            client_id: "http://localhost".to_string(),
            redirect_uri: "http://127.0.0.1/callback".to_string(),
            scope: "atproto transition:generic".to_string(),
        }
    }
}

/// Authorization server metadata from /.well-known/oauth-authorization-server
#[allow(dead_code)] // Fields used during OAuth server discovery
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
#[allow(dead_code)] // Fields used during resource discovery
#[derive(Debug, Deserialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
}

/// PAR (Pushed Authorization Request) response
#[allow(dead_code)] // Fields used in PAR workflow
#[derive(Debug, Deserialize)]
pub struct PARResponse {
    pub request_uri: String,
    pub expires_in: u64,
}

/// Token response
#[allow(dead_code)] // Fields used in token exchange
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
    dpop: DPoPManager,
}

impl AtProtoOAuthManager {
    /// Create new OAuth manager with default config
    pub fn new() -> Result<Self, AppError> {
        Self::with_config(AtProtoOAuthConfig::default())
    }

    /// Create OAuth manager with custom config
    pub fn with_config(config: AtProtoOAuthConfig) -> Result<Self, AppError> {
        let client = crate::http::client_with_timeout(Duration::from_secs(30));
        let dpop = DPoPManager::new()?;
        Ok(Self {
            config,
            client,
            dpop,
        })
    }

    /// Update redirect URI and scopes (used when callback server port is determined dynamically)
    /// For localhost development clients, scopes must be passed as query parameters
    pub fn set_redirect_uri(&mut self, redirect_uri: String) {
        // For localhost development, we need to add scopes and redirect_uri as query params
        self.config.client_id = format!(
            "http://localhost?redirect_uri={}&scope={}",
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&self.config.scope)
        );
        self.config.redirect_uri = redirect_uri;
    }

    /// Resolve handle to DID using AT Protocol identity resolution
    ///
    /// This follows the handle resolution spec with fallback chain:
    /// 1. Try DNS-based resolution via well-known endpoint
    /// 2. Fall back to directory resolution via api.bsky.app
    pub async fn resolve_handle_to_did(&self, handle: &str) -> Result<String, AppError> {
        // Try DNS-based resolution first (recommended)
        let dns_url = format!("https://{}/.well-known/atproto-did", handle);

        match self
            .client
            .get(&dns_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(did) = response.text().await {
                    let did = did.trim().to_string();
                    if did.starts_with("did:") {
                        return Ok(did);
                    }
                }
            }
            _ => {}
        }

        // Fall back to directory resolution via api.bsky.app
        let api_url = format!(
            "https://api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
            handle
        );

        let response = self
            .client
            .get(&api_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                AuthError::AuthenticationFailed(format!("Handle resolution failed: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Handle resolution failed with status {} for handle '{}'",
                response.status(),
                handle
            ))
            .into());
        }

        #[derive(Deserialize)]
        struct ResolveResponse {
            did: String,
        }

        let result: ResolveResponse = response.json().await.map_err(|e| {
            AuthError::AuthenticationFailed(format!("Failed to parse resolution response: {}", e))
        })?;

        let did = result.did;

        if !did.starts_with("did:") {
            return Err(
                AuthError::AuthenticationFailed(format!("Invalid DID format: {}", did)).into(),
            );
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
            return Err(AuthError::AuthenticationFailed(format!(
                "Unsupported DID method: {}",
                did
            ))
            .into());
        };

        let response = self
            .client
            .get(&did_doc_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("DID resolution failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "DID resolution failed with status {}",
                response.status()
            ))
            .into());
        }

        let did_doc: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse DID document: {}", e)))?;

        // Extract PDS endpoint from service array
        let services = did_doc
            .get("service")
            .and_then(|s| s.as_array())
            .ok_or_else(|| {
                AuthError::AuthenticationFailed("No services in DID document".to_string())
            })?;

        for service in services {
            if service.get("id").and_then(|id| id.as_str()) == Some("#atproto_pds") {
                if let Some(endpoint) = service.get("serviceEndpoint").and_then(|e| e.as_str()) {
                    return Ok(endpoint.to_string());
                }
            }
        }

        Err(
            AuthError::AuthenticationFailed("No PDS endpoint found in DID document".to_string())
                .into(),
        )
    }

    /// Discover authorization server metadata from PDS
    pub async fn discover_authorization_server(
        &self,
        pds_url: &str,
    ) -> Result<AuthServerMetadata, AppError> {
        // First get protected resource metadata
        let protected_resource_url = format!("{}/.well-known/oauth-protected-resource", pds_url);

        let pr_response = self
            .client
            .get(&protected_resource_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                AppError::NetworkError(format!(
                    "Failed to fetch protected resource metadata: {}",
                    e
                ))
            })?;

        if !pr_response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Protected resource metadata fetch failed with status {}",
                pr_response.status()
            ))
            .into());
        }

        let pr_metadata: ProtectedResourceMetadata = pr_response.json().await.map_err(|e| {
            AppError::ParseError(format!(
                "Failed to parse protected resource metadata: {}",
                e
            ))
        })?;

        // Get the first authorization server
        let auth_server_issuer = pr_metadata.authorization_servers.first().ok_or_else(|| {
            AuthError::AuthenticationFailed("No authorization servers found".to_string())
        })?;

        // Fetch authorization server metadata
        let auth_metadata_url = format!(
            "{}/.well-known/oauth-authorization-server",
            auth_server_issuer
        );

        let as_response = self
            .client
            .get(&auth_metadata_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                AppError::NetworkError(format!("Failed to fetch auth server metadata: {}", e))
            })?;

        if !as_response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(format!(
                "Auth server metadata fetch failed with status {}",
                as_response.status()
            ))
            .into());
        }

        let auth_metadata: AuthServerMetadata = as_response.json().await.map_err(|e| {
            AppError::ParseError(format!("Failed to parse auth server metadata: {}", e))
        })?;

        Ok(auth_metadata)
    }

    /// Generate PKCE code verifier and challenge (S256 method)
    fn generate_pkce() -> (String, String) {
        use base64::Engine;
        use rand::Rng;
        use sha2::{Digest, Sha256};

        // Generate 32 random bytes for code verifier
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes);

        // Generate S256 code challenge
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let challenge_bytes = hasher.finalize();
        let code_challenge =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge_bytes);

        (code_verifier, code_challenge)
    }

    /// Generate random state for CSRF protection
    fn generate_state() -> String {
        use base64::Engine;
        use rand::Rng;
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
    pub async fn start_browser_flow(&mut self, handle: &str) -> Result<BrowserFlowState, AppError> {
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
        let par_response = self
            .submit_par(
                &auth_metadata.pushed_authorization_request_endpoint,
                &code_challenge,
                &state,
            )
            .await?;
        tracing::info!(
            "PAR submitted successfully, request_uri valid for {} seconds",
            par_response.expires_in
        );

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

    /// Submit PAR (Pushed Authorization Request) with DPoP
    async fn submit_par(
        &mut self,
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

        // Create DPoP proof for PAR request
        let dpop_proof = self.dpop.create_proof("POST", par_endpoint)?;

        let response = self
            .client
            .post(par_endpoint)
            .header("DPoP", dpop_proof)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("PAR request failed: {}", e)))?;

        // Extract and store DPoP nonce if present
        if let Some(nonce) = response.headers().get("dpop-nonce") {
            if let Ok(nonce_str) = nonce.to_str() {
                self.dpop.set_nonce(nonce_str.to_string());
            }
        }

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if we need to retry with nonce
            if error_text.contains("use_dpop_nonce") {
                // Retry with the nonce we just received
                let dpop_proof = self.dpop.create_proof("POST", par_endpoint)?;
                let retry_response = self
                    .client
                    .post(par_endpoint)
                    .header("DPoP", dpop_proof)
                    .form(&params)
                    .send()
                    .await
                    .map_err(|e| AppError::NetworkError(format!("PAR retry failed: {}", e)))?;

                if !retry_response.status().is_success() {
                    let retry_error = retry_response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(AuthError::AuthenticationFailed(format!(
                        "PAR failed after retry: {}",
                        retry_error
                    ))
                    .into());
                }

                let par_response: PARResponse = retry_response.json().await.map_err(|e| {
                    AppError::ParseError(format!("Failed to parse PAR response: {}", e))
                })?;

                return Ok(par_response);
            }

            return Err(
                AuthError::AuthenticationFailed(format!("PAR failed: {}", error_text)).into(),
            );
        }

        let par_response: PARResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse PAR response: {}", e)))?;

        Ok(par_response)
    }

    /// Exchange authorization code for tokens with DPoP
    pub async fn exchange_code(
        &mut self,
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

        // Create DPoP proof for token request
        let dpop_proof = self.dpop.create_proof("POST", token_endpoint)?;

        let response = self
            .client
            .post(token_endpoint)
            .header("DPoP", dpop_proof)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Token exchange failed: {}", e)))?;

        // Extract and store DPoP nonce if present
        if let Some(nonce) = response.headers().get("dpop-nonce") {
            if let Ok(nonce_str) = nonce.to_str() {
                self.dpop.set_nonce(nonce_str.to_string());
            }
        }

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if we need to retry with nonce
            if error_text.contains("use_dpop_nonce") || error_text.contains("invalid_dpop_proof") {
                tracing::debug!("Retrying token exchange with DPoP nonce");

                // Retry with the nonce we just received
                let dpop_proof = self.dpop.create_proof("POST", token_endpoint)?;
                let retry_response = self
                    .client
                    .post(token_endpoint)
                    .header("DPoP", dpop_proof)
                    .form(&params)
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::NetworkError(format!("Token exchange retry failed: {}", e))
                    })?;

                if !retry_response.status().is_success() {
                    let retry_error = retry_response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(AuthError::AuthenticationFailed(format!(
                        "Token exchange failed after retry: {}",
                        retry_error
                    ))
                    .into());
                }

                let token_response: TokenResponse = retry_response.json().await.map_err(|e| {
                    AppError::ParseError(format!("Failed to parse token response: {}", e))
                })?;

                return Ok(token_response);
            }

            return Err(AuthError::AuthenticationFailed(format!(
                "Token exchange failed: {}",
                error_text
            ))
            .into());
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| AppError::ParseError(format!("Failed to parse token response: {}", e)))?;

        Ok(token_response)
    }

    /// Complete the OAuth flow and create a session
    pub async fn complete_flow(
        &mut self,
        code: &str,
        state: &BrowserFlowState,
    ) -> Result<Session, AppError> {
        // Exchange code for tokens
        let token_response = self
            .exchange_code(code, &state.code_verifier, &state.token_endpoint)
            .await?;

        // Create session - note: handle extraction from DID needs proper implementation
        // For now, using DID as handle which needs to be fixed
        let handle = state
            .did
            .split(':')
            .next_back()
            .unwrap_or(&state.did)
            .to_string();

        // Create session
        let session = Session {
            handle,
            did: state.did.clone(),
            access_jwt: token_response.access_token,
            refresh_jwt: token_response.refresh_token.unwrap_or_default(),
            service: state.pds_url.clone(),
            expires_at: Some(
                chrono::Utc::now() + chrono::Duration::seconds(token_response.expires_in as i64),
            ),
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
