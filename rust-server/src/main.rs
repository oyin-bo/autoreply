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
    
    // For now, only support password method
    if args.method != "password" {
        return Err(anyhow::anyhow!("Authentication method {} not yet implemented", args.method));
    }
    
    // Get password
    print!("App password: ");
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    
    // Create credential manager
    let cm = auth::CredentialManager::new()?;
    
    // For now, store the app password as a placeholder
    // TODO: Implement actual OAuth flow
    let creds = auth::Credentials {
        access_token: password, // Placeholder until OAuth is implemented
        refresh_token: String::new(),
        dpop_key: String::new(),
        expires_at: std::time::SystemTime::now() + std::time::Duration::from_secs(30 * 24 * 3600), // 30 days
    };
    
    cm.store_credentials(&handle, &creds)?;
    cm.set_default_account(&handle)?;
    
    Ok(format!("✓ Successfully stored credentials for @{}\n  Credentials stored securely in system keyring", handle))
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
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    
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
