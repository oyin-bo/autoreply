use super::dpop::{calculate_access_token_hash, DPoPKeyPair};
use super::{AuthError, PKCEParams, TokenResponse};
use anyhow::{Context, Result};
use reqwest;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// AT Protocol OAuth client with full DPoP support
pub struct AtProtoOAuthClient {
    client_id: String,
    http_client: reqwest::Client,
}

impl AtProtoOAuthClient {
    /// Create a new AT Protocol OAuth client
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }
    
    /// Discover OAuth server metadata for a PDS following AT Protocol spec
    pub async fn discover_metadata(&self, pds_url: &str) -> Result<OAuthServerMetadata> {
        // Step 1: Get protected resource metadata from PDS to find authorization server
        let protected_resource_url = format!("{}/.well-known/oauth-protected-resource", pds_url);
        
        // Add timeout for protected resource discovery
        let timeout_duration = std::time::Duration::from_secs(10);
        let resp = tokio::time::timeout(
            timeout_duration,
            self.http_client.get(&protected_resource_url).send()
        ).await;
        
        let resp = match resp {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                anyhow::bail!(
                    "Failed to fetch OAuth protected resource metadata from {}: {}. The server may not support AT Protocol OAuth yet. Please try --method password instead.",
                    protected_resource_url, e
                );
            }
            Err(_) => {
                anyhow::bail!(
                    "OAuth protected resource discovery timed out after 10 seconds at {}. The server may not support AT Protocol OAuth yet. Please use --method password instead.",
                    pds_url
                );
            }
        };
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "OAuth protected resource discovery failed with status {} from {}. Response: {}. The server may not support AT Protocol OAuth yet. Please try --method password instead.",
                status, protected_resource_url, body
            );
        }
        
        #[derive(serde::Deserialize)]
        struct ProtectedResourceMetadata {
            authorization_servers: Vec<String>,
        }
        
        let protected_resource: ProtectedResourceMetadata = resp.json().await
            .context(format!("Failed to decode OAuth protected resource metadata from {}", protected_resource_url))?;
        
        if protected_resource.authorization_servers.is_empty() {
            anyhow::bail!(
                "No authorization servers found in protected resource metadata from {}. The server may not support AT Protocol OAuth yet. Please try --method password instead.",
                protected_resource_url
            );
        }
        
        // Step 2: Get authorization server metadata from the first authorization server
        let auth_server_url = &protected_resource.authorization_servers[0];
        let metadata_url = format!("{}/.well-known/oauth-authorization-server", auth_server_url);
        
        let resp2 = tokio::time::timeout(
            timeout_duration,
            self.http_client.get(&metadata_url).send()
        ).await;
        
        let resp2 = match resp2 {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                anyhow::bail!(
                    "Failed to fetch OAuth authorization server metadata from {}: {}. The server may not support AT Protocol OAuth yet. Please try --method password instead.",
                    metadata_url, e
                );
            }
            Err(_) => {
                anyhow::bail!(
                    "OAuth authorization server metadata discovery timed out after 10 seconds at {}. The server may not support AT Protocol OAuth yet. Please use --method password instead.",
                    auth_server_url
                );
            }
        };
        
        if !resp2.status().is_success() {
            let status = resp2.status();
            let body = resp2.text().await.unwrap_or_default();
            anyhow::bail!(
                "OAuth authorization server metadata discovery failed with status {} from {}. Response: {}. The server may not support AT Protocol OAuth yet. Please try --method password instead.",
                status, metadata_url, body
            );
        }
        
        let metadata: OAuthServerMetadata = resp2.json().await
            .context(format!("Failed to decode OAuth authorization server metadata from {}", metadata_url))?;
        Ok(metadata)
    }
    
    /// Send a Pushed Authorization Request (PAR)
    pub async fn send_par(
        &self,
        metadata: &OAuthServerMetadata,
        pkce: &PKCEParams,
        dpop_keypair: &DPoPKeyPair,
        handle: &str,
    ) -> Result<PARResponse, AuthError> {
        let par_endpoint = metadata.pushed_authorization_request_endpoint.as_ref()
            .ok_or_else(|| AuthError::OAuthError("PAR endpoint not found".to_string()))?;
        
        // Create DPoP proof for PAR request
        let dpop_proof = dpop_keypair.create_dpop_proof(
            "POST",
            par_endpoint,
            None,
            None,
        ).map_err(|e| AuthError::OAuthError(format!("Failed to create DPoP proof: {}", e)))?;
        
        // Build PAR request
        let params = [
            ("response_type", "code"),
            ("client_id", &self.client_id),
            ("code_challenge", &pkce.code_challenge),
            ("code_challenge_method", "S256"),
            ("scope", "atproto transition:generic"),
            ("login_hint", handle),
        ];
        
        let resp = self.http_client
            .post(par_endpoint)
            .header("DPoP", dpop_proof)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("PAR request failed: {}", e)))?;
        
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthError(format!(
                "PAR failed with status {}: {}",
                status,
                body
            )));
        }
        
        let par_response: PARResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::ParseError(format!("Failed to decode PAR response: {}", e)))?;
        
        Ok(par_response)
    }
    
    /// Build authorization URL using PAR response
    pub fn build_authorization_url(
        &self,
        metadata: &OAuthServerMetadata,
        par_response: &PARResponse,
    ) -> String {
        format!(
            "{}?client_id={}&request_uri={}",
            metadata.authorization_endpoint,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&par_response.request_uri)
        )
    }
    
    /// Exchange authorization code for tokens using DPoP
    pub async fn exchange_code_for_tokens(
        &self,
        metadata: &OAuthServerMetadata,
        code: &str,
        code_verifier: &str,
        dpop_keypair: &DPoPKeyPair,
        redirect_uri: &str,
    ) -> Result<TokenResponse, AuthError> {
        // Create DPoP proof for token request
        let dpop_proof = dpop_keypair.create_dpop_proof(
            "POST",
            &metadata.token_endpoint,
            None,
            None,
        ).map_err(|e| AuthError::OAuthError(format!("Failed to create DPoP proof: {}", e)))?;
        
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &self.client_id),
            ("code_verifier", code_verifier),
        ];
        
        let resp = self.http_client
            .post(&metadata.token_endpoint)
            .header("DPoP", dpop_proof)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Token request failed: {}", e)))?;
        
        let status = resp.status();
        
        // Extract DPoP nonce if present (for retry)
        let dpop_nonce = resp.headers()
            .get("dpop-nonce")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            
            // If we got a nonce and 401, retry with nonce
            if status.as_u16() == 401 && dpop_nonce.is_some() {
                return self.exchange_code_for_tokens_with_nonce(
                    metadata,
                    code,
                    code_verifier,
                    dpop_keypair,
                    redirect_uri,
                    dpop_nonce.as_deref(),
                ).await;
            }
            
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
    
    /// Exchange code with DPoP nonce (retry)
    async fn exchange_code_for_tokens_with_nonce(
        &self,
        metadata: &OAuthServerMetadata,
        code: &str,
        code_verifier: &str,
        dpop_keypair: &DPoPKeyPair,
        redirect_uri: &str,
        nonce: Option<&str>,
    ) -> Result<TokenResponse, AuthError> {
        // Create DPoP proof with nonce
        let dpop_proof = dpop_keypair.create_dpop_proof(
            "POST",
            &metadata.token_endpoint,
            nonce,
            None,
        ).map_err(|e| AuthError::OAuthError(format!("Failed to create DPoP proof: {}", e)))?;
        
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &self.client_id),
            ("code_verifier", code_verifier),
        ];
        
        let resp = self.http_client
            .post(&metadata.token_endpoint)
            .header("DPoP", dpop_proof)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
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
    
    /// Refresh access token using refresh token with DPoP
    pub async fn refresh_token(
        &self,
        metadata: &OAuthServerMetadata,
        refresh_token: &str,
        dpop_keypair: &DPoPKeyPair,
    ) -> Result<TokenResponse, AuthError> {
        // Create DPoP proof for token refresh
        let dpop_proof = dpop_keypair.create_dpop_proof(
            "POST",
            &metadata.token_endpoint,
            None,
            None,
        ).map_err(|e| AuthError::OAuthError(format!("Failed to create DPoP proof: {}", e)))?;
        
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
        ];
        
        let resp = self.http_client
            .post(&metadata.token_endpoint)
            .header("DPoP", dpop_proof)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Token refresh failed: {}", e)))?;
        
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
    
    /// Make an authenticated API request with DPoP
    pub async fn make_authenticated_request(
        &self,
        method: &str,
        url: &str,
        access_token: &str,
        dpop_keypair: &DPoPKeyPair,
    ) -> Result<reqwest::Response> {
        // Calculate access token hash for DPoP
        let ath = calculate_access_token_hash(access_token);
        
        // Create DPoP proof
        let dpop_proof = dpop_keypair.create_dpop_proof(
            method,
            url,
            None,
            Some(&ath),
        )?;
        
        // Make request
        let req = match method {
            "GET" => self.http_client.get(url),
            "POST" => self.http_client.post(url),
            "PUT" => self.http_client.put(url),
            "DELETE" => self.http_client.delete(url),
            _ => anyhow::bail!("Unsupported HTTP method: {}", method),
        };
        
        let resp = req
            .header("Authorization", format!("DPoP {}", access_token))
            .header("DPoP", dpop_proof)
            .send()
            .await?;
        
        Ok(resp)
    }
}

