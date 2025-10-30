//! autoreply MCP Server & CLI (Rust)
//!
//! Dual-mode application:
//! - MCP Server Mode (default): Model Context Protocol server using stdio
//! - CLI Mode: Command-line utility for direct tool execution
//!
//! Implements two tools:
//! - `profile(account)` - Retrieve user profile information
//! - `search(account, query)` - Search posts within a user's repository

mod auth;
mod bluesky;
mod car;
mod cli;
mod error;
mod http;
mod mcp;

#[cfg(feature = "experimental-sentencepiece")]
mod sentencepiece;

#[cfg(not(feature = "experimental-sentencepiece"))]
#[allow(dead_code)]
mod sentencepiece_stub;

mod tools;

use anyhow::Result;
use auth::{LoginManager, LoginRequest};
use clap::Parser;
use cli::{Cli, Commands};
use tracing::info;

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
        Some(Commands::Profile(args)) => execute_profile_cli(args).await,
        Some(Commands::Search(args)) => execute_search_cli(args).await,
        Some(Commands::Login(args)) => execute_login_cli(args).await,
        Some(Commands::Feed(args)) => execute_feed_cli(args).await,
        Some(Commands::Thread(args)) => execute_thread_cli(args).await,
        Some(Commands::Post(args)) => execute_post_cli(args).await,
        Some(Commands::React(args)) => execute_react_cli(args).await,
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

    let result = timeout(
        Duration::from_secs(120),
        tools::profile::execute_profile(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
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

    let result = timeout(
        Duration::from_secs(120),
        tools::search::execute_search(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute login command in CLI mode
async fn execute_login_cli(args: cli::LoginCommand) -> Result<String> {
    use std::io::{self, Write};

    let manager = LoginManager::new()?;
    let mut command = args;

    loop {
        let request = LoginRequest {
            payload: command.clone(),
            interactive: true,
        };

        let outcome = manager
            .execute(request)
            .await
            .map_err(|e| anyhow::anyhow!(e.message()))?;

        if let Some(elicitation) = outcome.elicitation {
            if !outcome.message.is_empty() {
                eprintln!("{}", outcome.message);
            }

            match elicitation.field.as_str() {
                "handle" => {
                    eprint!("{}: ", elicitation.message);
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    let value = input.trim();
                    command.handle = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                "password" => {
                    eprint!("{}: ", elicitation.message);
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    command.password = Some(input.trim().to_string());
                }
                other => {
                    return Err(anyhow::anyhow!(format!(
                        "Unsupported login prompt field: {}",
                        other
                    )));
                }
            }

            continue;
        }

        return Ok(outcome.message);
    }
}

/// Execute feed command in CLI mode
async fn execute_feed_cli(args: cli::FeedArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};

    let result = timeout(
        Duration::from_secs(120),
        tools::feed::execute_feed(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute thread command in CLI mode
async fn execute_thread_cli(args: cli::ThreadArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};

    let result = timeout(
        Duration::from_secs(120),
        tools::thread::execute_thread(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute post command in CLI mode
async fn execute_post_cli(args: cli::PostArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};

    let result = timeout(
        Duration::from_secs(120),
        tools::post::execute_post(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
    }
}

/// Execute react command in CLI mode
async fn execute_react_cli(args: cli::ReactArgs) -> Result<String> {
    use tokio::time::{timeout, Duration};

    let result = timeout(
        Duration::from_secs(120),
        tools::react::execute_react(args),
    )
    .await;

    match result {
        Ok(Ok(tool_result)) => {
            // Extract markdown text from ToolResult
            Ok(tool_result
                .content
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!(e.message())),
        Err(_) => Err(anyhow::anyhow!("Request exceeded 120 second timeout")),
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
