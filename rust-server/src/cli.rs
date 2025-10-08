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
    /// Manage authentication and accounts
    Login(LoginCommand),
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
    /// Handle or DID of the account to search posts from (optional when login is provided)
    #[arg(short = 'f', long)]
    #[schemars(description = "Handle or DID of the account to search posts from (optional when login is provided)")]
    pub from: Option<String>,

    /// Search terms (case-insensitive)
    #[arg(short = 'q', long)]
    #[schemars(description = "Search terms (case-insensitive)")]
    pub query: String,

    /// Maximum number of results (default 50, max 200)
    #[arg(short = 'l', long)]
    #[schemars(description = "Maximum number of results (default 50, max 200)")]
    pub limit: Option<usize>,

    /// Login handle for authenticated search (must be previously authenticated)
    #[arg(long)]
    #[schemars(description = "Login handle for authenticated search (must be previously authenticated)")]
    pub login: Option<String>,
}

/// Login command with subcommands for account management
#[derive(Parser, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginCommand {
    #[command(subcommand)]
    pub command: Option<LoginSubcommands>,

    /// Handle (alice.bsky.social) - for add operation
    #[arg(short = 'u', long, global = true)]
    pub handle: Option<String>,

    /// App password (use this to skip OAuth and authenticate with app password)
    /// If provided without value, will prompt on console
    #[arg(short = 'p', long, num_args = 0..=1, default_missing_value = "", global = true)]
    pub password: Option<String>,

    /// Service URL (defaults to <https://bsky.social>)
    #[arg(short = 's', long, global = true)]
    pub service: Option<String>,

    /// Opaque prompt identifier used by MCP login elicitation
    #[arg(skip = Option::<String>::None)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Opaque prompt identifier used when responding to MCP login prompts")]
    pub prompt_id: Option<String>,
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum LoginSubcommands {
    /// List all stored accounts
    List,
    /// Set default account
    Default {
        /// Handle to set as default
        handle: String,
    },
    /// Remove stored credentials
    Delete {
        /// Handle to delete (defaults to current/default account)
        #[arg(short = 'u', long)]
        handle: Option<String>,
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
            from: Some("bob.bsky.social".to_string()),
            query: "rust programming".to_string(),
            limit: Some(10),
            login: None,
        };
        assert_eq!(args.from, Some("bob.bsky.social".to_string()));
        assert_eq!(args.query, "rust programming");
        assert_eq!(args.limit, Some(10));
    }
}
