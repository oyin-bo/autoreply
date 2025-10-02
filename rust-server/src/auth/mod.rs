pub mod atproto_oauth;
pub mod config;
pub mod dpop;
pub mod keyring;
pub mod manager;
pub mod oauth;
pub mod oauth_callback;
pub mod types;

pub use atproto_oauth::{AtProtoOAuthClient, OAuthServerMetadata, PARResponse};
#[allow(unused_imports)] // Public API exports for external use
pub use config::{config_path, load_config, save_config};
pub use dpop::{DPoPKeyPair, calculate_access_token_hash};
#[allow(unused_imports)] // Public API exports for external use
pub use keyring::KeyringBackend;
pub use manager::CredentialManager;
#[allow(unused_imports)] // Public API exports for external use
pub use oauth::{
    AuthorizationRequest, AuthorizationResponse, DeviceAuthorizationRequest,
    DeviceAuthorizationResponse, OAuthClient, PKCEParams, PollDeviceTokenRequest,
    TokenRequest, TokenResponse,
};
pub use oauth_callback::{OAuthCallbackResult, OAuthCallbackServer};
#[allow(unused_imports)] // Public API exports for external use
pub use types::{Account, AuthError, Config, Credentials, Settings};
