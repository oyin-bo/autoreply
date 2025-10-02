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

use anyhow::{Context, Result};
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
        Some(Commands::Accounts) => {
            execute_accounts_cli().await
        }
        Some(Commands::Logout(args)) => {
            execute_logout_cli(args).await
        }
        Some(Commands::Use(args)) => {
            execute_use_cli(args).await
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

/// Execute login command
async fn execute_login_cli(args: cli::LoginArgs) -> Result<String> {
    use std::io::{self, Write};
    
    // Get handle
    let handle = if let Some(h) = args.handle {
        h
    } else {
        print!("BlueSky handle: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };
    
    // Route to appropriate authentication method
    match args.method.as_str() {
        "password" | "" => login_with_password(&handle).await,
        "oauth" => login_with_oauth(&handle).await,
        "device" => login_with_device(&handle).await,
        _ => Err(anyhow::anyhow!(
            "Unsupported authentication method: {}\n\
            Supported methods:\n\
              password - App password authentication (recommended)\n\
              oauth    - OAuth PKCE flow (not yet fully implemented)\n\
              device   - Device authorization (not yet fully implemented)", 
            args.method
        ))
    }
}

/// Login with password
async fn login_with_password(handle: &str) -> Result<String> {
    use std::io::{self, Write};
    
    // Get password
    print!("App password: ");
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    
    // Create credential manager
    let cm = auth::CredentialManager::new()?;
    
    // Store the app password directly as access token
    let creds = auth::Credentials {
        access_token: password,
        refresh_token: String::new(),
        dpop_key: String::new(),
        expires_at: std::time::SystemTime::now() + std::time::Duration::from_secs(30 * 24 * 3600), // 30 days
    };
    
    cm.store_credentials(handle, &creds)?;
    cm.set_default_account(handle)?;
    
    Ok(format!("✓ Successfully stored credentials for @{}\n  Credentials stored securely in system keyring", handle))
}

/// Login with OAuth PKCE flow
async fn login_with_oauth(handle: &str) -> Result<String> {
    use crate::auth::{AtProtoOAuthClient, CredentialManager, DPoPKeyPair, PKCEParams, OAuthCallbackServer};
    use crate::bluesky::did::DidResolver;
    
    println!("Starting OAuth PKCE flow for @{}...", handle);
    
    // Step 1: Resolve handle to DID and discover PDS
    println!("⏳ Resolving handle to DID and discovering PDS...");
    let resolver = DidResolver::new();
    let did = resolver.resolve_handle(handle).await
        .context("Failed to resolve handle to DID")?;
    println!("✓ Resolved DID: {}", did);
    
    let pds = resolver.discover_pds(&did).await
        .context("Failed to discover PDS")?
        .ok_or_else(|| anyhow::anyhow!("No PDS endpoint found for user"))?;
    println!("✓ Discovered PDS: {}", pds);
    
    // Step 2: Discover OAuth metadata
    println!("⏳ Discovering OAuth server metadata...");
    let client = AtProtoOAuthClient::new("autoreply-mcp-client".to_string());
    let metadata = client.discover_metadata(&pds).await
        .context("Failed to discover OAuth metadata")?;
    println!("✓ OAuth server: {}", metadata.issuer);
    
    // Step 3: Generate PKCE and DPoP key pair
    println!("⏳ Generating PKCE challenge and DPoP key pair...");
    let pkce = PKCEParams::generate()
        .context("Failed to generate PKCE parameters")?;
    let dpop_keypair = DPoPKeyPair::generate()
        .context("Failed to generate DPoP key pair")?;
    println!("✓ Generated security parameters");
    
    // Step 4: Send PAR (Pushed Authorization Request)
    println!("⏳ Sending Pushed Authorization Request...");
    let par_response = client.send_par(&metadata, &pkce, &dpop_keypair, handle).await
        .context("Failed to send PAR")?;
    println!("✓ Received request URI (expires in {}s)", par_response.expires_in);
    
    // Step 5: Build authorization URL
    let auth_url = client.build_authorization_url(&metadata, &par_response);
    
    // Step 6: Start callback server
    println!("⏳ Starting OAuth callback server on port 8080...");
    let mut callback_server = OAuthCallbackServer::new(8080);
    
    // Step 7: Open browser
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  Please authorize in your browser:                          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  {}  ║", auth_url);
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    if let Err(e) = open::that(&auth_url) {
        eprintln!("⚠  Could not open browser automatically: {}", e);
        eprintln!("   Please open the URL above manually.");
    }
    
    // Step 8: Wait for callback
    println!("⏳ Waiting for authorization callback...");
    let callback_result = callback_server.wait_for_callback().await
        .context("Failed to receive OAuth callback")?;
    println!("✓ Received authorization code");
    
    // Step 9: Exchange code for tokens
    println!("⏳ Exchanging authorization code for access token...");
    let tokens = client.exchange_code_for_tokens(
        &metadata,
        &callback_result.code,
        &pkce.code_verifier,
        &dpop_keypair,
        "http://127.0.0.1:8080/callback",
    ).await.context("Failed to exchange code for tokens")?;
    println!("✓ Received access token (expires in {}s)", tokens.expires_in);
    
    // Step 10: Store credentials
    println!("⏳ Storing credentials securely...");
    let cm = CredentialManager::new()
        .context("Failed to create credential manager")?;
    
    let dpop_key_pem = dpop_keypair.to_pem()
        .context("Failed to export DPoP key")?;
    
    let creds = crate::auth::Credentials {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token.clone(),
        dpop_key: dpop_key_pem,
        expires_at: tokens.expires_at,
    };
    
    cm.store_credentials(handle, &creds)
        .context("Failed to store credentials")?;
    
    cm.set_default_account(handle)
        .context("Failed to set default account")?;
    
    println!("✓ Successfully authenticated @{}", handle);
    println!("  DID: {}", did);
    println!("  PDS: {}", pds);
    println!("  Credentials stored securely in system keyring");
    
    Ok(format!("✓ Successfully authenticated @{}", handle))
}

/// Login with device flow
#[allow(dead_code)] // Will be used when device flow is fully implemented
async fn login_with_device(_handle: &str) -> Result<String> {
    // NOTE: Full AT Protocol Device Flow implementation requires:
    // 1. DID resolution and PDS discovery for the handle
    // 2. Device authorization endpoint discovery
    // 3. DPoP proof generation for token requests
    // 4. Proper polling with OAuth server metadata
    //
    // The current implementation provides basic device flow primitives but
    // does not implement the complete AT Protocol device authorization.
    // For production use, use app passwords until full implementation.
    
    Err(anyhow::anyhow!(
        "Device authorization flow not yet fully implemented for AT Protocol.\n\
        AT Protocol device flow requires additional components:\n\
          - DID resolution and PDS discovery\n\
          - Device authorization endpoint discovery\n\
          - DPoP proof generation\n\
          - OAuth metadata discovery\n\n\
        Please use --method password (app passwords) for now.\n\
        See docs/12-auth-implementation-plan.md for implementation details."
    ))
}

/// Execute accounts command
async fn execute_accounts_cli() -> Result<String> {
    use std::time::SystemTime;
    
    let cm = auth::CredentialManager::new()?;
    let accounts = cm.list_accounts()?;
    
    if accounts.is_empty() {
        return Ok("No authenticated accounts found.\nRun 'autoreply login' to authenticate.".to_string());
    }
    
    let default_account = cm.get_default_account()?;
    let mut output = String::from("Authenticated Accounts:\n");
    
    for account in accounts {
        let marker = if default_account.as_ref() == Some(&account.handle) {
            "✓"
        } else {
            " "
        };
        
        output.push_str(&format!("  {} {}\n", marker, account.handle));
        
        if !account.did.is_empty() {
            output.push_str(&format!("    DID: {}\n", account.did));
        }
        if !account.pds.is_empty() {
            output.push_str(&format!("    PDS: {}\n", account.pds));
        }
        
        // Format timestamps
        if let Ok(duration) = account.created_at.duration_since(SystemTime::UNIX_EPOCH) {
            let secs = duration.as_secs();
            output.push_str(&format!("    Created: {}\n", format_timestamp(secs)));
        }
        if let Ok(duration) = account.last_used.duration_since(SystemTime::UNIX_EPOCH) {
            let secs = duration.as_secs();
            output.push_str(&format!("    Last used: {}\n", format_timestamp(secs)));
        }
        
        if marker == "✓" {
            output.push_str("    (default)\n");
        }
        output.push('\n');
    }
    
    Ok(output)
}

/// Execute logout command
async fn execute_logout_cli(args: cli::LogoutArgs) -> Result<String> {
    let cm = auth::CredentialManager::new()?;
    cm.delete_credentials(&args.handle)?;
    Ok(format!("✓ Logged out from @{}\n  Credentials removed from system keyring", args.handle))
}

/// Execute use command
async fn execute_use_cli(args: cli::UseArgs) -> Result<String> {
    let cm = auth::CredentialManager::new()?;
    cm.set_default_account(&args.handle)?;
    Ok(format!("✓ Default account set to @{}", args.handle))
}

/// Format a Unix timestamp as a readable string
fn format_timestamp(secs: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    // Simple formatting - in a real app you'd use chrono
    format!("{:?}", dt)
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
