//! Unit tests for auth module helpers and utilities

#[cfg(test)]
mod auth_helper_tests {
    use crate::auth::session::Session;
    use crate::auth::credentials::Credentials;
    use chrono::{Utc, Duration};

    #[test]
    fn test_session_expiration_check_not_expired() {
        // Session with expiry far in the future should not be expired
        let session = Session {
            access_jwt: "test_access".to_string(),
            refresh_jwt: "test_refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() + Duration::hours(2)),
        };

        assert!(!session.is_expired(), "Session should not be expired when expiry is far in future");
    }

    #[test]
    fn test_session_expiration_check_expired_soon() {
        // Session expiring in 3 minutes (within 5-minute window) should be considered expired
        let session = Session {
            access_jwt: "test_access".to_string(),
            refresh_jwt: "test_refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() + Duration::minutes(3)),
        };

        assert!(session.is_expired(), "Session should be expired when within 5-minute window");
    }

    #[test]
    fn test_session_expiration_check_already_expired() {
        // Session with past expiry should be expired
        let session = Session {
            access_jwt: "test_access".to_string(),
            refresh_jwt: "test_refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() - Duration::hours(1)),
        };

        assert!(session.is_expired(), "Session should be expired when expiry is in the past");
    }

    #[test]
    fn test_session_no_expiry_timestamp_not_expired() {
        // Session without explicit expiry timestamp should not be considered expired
        let session = Session {
            access_jwt: "test_access".to_string(),
            refresh_jwt: "test_refresh".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:test".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: None,
        };

        assert!(!session.is_expired(), "Session without expiry should not be expired");
    }

    #[test]
    fn test_credentials_new_basic() {
        // Test basic credential creation
        let creds = Credentials::new("user@example.com", "password123");
        assert_eq!(creds.identifier, "user@example.com");
        // Note: Credentials struct should have the password field accessible or we test what we can
    }

    #[test]
    fn test_credentials_with_service() {
        // Test credential creation with custom service
        let creds = Credentials::with_service(
            "alice.bsky.social",
            "app_password_123",
            "https://custom.pds.service",
        );
        assert_eq!(creds.identifier, "alice.bsky.social");
        assert_eq!(creds.service, "https://custom.pds.service");
    }

    #[test]
    fn test_session_serialization_roundtrip() {
        // Test that session can be serialized and deserialized
        let original = Session {
            access_jwt: "access_token_xyz".to_string(),
            refresh_jwt: "refresh_token_abc".to_string(),
            handle: "alice.bsky.social".to_string(),
            did: "did:plc:test123".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original).expect("Should serialize");
        
        // Deserialize back
        let deserialized: Session = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(original.access_jwt, deserialized.access_jwt);
        assert_eq!(original.handle, deserialized.handle);
        assert_eq!(original.did, deserialized.did);
    }

    #[test]
    fn test_session_serialization_without_expiry() {
        // Test serialization of session without expiry field
        let original = Session {
            access_jwt: "access_token".to_string(),
            refresh_jwt: "refresh_token".to_string(),
            handle: "test.bsky.social".to_string(),
            did: "did:plc:abc".to_string(),
            service: "https://bsky.social".to_string(),
            expires_at: None,
        };

        let json = serde_json::to_string(&original).expect("Should serialize");
        assert!(!json.contains("expiresAt"), "Should not include null expiresAt field");
        
        let deserialized: Session = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.expires_at, None);
    }
}
