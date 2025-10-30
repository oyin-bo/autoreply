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
    /// Get the latest feed from BlueSky
    Feed(FeedArgs),
    /// Fetch a thread by post URI
    Thread(ThreadArgs),
    /// Create a new post or reply on BlueSky
    Post(PostArgs),
    /// Perform batch reactions on posts (like, unlike, repost, delete)
    React(ReactArgs),
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

/// Post tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct PostArgs {
    /// Handle or DID to post as
    #[arg(short = 'a', long)]
    #[schemars(description = "Handle or DID to post as (postAs)")]
    #[serde(rename = "postAs")]
    pub post_as: String,

    /// The text content of the post
    #[arg(short = 't', long)]
    #[schemars(description = "The text content of the post")]
    pub text: String,

    /// Optional post URI or URL to reply to
    #[arg(short = 'r', long)]
    #[schemars(description = "Optional at:// URI or https://bsky.app/... URL to reply to")]
    #[serde(rename = "replyTo")]
    pub reply_to: Option<String>,
}

/// Feed tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct FeedArgs {
    /// Optional feed URI or name to search for
    #[arg(short = 'f', long)]
    #[schemars(description = "Optional feed URI or name. If unspecified, returns the default popular feed")]
    pub feed: Option<String>,

    /// Optional BlueSky handle for authenticated feed
    #[arg(short = 'u', long)]
    #[schemars(description = "Optional BlueSky handle for authenticated access")]
    pub login: Option<String>,

    /// Optional password for authentication
    #[arg(short = 'p', long)]
    #[schemars(description = "Optional BlueSky password")]
    pub password: Option<String>,

    /// Cursor for pagination
    #[arg(short = 'c', long)]
    #[schemars(description = "Optional cursor for pagination")]
    pub cursor: Option<String>,

    /// Limit the number of posts returned (default 20, max 100)
    #[arg(short = 'l', long)]
    #[schemars(description = "Limit the number of posts (default 20, max 100)")]
    pub limit: Option<usize>,
}

/// Thread tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct ThreadArgs {
    /// The BlueSky URL or at:// URI of the post
    #[arg(short = 'p', long)]
    #[schemars(description = "The BlueSky URL or at:// URI of the post to fetch the thread for")]
    #[serde(rename = "postURI")]
    pub post_uri: String,

    /// Optional BlueSky handle for authenticated access
    #[arg(short = 'u', long)]
    #[schemars(description = "Optional BlueSky handle for authenticated access")]
    pub login: Option<String>,

    /// Optional password for authentication
    #[arg(short = 'w', long)]
    #[schemars(description = "Optional BlueSky password")]
    pub password: Option<String>,
}

/// React tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct ReactArgs {
    /// Handle or DID to react as
    #[arg(short = 'a', long)]
    #[schemars(description = "Handle or DID to react as (reactAs)")]
    #[serde(rename = "reactAs")]
    pub react_as: String,

    /// Post URIs/URLs to like
    #[arg(long)]
    #[schemars(description = "Array of post URIs/URLs to like")]
    #[serde(default)]
    pub like: Vec<String>,

    /// Post URIs/URLs to unlike
    #[arg(long)]
    #[schemars(description = "Array of post URIs/URLs to unlike")]
    #[serde(default)]
    pub unlike: Vec<String>,

    /// Post URIs/URLs to repost
    #[arg(long)]
    #[schemars(description = "Array of post URIs/URLs to repost")]
    #[serde(default)]
    pub repost: Vec<String>,

    /// Post URIs/URLs to delete
    #[arg(long)]
    #[schemars(description = "Array of post URIs/URLs to delete")]
    #[serde(default)]
    pub delete: Vec<String>,
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
            account: "bob.bsky.social".to_string(),
            query: "rust programming".to_string(),
            limit: Some(10),
        };
        assert_eq!(args.account, "bob.bsky.social");
        assert_eq!(args.query, "rust programming");
        assert_eq!(args.limit, Some(10));
    }

    #[test]
    fn test_post_args() {
        let args = PostArgs {
            post_as: "alice.bsky.social".to_string(),
            text: "Hello, world!".to_string(),
            reply_to: None,
        };
        assert_eq!(args.post_as, "alice.bsky.social");
        assert_eq!(args.text, "Hello, world!");
        assert!(args.reply_to.is_none());
    }

    #[test]
    fn test_react_args() {
        let args = ReactArgs {
            react_as: "bob.bsky.social".to_string(),
            like: vec!["at://did:plc:abc/app.bsky.feed.post/123".to_string()],
            unlike: vec![],
            repost: vec![],
            delete: vec![],
        };
        assert_eq!(args.react_as, "bob.bsky.social");
        assert_eq!(args.like.len(), 1);
        assert_eq!(args.unlike.len(), 0);
    }

    #[test]
    fn test_feed_args() {
        let args = FeedArgs {
            feed: Some("at://did:plc:xyz/app.bsky.feed.generator/hot".to_string()),
            login: Some("alice.bsky.social".to_string()),
            password: None,
            cursor: None,
            limit: Some(50),
        };
        assert_eq!(args.feed, Some("at://did:plc:xyz/app.bsky.feed.generator/hot".to_string()));
        assert_eq!(args.limit, Some(50));
    }

    #[test]
    fn test_thread_args() {
        let args = ThreadArgs {
            post_uri: "at://did:plc:abc/app.bsky.feed.post/123".to_string(),
            login: None,
            password: None,
        };
        assert_eq!(args.post_uri, "at://did:plc:abc/app.bsky.feed.post/123");
    }
}
