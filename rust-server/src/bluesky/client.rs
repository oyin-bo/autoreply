//! BlueSky API client for feed and thread operations

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// BlueSky API client
pub struct BskyClient {
    client: reqwest::Client,
    service: String,
    access_token: Option<String>,
}

impl BskyClient {
    /// Create a new client (unauthenticated)
    pub fn new() -> Self {
        let client = crate::http::client_with_timeout(std::time::Duration::from_secs(30));
        Self {
            client,
            service: "https://public.api.bsky.app".to_string(),
            access_token: None,
        }
    }

    /// Create a new authenticated client
    pub fn with_auth(access_token: String) -> Self {
        let client = crate::http::client_with_timeout(std::time::Duration::from_secs(30));
        Self {
            client,
            service: "https://bsky.social".to_string(),
            access_token: Some(access_token),
        }
    }

    /// Get feed using app.bsky.feed.getFeed
    pub async fn get_feed(
        &self,
        feed_uri: Option<&str>,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<FeedResponse> {
        let feed = feed_uri.unwrap_or("at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot");
        
        let mut params: Vec<(String, String)> = vec![("feed".to_string(), feed.to_string())];
        
        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c.to_string()));
        }
        
        if let Some(l) = limit {
            params.push(("limit".to_string(), l.to_string()));
        }

        let url = format!("{}/xrpc/app.bsky.feed.getFeed", self.service);
        
        let mut request = self.client.get(&url).query(&params);
        
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Feed API error {}: {}", status, text));
        }

        let feed_response: FeedResponse = response.json().await?;
        Ok(feed_response)
    }

    /// Get timeline using app.bsky.feed.getTimeline
    pub async fn get_timeline(
        &self,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<FeedResponse> {
        let mut params: Vec<(String, String)> = vec![];
        
        if let Some(c) = cursor {
            params.push(("cursor".to_string(), c.to_string()));
        }
        
        if let Some(l) = limit {
            params.push(("limit".to_string(), l.to_string()));
        }

        let url = format!("{}/xrpc/app.bsky.feed.getTimeline", self.service);
        
        let mut request = self.client.get(&url).query(&params);
        
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Timeline API error {}: {}", status, text));
        }

        let feed_response: FeedResponse = response.json().await?;
        Ok(feed_response)
    }

    /// Get post thread using app.bsky.feed.getPostThread
    pub async fn get_post_thread(&self, uri: &str) -> Result<ThreadResponse> {
        let url = format!("{}/xrpc/app.bsky.feed.getPostThread", self.service);
        
        let mut request = self.client.get(&url).query(&[("uri", uri)]);
        
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Thread API error {}: {}", status, text));
        }

        let thread_response: ThreadResponse = response.json().await?;
        Ok(thread_response)
    }
}

/// Feed response from API
#[derive(Debug, Deserialize, Serialize)]
pub struct FeedResponse {
    pub feed: Vec<FeedViewPost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Feed view post
#[derive(Debug, Deserialize, Serialize)]
pub struct FeedViewPost {
    pub post: Post,
}

/// Post data
#[derive(Debug, Deserialize, Serialize)]
pub struct Post {
    pub uri: String,
    pub cid: String,
    pub author: Author,
    pub record: Value,
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "likeCount")]
    pub like_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "replyCount")]
    pub reply_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "repostCount")]
    pub repost_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "quoteCount")]
    pub quote_count: Option<u32>,
}

/// Author data
#[derive(Debug, Deserialize, Serialize)]
pub struct Author {
    pub did: String,
    pub handle: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "displayName")]
    pub display_name: Option<String>,
}

/// Thread response from API
#[derive(Debug, Deserialize, Serialize)]
pub struct ThreadResponse {
    pub thread: ThreadView,
}

/// Thread view (can be ThreadViewPost or other types)
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ThreadView {
    Post(ThreadViewPost),
    NotFound(Value),
    Blocked(Value),
}

/// Thread view post
#[derive(Debug, Deserialize, Serialize)]
pub struct ThreadViewPost {
    pub post: Post,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Box<ThreadView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<Vec<ThreadView>>,
}
