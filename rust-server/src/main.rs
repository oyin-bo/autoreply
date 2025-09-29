//! autoreply MCP Server (Rust)
//!
//! Model Context Protocol server for Bluesky profile and post search functionality.
//! Implements two MCP tools:
//! - `profile(account)` - Retrieve user profile information
//! - `search(account, query)` - Search posts within a user's repository

mod mcp;
mod error;
mod cache;
mod bluesky;
mod tools;
mod http;

use anyhow::Result;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting autoreply MCP Server");

    // Handle stdio MCP communication
    mcp::handle_stdio().await?;

    Ok(())
}
