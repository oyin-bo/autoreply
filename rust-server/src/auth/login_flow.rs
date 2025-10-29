use crate::auth::{
    AtProtoOAuthManager, CallbackResult, CallbackServer, CredentialStorage, Credentials,
    SessionManager, StorageBackend,
};
use crate::cli::{LoginCommand, LoginSubcommands};
use crate::error::AppError;
use tokio::time::Duration;
use tracing::{info, warn};

/// Result returned when additional user input is required (MCP elicitation)
pub struct LoginElicitation {
    pub field: String,
    pub message: String,
}

/// Shared login request used by CLI and MCP pathways
pub struct LoginRequest {
    pub payload: LoginCommand,
    pub interactive: bool,
}

/// High-level login outcome
pub struct LoginOutcome {
    pub message: String,
    pub elicitation: Option<LoginElicitation>,
}
pub struct LoginManager {
    storage: CredentialStorage,
}

impl LoginManager {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            storage: CredentialStorage::new()?,
        })
    }

    pub async fn execute(&self, request: LoginRequest) -> Result<LoginOutcome, AppError> {
        match &request.payload.command {
            Some(LoginSubcommands::List) => {
                let accounts = self.storage.list_accounts()?;
                let default = self.storage.get_default_account()?;
                let message = format_account_list(&accounts, default.as_deref());
                Ok(LoginOutcome {
                    message,
                    elicitation: None,
                })
            }
            Some(LoginSubcommands::Default { handle }) => {
                self.storage.get_credentials(handle)?;
                self.storage.set_default_account(handle)?;
                Ok(LoginOutcome {
                    message: format!("✓ Set @{} as default account", handle),
                    elicitation: None,
                })
            }
            Some(LoginSubcommands::Delete { handle }) => {
                let handle_to_delete = if let Some(explicit) = handle.clone() {
                    explicit
                } else {
                    self.storage.get_default_account()?.ok_or_else(|| {
                        AppError::InvalidInput(
                            "No default account set. Specify --handle".to_string(),
                        )
                    })?
                };
                self.storage.remove_account(&handle_to_delete)?;
                Ok(LoginOutcome {
                    message: format!("✓ Deleted account @{}", handle_to_delete),
                    elicitation: None,
                })
            }
            None => self.handle_login(request).await,
        }
    }

    async fn handle_login(&self, request: LoginRequest) -> Result<LoginOutcome, AppError> {
        let LoginCommand {
            mut handle,
            password,
            service,
            ..
        } = request.payload.clone();

        normalize_handle(&mut handle);

        let handle = match handle {
            Some(h) if !h.trim().is_empty() => h,
            _ if request.interactive => {
                return Ok(LoginOutcome {
                    message: "Handle is required".to_string(),
                    elicitation: Some(LoginElicitation {
                        field: "handle".to_string(),
                        message: "Enter Bluesky handle (e.g., alice.bsky.social)".to_string(),
                    }),
                });
            }
            _ => return Err(AppError::InvalidInput("Handle is required".to_string())),
        };

        let explicitly_app_password = password.is_some();

        if explicitly_app_password {
            let pwd = password.unwrap_or_default();
            if pwd.is_empty() {
                if request.interactive {
                    return Ok(LoginOutcome {
                        message: "Password required".to_string(),
                        elicitation: Some(LoginElicitation {
                            field: "password".to_string(),
                            message: format!("App password for @{}", handle),
                        }),
                    });
                }
                return Err(AppError::InvalidInput(r#"# Login via app password failed: the client does not support interactive prompts (MCP elicitation). Please choose one of these options:

1. Use OAuth (strongly recommended): call login with your handle only, e.g.: {"handle": "your.handle.bsky.social"}

2. Provide app password up-front: call login with password, e.g.: {"handle": "your.handle.bsky.social", "password": "your-app-password"}

IMPORTANT Security Warning:
- Do NOT use your main BlueSky account password
- Create an app password at: https://bsky.app/settings/app-passwords
- OAuth is the most secure option and is strongly preferred"#.to_string()));
            }

            let credentials = build_credentials(&handle, &pwd, service.as_deref());
            let message = self
                .authenticate_with_app_password(&handle, credentials)
                .await?;
            return Ok(LoginOutcome {
                message,
                elicitation: None,
            });
        }

        match self
            .authenticate_with_oauth(&handle, service.as_deref())
            .await
        {
            Ok(response) => Ok(LoginOutcome {
                message: response,
                elicitation: None,
            }),
            Err(oauth_error) => {
                warn!("OAuth authentication failed: {}", oauth_error.message());
                if request.interactive {
                    Ok(LoginOutcome {
                        message: format!("OAuth authentication failed: {}", oauth_error.message()),
                        elicitation: Some(LoginElicitation {
                            field: "password".to_string(),
                            message: format!("OAuth failed. Enter app password for @{}", handle),
                        }),
                    })
                } else {
                    Err(oauth_error)
                }
            }
        }
    }

    async fn authenticate_with_oauth(
        &self,
        handle: &str,
        service: Option<&str>,
    ) -> Result<String, AppError> {
        use crate::error::AppError as Err;

        info!("Starting OAuth login flow for @{}", handle);

        let callback_server = CallbackServer::new()
            .map_err(|e| Err::ConfigError(format!("Failed to start callback server: {}", e)))?;

        let mut oauth_manager = AtProtoOAuthManager::new()?;
        oauth_manager.set_redirect_uri(callback_server.callback_url());

        let flow_state = oauth_manager.start_browser_flow(handle).await?;

        info!(
            "OAuth callback server started on {}",
            callback_server.callback_url()
        );
        info!("Authorization URL: {}", flow_state.auth_url);

        if webbrowser::open(&flow_state.auth_url).is_ok() {
            info!("Browser opened for authorization");
        } else {
            eprintln!(
                "Please visit this URL in your browser: {}",
                flow_state.auth_url
            );
        }

        let callback_result = callback_server
            .wait_for_callback(Duration::from_secs(300))
            .await
            .map_err(|e| Err::Authentication(format!("OAuth callback failed: {}", e)))?;

        let session = match callback_result {
            CallbackResult::Success { code, state } => {
                if state != flow_state.state {
                    return Err(Err::Authentication("State parameter mismatch".to_string()));
                }

                info!("OAuth authorization successful, exchanging code for tokens");
                let mut session = oauth_manager.complete_flow(&code, &flow_state).await?;
                if let Some(service_url) = service {
                    session.service = service_url.to_string();
                }

                self.storage.store_credentials_with_fallback(
                    handle,
                    Credentials::with_service(&session.did, &session.refresh_jwt, &session.service),
                )?;
                session
            }
            CallbackResult::Error { error, description } => {
                return Err(Err::Authentication(format!(
                    "OAuth authorization failed: {} - {}",
                    error,
                    description.unwrap_or_else(|| "No description".to_string())
                )));
            }
        };

        self.storage.store_session(handle, session.clone())?;

        ensure_default(&self.storage, handle)?;

        Ok(format!(
            "✓ Successfully authenticated as @{}\n  DID: {}\n  Method: OAuth (browser)\n  Storage: {}",
            session.handle,
            session.did,
            match self.storage.backend() {
                StorageBackend::Keyring => "OS keyring",
                StorageBackend::File => "file",
            }
        ))
    }

    async fn authenticate_with_app_password(
        &self,
        handle: &str,
        credentials: Credentials,
    ) -> Result<String, AppError> {
        info!("Authenticating with app password for @{}", handle);
        let manager = SessionManager::new()?;
        let session = manager.login(&credentials).await?;

        self.storage
            .store_credentials_with_fallback(handle, credentials)?;
        self.storage.store_session(handle, session.clone())?;

        ensure_default(&self.storage, handle)?;

        Ok(format!(
            "✓ Successfully authenticated as @{}\n  DID: {}\n  Method: app password\n  Storage: {}",
            session.handle,
            session.did,
            match self.storage.backend() {
                StorageBackend::Keyring => "OS keyring",
                StorageBackend::File => "file",
            }
        ))
    }
}

fn build_credentials(handle: &str, password: &str, service: Option<&str>) -> Credentials {
    if let Some(service_url) = service {
        Credentials::with_service(handle, password, service_url)
    } else {
        Credentials::new(handle, password)
    }
}

fn ensure_default(storage: &CredentialStorage, handle: &str) -> Result<(), AppError> {
    if storage.get_default_account()?.is_none() {
        storage.set_default_account(handle)?;
    }
    Ok(())
}

fn format_account_list(accounts: &[String], default: Option<&str>) -> String {
    if accounts.is_empty() {
        return "No accounts stored. Use 'autoreply login' to add an account.".to_string();
    }

    let mut output = format!("Authenticated accounts ({}):\n", accounts.len());
    for account in accounts {
        let marker = if Some(account.as_str()) == default {
            " (default)"
        } else {
            ""
        };
        output.push_str(&format!("  • @{}{}\n", account, marker));
    }
    output
}

fn normalize_handle(handle: &mut Option<String>) {
    if let Some(ref mut h) = handle {
        let normalized = h.trim().trim_start_matches('@').to_string();
        if normalized.is_empty() {
            *handle = None;
        } else {
            *h = normalized;
        }
    }
}

// prompt_id removed: CLI and MCP flows coordinate via sequential state and JSON-RPC request IDs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_handle_strips_at() {
        let mut handle = Some("@alice.bsky.social".to_string());
        normalize_handle(&mut handle);
        assert_eq!(handle.as_deref(), Some("alice.bsky.social"));

        let mut empty = Some("   @   ".to_string());
        normalize_handle(&mut empty);
        assert!(empty.is_none());
    }

    #[test]
    fn prompt_ids_removed_placeholder() {
        // placeholder test to keep module structure stable; prompt_id generator removed
        assert!(true);
    }

    #[test]
    fn format_account_list_marks_default() {
        let accounts = vec![
            "alice.bsky.social".to_string(),
            "bob.bsky.social".to_string(),
        ];
        let output = format_account_list(&accounts, Some("alice.bsky.social"));
        assert!(output.contains("@alice.bsky.social (default)"));
        assert!(output.contains("@bob.bsky.social"));
    }
}
