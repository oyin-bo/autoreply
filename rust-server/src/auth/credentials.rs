//! Credential management for BlueSky accounts

use serde::{Deserialize, Serialize};

/// User credentials for BlueSky authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    /// Account identifier (handle or DID)
    pub identifier: String,
    
    /// App password
    pub password: String,
    
    /// Service URL (defaults to https://bsky.social)
    #[serde(default = "default_service")]
    pub service: String,
}

fn default_service() -> String {
    crate::auth::DEFAULT_SERVICE.to_string()
}

impl Credentials {
    /// Create new credentials
    pub fn new(identifier: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            password: password.into(),
            service: default_service(),
        }
    }
    
    /// Create credentials with custom service URL
    pub fn with_service(
        identifier: impl Into<String>,
        password: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            password: password.into(),
            service: service.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_credentials_new() {
        let creds = Credentials::new("alice.bsky.social", "app-password-123");
        assert_eq!(creds.identifier, "alice.bsky.social");
        assert_eq!(creds.password, "app-password-123");
        assert_eq!(creds.service, "https://bsky.social");
    }
    
    #[test]
    fn test_credentials_with_service() {
        let creds = Credentials::with_service(
            "bob.bsky.social",
            "password",
            "https://custom.pds.example",
        );
        assert_eq!(creds.service, "https://custom.pds.example");
    }
    
    #[test]
    fn test_credentials_serialization() {
        let creds = Credentials::new("test.bsky.social", "pass");
        let json = serde_json::to_string(&creds).unwrap();
        let deserialized: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(creds.identifier, deserialized.identifier);
        assert_eq!(creds.password, deserialized.password);
    }
}
