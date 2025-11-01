//! CLI mode implementation
//!
//! Provides command-line interface for the autoreply tools

#![allow(non_snake_case)]

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
    #[arg(short = 'a', long)]
    #[schemars(
        description = "Account to find: handle (alice.bsky.social), DID (did:plc:...), Bsky.app profile URL or even display name or search term"
    )]
    pub account: String,
}

/// Search tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct SearchArgs {
    #[arg(short = 'f', long)]
    #[schemars(description = "Account whose posts to search: handle, DID, Bsky.app profile URL")]
    pub from: String,

    #[arg(short = 'q', long)]
    #[schemars(description = "Search terms")]
    pub query: String,

    #[arg(short = 'l', long)]
    #[schemars(description = "Defaults to 50")]
    pub limit: Option<usize>,
}

/// Post tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct PostArgs {
    #[arg(short = 'a', long)]
    #[schemars(description = "Account to post as: handle, DID, Bsky.app profile URL")]
    pub postAs: String,

    #[arg(short = 't', long)]
    #[schemars(description = "The text of the post")]
    pub text: String,

    #[arg(short = 'r', long)]
    #[schemars(
        description = "When replying to a post, pass the link to that post here, you can use at:// URI or https://bsky.app/... URL or even a simple @handle/rkey form."
    )]
    pub replyTo: Option<String>,
}

/// Feed tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct FeedArgs {
    #[arg(short = 'f', long)]
    #[schemars(description = "Feed URI or name. If omitted, returns the default popular feed")]
    pub feed: Option<String>,

    #[arg(short = 'v', long)]
    #[schemars(
        description = "Optional account to view feed with authenticated pattern: handle, DID, Bsky.app profile URL"
    )]
    pub viewAs: Option<String>,

    #[arg(short = 'c', long)]
    #[schemars(description = "Optional cursor for pagination.")]
    pub continueAtCursor: Option<String>,

    #[arg(short = 'l', long)]
    #[schemars(
        description = "Desired number of posts, when omitted will return a reasonable default batch."
    )]
    pub limit: Option<usize>,
}

/// Thread tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
pub struct ThreadArgs {
    #[arg(short = 'p', long)]
    #[schemars(
        description = "Thread post reference: at:// URI, https://bsky.app/... URL, or @handle/rkey format"
    )]
    pub postURI: String,

    #[arg(short = 'v', long)]
    #[schemars(
        description = "Optional account to view thread as in authenticated mode: handle, DID, Bsky.app profile URL. Use 'anonymous' for incognito mode"
    )]
    pub viewAs: Option<String>,
}

/// React tool arguments
#[derive(Parser, JsonSchema, Deserialize, Serialize, Clone, Debug)]
#[schemars(
    description = "Perform batch reactions on posts. Post references use at:// URIs, https://bsky.app/... URLs, or @handle/rkey format."
)]
pub struct ReactArgs {
    #[arg(short = 'a', long)]
    #[schemars(description = "Account to react as: handle, DID, Bsky.app profile URL")]
    pub reactAs: String,

    #[arg(long)]
    #[schemars(description = "Posts to like")]
    #[serde(default)]
    pub like: Vec<String>,

    #[arg(long)]
    #[schemars(description = "Posts to unlike (remove like)")]
    #[serde(default)]
    pub unlike: Vec<String>,

    #[arg(long)]
    #[schemars(description = "Posts to repost")]
    #[serde(default)]
    pub repost: Vec<String>,

    #[arg(long)]
    #[schemars(description = "Posts to delete (must be your own)")]
    #[serde(default)]
    pub delete: Vec<String>,
}

/// Login command with subcommands for account management
#[derive(Parser, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginCommand {
    #[command(subcommand)]
    pub command: Option<LoginSubcommands>,

    #[arg(short = 'u', long, global = true)]
    #[schemars(
        description = "Handle (alice.bsky.social) - optional for OAuth (allows account selection). Required for app password auth."
    )]
    pub handle: Option<String>,

    #[arg(short = 'p', long, num_args = 0..=1, default_missing_value = "", global = true)]
    #[schemars(
        description = "App password (use this to skip OAuth and authenticate with app password). If provided without value, will prompt on console"
    )]
    pub password: Option<String>,

    // Service is hidden from schema but available for internal/CLI use
    #[arg(short = 's', long, global = true)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub service: Option<String>,
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum LoginSubcommands {
    /// List all stored accounts
    List,
    /// Set default account
    Default {
        #[schemars(description = "Handle to set as default")]
        handle: String,
    },
    /// Remove stored credentials
    Delete {
        #[arg(short = 'u', long)]
        #[schemars(description = "Handle to delete (defaults to current/default account)")]
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
            from: "bob.bsky.social".to_string(),
            query: "rust programming".to_string(),
            limit: Some(10),
        };
        assert_eq!(args.from, "bob.bsky.social");
        assert_eq!(args.query, "rust programming");
        assert_eq!(args.limit, Some(10));
    }

    #[test]
    fn test_post_args() {
        let args = PostArgs {
            postAs: "alice.bsky.social".to_string(),
            text: "Hello, world!".to_string(),
            replyTo: None,
        };
        assert_eq!(args.postAs, "alice.bsky.social");
        assert_eq!(args.text, "Hello, world!");
        assert!(args.replyTo.is_none());
    }

    #[test]
    fn test_react_args() {
        let args = ReactArgs {
            reactAs: "bob.bsky.social".to_string(),
            like: vec!["at://did:plc:abc/app.bsky.feed.post/123".to_string()],
            unlike: vec![],
            repost: vec![],
            delete: vec![],
        };
        assert_eq!(args.reactAs, "bob.bsky.social");
        assert_eq!(args.like.len(), 1);
        assert_eq!(args.unlike.len(), 0);
    }

    #[test]
    fn test_feed_args() {
        let args = FeedArgs {
            feed: Some("at://did:plc:xyz/app.bsky.feed.generator/hot".to_string()),
            viewAs: Some("alice.bsky.social".to_string()),
            continueAtCursor: None,
            limit: Some(50),
        };
        assert_eq!(
            args.feed,
            Some("at://did:plc:xyz/app.bsky.feed.generator/hot".to_string())
        );
        assert_eq!(args.limit, Some(50));
    }

    #[test]
    fn test_thread_args() {
        let args = ThreadArgs {
            postURI: "at://did:plc:abc/app.bsky.feed.post/123".to_string(),
            viewAs: None,
        };
        assert_eq!(args.postURI, "at://did:plc:abc/app.bsky.feed.post/123");
    }
}
