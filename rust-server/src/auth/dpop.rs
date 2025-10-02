use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use p256::ecdsa::{SigningKey, VerifyingKey};
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// DPoP key pair for token binding
#[derive(Clone)]
pub struct DPoPKeyPair {
    pub private_key: SigningKey,
    pub public_key: VerifyingKey,
    pub jwk_thumbprint: String,
}

impl DPoPKeyPair {
    /// Generate a new ES256 key pair for DPoP
    pub fn generate() -> Result<Self> {
        let private_key = SigningKey::random(&mut rand::thread_rng());
        let public_key = VerifyingKey::from(&private_key);
        
        // Calculate JWK thumbprint (used as key ID)
        let jwk_thumbprint = Self::calculate_jwk_thumbprint(&public_key)?;
        
        Ok(Self {
            private_key,
            public_key,
            jwk_thumbprint,
        })
    }
    
    /// Load from PEM-encoded private key
    pub fn from_pem(pem: &str) -> Result<Self> {
        use p256::pkcs8::DecodePrivateKey;
        let private_key = SigningKey::from_pkcs8_pem(pem)
            .context("Failed to parse private key PEM")?;
        let public_key = VerifyingKey::from(&private_key);
        let jwk_thumbprint = Self::calculate_jwk_thumbprint(&public_key)?;
        
        Ok(Self {
            private_key,
            public_key,
            jwk_thumbprint,
        })
    }
    
    /// Export private key as PEM
    pub fn to_pem(&self) -> Result<String> {
        self.private_key
            .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
            .context("Failed to encode private key as PEM")
            .map(|s| s.to_string())
    }
    
    /// Get public key as JWK
    pub fn public_jwk(&self) -> Result<serde_json::Value> {
        use p256::elliptic_curve::sec1::ToEncodedPoint;
        
        let point = self.public_key.to_encoded_point(false);
        let x = URL_SAFE_NO_PAD.encode(point.x().ok_or_else(|| anyhow::anyhow!("Missing x coordinate"))?);
        let y = URL_SAFE_NO_PAD.encode(point.y().ok_or_else(|| anyhow::anyhow!("Missing y coordinate"))?);
        
        Ok(serde_json::json!({
            "kty": "EC",
            "crv": "P-256",
            "x": x,
            "y": y,
            "use": "sig",
            "alg": "ES256",
        }))
    }
    
    /// Calculate JWK thumbprint (SHA-256 of canonical JWK)
    fn calculate_jwk_thumbprint(public_key: &VerifyingKey) -> Result<String> {
        use p256::elliptic_curve::sec1::ToEncodedPoint;
        
        let point = public_key.to_encoded_point(false);
        let x = URL_SAFE_NO_PAD.encode(point.x().ok_or_else(|| anyhow::anyhow!("Missing x coordinate"))?);
        let y = URL_SAFE_NO_PAD.encode(point.y().ok_or_else(|| anyhow::anyhow!("Missing y coordinate"))?);
        
        // Canonical JWK (RFC 7638) - fields in lexicographic order
        let canonical = format!(r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#, x, y);
        
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();
        
        Ok(URL_SAFE_NO_PAD.encode(&hash))
    }
    
    /// Create a DPoP proof JWT
    pub fn create_dpop_proof(
        &self,
        htm: &str,  // HTTP method (e.g., "POST")
        htu: &str,  // HTTP URI (e.g., "https://server.example.com/token")
        nonce: Option<&str>,  // Server-provided nonce
        ath: Option<&str>,  // Access token hash (for authenticated requests)
    ) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        let jti = Uuid::new_v4().to_string();
        
        let mut claims = serde_json::json!({
            "jti": jti,
            "htm": htm,
            "htu": htu,
            "iat": now,
            "jwk": self.public_jwk()?,
        });
        
        if let Some(n) = nonce {
            claims["nonce"] = serde_json::Value::String(n.to_string());
        }
        
        if let Some(a) = ath {
            claims["ath"] = serde_json::Value::String(a.to_string());
        }
        
        // Create JWT header
        let mut header = Header::new(Algorithm::ES256);
        header.typ = Some("dpop+jwt".to_string());
        
        // Sign the JWT
        use p256::ecdsa::signature::Signer;
        let claims_json = serde_json::to_string(&claims)?;
        let header_json = serde_json::to_string(&header)?;
        
        let header_b64 = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
        let payload_b64 = URL_SAFE_NO_PAD.encode(claims_json.as_bytes());
        let signing_input = format!("{}.{}", header_b64, payload_b64);
        
        let signature: p256::ecdsa::Signature = self.private_key.sign(signing_input.as_bytes());
        let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        
        Ok(format!("{}.{}.{}", header_b64, payload_b64, signature_b64))
    }
}

/// Calculate access token hash for DPoP ath claim
pub fn calculate_access_token_hash(access_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(access_token.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(&hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_dpop_keypair() {
        let keypair = DPoPKeyPair::generate().unwrap();
        assert!(!keypair.jwk_thumbprint.is_empty());
    }
    
    #[test]
    fn test_export_import_pem() {
        let keypair1 = DPoPKeyPair::generate().unwrap();
        let pem = keypair1.to_pem().unwrap();
        let keypair2 = DPoPKeyPair::from_pem(&pem).unwrap();
        assert_eq!(keypair1.jwk_thumbprint, keypair2.jwk_thumbprint);
    }
    
    #[test]
    fn test_create_dpop_proof() {
        let keypair = DPoPKeyPair::generate().unwrap();
        let proof = keypair.create_dpop_proof(
            "POST",
            "https://server.example.com/token",
            None,
            None,
        ).unwrap();
        
        // Basic validation - should have 3 parts
        let parts: Vec<&str> = proof.split('.').collect();
        assert_eq!(parts.len(), 3);
    }
    
    #[test]
    fn test_create_dpop_proof_with_nonce() {
        let keypair = DPoPKeyPair::generate().unwrap();
        let proof = keypair.create_dpop_proof(
            "POST",
            "https://server.example.com/token",
            Some("test-nonce-123"),
            None,
        ).unwrap();
        
        assert!(proof.len() > 100);
    }
    
    #[test]
    fn test_access_token_hash() {
        let token = "test-access-token";
        let hash = calculate_access_token_hash(token);
        assert!(!hash.is_empty());
        
        // Hash should be deterministic
        let hash2 = calculate_access_token_hash(token);
        assert_eq!(hash, hash2);
    }
}
