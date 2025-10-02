//! autoreply MCP Server & CLI (Rust)
//!
//! Dual-mode application:
//! - MCP Server Mode (default): Model Context Protocol server using stdio
//! - CLI Mode: Command-line utility for direct tool execution
//!
//! Implements two tools:
//! - `profile(account)` - Retrieve user profile information
//! - `search(account, query)` - Search posts within a user's repository

mod mcp;
mod error;
mod bluesky;
mod tools;
mod http;
mod cli;
mod car;
mod auth;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Detect mode: CLI if args present, MCP server otherwise
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        // CLI mode - parse arguments and execute
        run_cli_mode().await
    } else {
        // MCP server mode - default behavior
        run_mcp_mode().await
    }
}

/// Run in CLI mode
async fn run_cli_mode() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging based on verbosity flags
    let log_level = if cli.quiet {
        "error"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_writer(std::io::stderr) // Log to stderr to keep stdout clean
        .init();

    // Execute command
    let result = match cli.command {
        Some(Commands::Profile(args)) => {
            execute_profile_cli(args).await
        }
        Some(Commands::Search(args)) => {
            execute_search_cli(args).await
        }
        Some(Commands::Login(args)) => {
            execute_login_cli(args).await
        }
        Some(Commands::Logout(args)) => {
            execute_logout_cli(args).await
        }
        Some(Commands::Accounts(args)) => {
            execute_accounts_cli(args).await
        }
        None => {
            eprintln!("Error: No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    };

    // Handle result and exit with appropriate code
    match result {
        Ok(output) => {
            println!("{}", output);
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(get_exit_code(&e));
        }
    }
}

/// Execute profile command in CLI mode
async fn execute_profile_cli(args: cli::ProfileArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};
    
    let result = timeout(Duration::from_secs(120), tools::profile::execute_profile(args)).await;
    
    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result.content.first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute search command in CLI mode
async fn execute_search_cli(args: cli::SearchArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};
    
    let result = timeout(Duration::from_secs(120), tools::search::execute_search(args)).await;
    
    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result.content.first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute login command in CLI mode
async fn execute_login_cli(args: cli::LoginArgs) -> Result<String> {
    use auth::{Credentials, CredentialStorage, SessionManager, OAuthManager, OAuthConfig};
    use std::io::{self, Write};
    
    let storage = CredentialStorage::new()?;
    
    // Get handle - prompt if not provided
    let handle = if let Some(h) = args.handle {
        h
    } else {
        print!("Handle (e.g., alice.bsky.social): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };
    
    if handle.is_empty() {
        return Err(anyhow::anyhow!("Handle is required"));
    }
    
    // Determine authentication method
    let session = if args.device {
        // Device flow is not supported in atproto OAuth spec
        return Err(anyhow::anyhow!(
            "Device flow is not supported in atproto OAuth specification.\n\
             Use --oauth for browser-based OAuth, or app passwords (default)."
        ));
    } else if args.oauth {
        // OAuth browser flow with proper atproto identity resolution
        info!("Starting atproto OAuth browser flow...");
        
        use auth::{AtProtoOAuthManager, AtProtoOAuthConfig, CallbackServer, CallbackResult};
        
        // Create OAuth manager
        let oauth_manager = AtProtoOAuthManager::new()?;
        
        // Start the flow - this does identity resolution and PAR
        info!("Resolving handle and discovering authorization server...");
        let flow_state = oauth_manager.start_browser_flow(&handle).await?;
        
        // Start local callback server
        let callback_server = CallbackServer::new()
            .map_err(|e| anyhow::anyhow!("Failed to start callback server: {}", e))?;
        
        info!("OAuth callback server started on {}", callback_server.callback_url());
        info!("Authorization URL: {}", flow_state.auth_url);
        
        // Open browser
        if webbrowser::open(&flow_state.auth_url).is_ok() {
            info!("Opened browser for authorization");
        } else {
            eprintln!("\nPlease visit this URL in your browser:");
            eprintln!("{}\n", flow_state.auth_url);
        }
        
        // Wait for callback (5 minute timeout)
        info!("Waiting for authorization callback...");
        let callback_result = callback_server
            .wait_for_callback(std::time::Duration::from_secs(300))
            .await
            .map_err(|e| anyhow::anyhow!("OAuth callback failed: {}", e))?;
        
        // Handle callback result
        match callback_result {
            CallbackResult::Success { code, state } => {
                // Verify state matches
                if state != flow_state.state {
                    return Err(anyhow::anyhow!("State parameter mismatch - possible CSRF attack"));
                }
                
                info!("Authorization successful, exchanging code for tokens...");
                
                // Exchange code for tokens
                let session = oauth_manager.complete_flow(&code, &flow_state).await?;
                
                info!("OAuth authentication successful!");
                session
            }
            CallbackResult::Error { error, description } => {
                let desc = description.unwrap_or_else(|| "No description provided".to_string());
                return Err(anyhow::anyhow!("OAuth authorization failed: {} - {}", error, desc));
            }
        }
    } else {
        // App password authentication (default)
        let password = if let Some(p) = args.password {
            p
        } else {
            // Use rpassword crate if available, otherwise just read from stdin
            print!("App password: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        };
        
        if password.is_empty() {
            return Err(anyhow::anyhow!("Password is required for app password authentication"));
        }
        
        // Create credentials
        let credentials = if let Some(service) = args.service {
            Credentials::with_service(&handle, &password, service)
        } else {
            Credentials::new(&handle, &password)
        };
        
        // Authenticate
        info!("Authenticating with app password...");
        let manager = SessionManager::new()?;
        let session = manager.login(&credentials).await?;
        
        // Store credentials for app password method
        storage.add_account(&handle, credentials)?;
        
        session
    };
    
    // Store session
    storage.store_session(&handle, session.clone())?;
    
    // Set as default if it's the first account
    let accounts = storage.list_accounts()?;
    if accounts.len() == 1 || storage.get_default_account()?.is_none() {
        storage.set_default_account(&handle)?;
    }
    
    let auth_method = if args.device {
        "OAuth (device flow)"
    } else if args.oauth {
        "OAuth (browser)"
    } else {
        "app password"
    };
    
    Ok(format!(
        "✓ Successfully authenticated as @{}\n  DID: {}\n  Method: {}\n  Storage: {}",
        session.handle,
        session.did,
        auth_method,
        match storage.backend() {
            auth::StorageBackend::Keyring => "OS keyring",
            auth::StorageBackend::File => "file",
        }
    ))
}

/// Execute logout command in CLI mode
async fn execute_logout_cli(args: cli::LogoutArgs) -> Result<String> {
    use auth::CredentialStorage;
    
    let storage = CredentialStorage::new()?;
    
    // Determine which account to logout
    let handle = if let Some(h) = args.handle {
        h
    } else {
        // Use default account
        storage.get_default_account()?
            .ok_or_else(|| anyhow::anyhow!("No default account set. Specify --handle"))?
    };
    
    // Remove credentials
    storage.remove_account(&handle)?;
    
    Ok(format!("✓ Logged out from @{}", handle))
}

/// Execute accounts command in CLI mode
async fn execute_accounts_cli(args: cli::AccountsArgs) -> Result<String> {
    use auth::CredentialStorage;
    
    let storage = CredentialStorage::new()?;
    
    match args.command {
        cli::AccountsCommands::List => {
            let accounts = storage.list_accounts()?;
            let default_account = storage.get_default_account()?;
            
            if accounts.is_empty() {
                return Ok("No accounts stored. Use 'autoreply login' to add an account.".to_string());
            }
            
            let mut output = format!("Authenticated accounts ({}):\n", accounts.len());
            for account in accounts {
                let marker = if Some(&account) == default_account.as_ref() {
                    " (default)"
                } else {
                    ""
                };
                output.push_str(&format!("  • @{}{}\n", account, marker));
            }
            
            Ok(output)
        }
        cli::AccountsCommands::Default { handle } => {
            // Verify account exists
            storage.get_credentials(&handle)?;
            
            // Set as default
            storage.set_default_account(&handle)?;
            
            Ok(format!("✓ Set @{} as default account", handle))
        }
    }
}

/// Map AppError to exit code
fn get_exit_code(err: &anyhow::Error) -> i32 {
    let err_str = err.to_string().to_lowercase();
    
    if err_str.contains("invalid") || err_str.contains("usage") {
        1 // Invalid arguments or usage error
    } else if err_str.contains("network") || err_str.contains("connection") {
        2 // Network or API error
    } else if err_str.contains("not found") {
        3 // Not found error
    } else if err_str.contains("timeout") {
        4 // Timeout error
    } else {
        5 // Other application errors
    }
}

/// Run in MCP server mode
async fn run_mcp_mode() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting autoreply MCP Server");

    // Handle stdio MCP communication
    mcp::handle_stdio().await?;

    Ok(())
}