/// OAuth server metadata from .well-known discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub pushed_authorization_request_endpoint: Option<String>,
    pub dpop_signing_alg_values_supported: Option<Vec<String>>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Vec<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
}

/// PAR (Pushed Authorization Request) response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PARResponse {
    pub request_uri: String,
    pub expires_in: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_atproto_client() {
        let client = AtProtoOAuthClient::new("test-client".to_string());
        assert_eq!(client.client_id, "test-client");
    }
    
    #[test]
    fn test_build_authorization_url() {
        let client = AtProtoOAuthClient::new("test-client".to_string());
        let metadata = OAuthServerMetadata {
            issuer: "https://example.com".to_string(),
            authorization_endpoint: "https://example.com/authorize".to_string(),
            token_endpoint: "https://example.com/token".to_string(),
            pushed_authorization_request_endpoint: Some("https://example.com/par".to_string()),
            dpop_signing_alg_values_supported: Some(vec!["ES256".to_string()]),
            response_types_supported: vec!["code".to_string()],
            grant_types_supported: vec!["authorization_code".to_string()],
            code_challenge_methods_supported: vec!["S256".to_string()],
        };
        
        let par_response = PARResponse {
            request_uri: "urn:ietf:params:oauth:request_uri:test123".to_string(),
            expires_in: 90,
        };
        
        let url = client.build_authorization_url(&metadata, &par_response);
        assert!(url.contains("request_uri="));
        assert!(url.contains("client_id="));
    }
}
