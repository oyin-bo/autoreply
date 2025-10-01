//! CLI mode implementation
//!
//! Provides command-line interface for the autoreply tools

use clap::{Parser, Subcommand};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Autoreply CLI
#[derive(Parser)]
#[command(name = "autoreply")]
#[command(about = "Bluesky profile and post search utility", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Suppress non-error output (no short flag to avoid conflicts)
    #[arg(long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Retrieve user profile information
    Profile(ProfileArgs),
    /// Search posts within a user's repository
    Search(SearchArgs),
    /// Authenticate with BlueSky
    Login(LoginArgs),
    /// Remove stored credentials
    Logout(LogoutArgs),
    /// Manage authenticated accounts
    Accounts(AccountsArgs),
}

/// Profile tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct ProfileArgs {
    /// Handle (alice.bsky.social) or DID (did:plc:...)
    #[arg(short = 'a', long)]
    #[schemars(description = "Handle (alice.bsky.social) or DID (did:plc:...)")]
    pub account: String,
}

/// Search tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct SearchArgs {
    /// Handle or DID
    #[arg(short = 'a', long)]
    #[schemars(description = "Handle or DID")]
    pub account: String,
    
    /// Search terms (case-insensitive)
    #[arg(short = 'q', long)]
    #[schemars(description = "Search terms (case-insensitive)")]
    pub query: String,
    
    /// Maximum number of results (default 50, max 200)
    #[arg(short = 'l', long)]
    #[schemars(description = "Maximum number of results (default 50, max 200)")]
    pub limit: Option<usize>,
}

/// Login command arguments
#[derive(Parser, Debug)]
pub struct LoginArgs {
    /// Handle (alice.bsky.social)
    #[arg(short = 'u', long)]
    pub handle: Option<String>,
    
    /// App password
    #[arg(short = 'p', long)]
    pub password: Option<String>,
    
    /// Service URL (defaults to https://bsky.social)
    #[arg(short = 's', long)]
    pub service: Option<String>,
}

/// Logout command arguments
#[derive(Parser, Debug)]
pub struct LogoutArgs {
    /// Handle to logout (defaults to current/default account)
    #[arg(short = 'u', long)]
    pub handle: Option<String>,
}

/// Accounts management arguments
#[derive(Parser, Debug)]
pub struct AccountsArgs {
    #[command(subcommand)]
    pub command: AccountsCommands,
}

#[derive(Subcommand, Debug)]
pub enum AccountsCommands {
    /// List all stored accounts
    List,
    /// Set default account
    Default {
        /// Handle to set as default
        handle: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_args() {
        let args = ProfileArgs {
            account: "alice.bsky.social".to_string(),
        };
        assert_eq!(args.account, "alice.bsky.social");
    }

    #[test]
    fn test_search_args() {
        let args = SearchArgs {
            account: "bob.bsky.social".to_string(),
            query: "rust programming".to_string(),
            limit: Some(10),
        };
        assert_eq!(args.account, "bob.bsky.social");
        assert_eq!(args.query, "rust programming");
        assert_eq!(args.limit, Some(10));
    }
}
