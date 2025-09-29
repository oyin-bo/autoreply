//! AT Protocol record types
//!
//! Defines the data structures for Bluesky records as specified in docs/7.1-rust.md

use serde::{Deserialize, Serialize};

/// Profile record from app.bsky.actor.profile collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRecord {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Post record from app.bsky.feed.post collection  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecord {
    pub uri: String,
    #[serde(default)]
    pub cid: String,
    pub text: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    #[serde(default)]
    pub facets: Vec<Facet>,
}

/// Embed types in posts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "$type")]
pub enum Embed {
    #[serde(rename = "app.bsky.embed.images")]
    Images { images: Vec<ImageEmbed> },
    #[serde(rename = "app.bsky.embed.external")]
    External { external: ExternalEmbed },
    #[serde(rename = "app.bsky.embed.record")]
    Record { record: RecordEmbed },
    #[serde(rename = "app.bsky.embed.recordWithMedia")]
    RecordWithMedia {
        record: RecordEmbed,
        media: Box<Embed>,
    },
}

/// Image embed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEmbed {
    pub alt: Option<String>,
    pub image: BlobRef,
}

/// External link embed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEmbed {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub thumb: Option<BlobRef>,
}

/// Record embed (quote posts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordEmbed {
    pub uri: String,
    pub cid: String,
}

/// Blob reference for images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobRef {
    #[serde(rename = "$type")]
    pub type_: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: u64,
}

/// Text facets (links, mentions, hashtags)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facet {
    pub index: FacetIndex,
    pub features: Vec<FacetFeature>,
}

/// Facet byte index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetIndex {
    #[serde(rename = "byteStart")]
    pub byte_start: u32,
    #[serde(rename = "byteEnd")]
    pub byte_end: u32,
}

/// Facet features (what the text represents)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "$type")]
pub enum FacetFeature {
    #[serde(rename = "app.bsky.richtext.facet#mention")]
    Mention { did: String },
    #[serde(rename = "app.bsky.richtext.facet#link")]
    Link { uri: String },
    #[serde(rename = "app.bsky.richtext.facet#tag")]
    Tag { tag: String },
}

impl ProfileRecord {
    /// Convert to markdown format as specified in docs
    pub fn to_markdown(&self, handle: &str, did: &str) -> String {
        let mut markdown = format!("# @{} ({})\n\n", handle, did);

        if let Some(display_name) = &self.display_name {
            markdown.push_str(&format!("**Display Name:** {}\n\n", display_name));
        }

        if let Some(description) = &self.description {
            markdown.push_str("**Description:**\n");
            markdown.push_str(description);
            markdown.push_str("\n\n");
        }

        if let Some(avatar) = &self.avatar {
            markdown.push_str(&format!("**Avatar:** ![Avatar]({})\n\n", avatar));
        }

        markdown.push_str("**Stats:**\n");
        markdown.push_str(&format!("- Created: {}\n", self.created_at));

        // Add raw profile data
        markdown.push_str("\n<details>\n<summary>Raw Profile Data</summary>\n\n```json\n");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            markdown.push_str(&json);
        }
        markdown.push_str("\n```\n</details>\n");

        markdown
    }
}

impl PostRecord {
    /// Get searchable text from the post
    pub fn get_searchable_text(&self) -> Vec<String> {
        let mut texts = vec![self.text.clone()];

        // Add embed text
        for embed in &self.embeds {
            match embed {
                Embed::Images { images } => {
                    for img in images {
                        if let Some(alt) = &img.alt {
                            texts.push(alt.clone());
                        }
                    }
                }
                Embed::External { external } => {
                    texts.push(external.title.clone());
                    texts.push(external.description.clone());
                }
                _ => {}
            }
        }

        // Add facet link URIs
        for facet in &self.facets {
            for feat in &facet.features {
                if let FacetFeature::Link { uri } = feat {
                    texts.push(uri.clone());
                }
            }
        }

        texts
    }

    /// Convert to markdown format for search results
    pub fn to_markdown(&self, handle: &str, query: &str) -> String {
        let mut markdown = String::new();

        // Post URI and timestamp
        if !self.uri.is_empty() {
            let post_url = format!("https://bsky.app/profile/{}/post/{}", 
                handle, 
                self.uri.split('/').last().unwrap_or("")
            );
            markdown.push_str(&format!("**URI:** [{}]({})\n", self.uri, post_url));
        }
        
        markdown.push_str(&format!("**Created:** {}\n\n", self.created_at));

        // Highlighted post text
        let highlighted_text = highlight_text(&self.text, query);
        markdown.push_str(&highlighted_text);
        markdown.push_str("\n\n");

        // Collect links from external embeds and facet link features
        let mut link_lines: Vec<String> = Vec::new();
        for embed in &self.embeds {
            if let Embed::External { external } = embed {
                link_lines.push(format!("- [{}]({})\n", external.title, external.uri));
            }
        }
        for facet in &self.facets {
            for feat in &facet.features {
                if let FacetFeature::Link { uri } = feat {
                    link_lines.push(format!("- {}\n", uri));
                }
            }
        }
        if !link_lines.is_empty() {
            markdown.push_str("**Links:**\n");
            for line in link_lines { markdown.push_str(&line); }
            markdown.push_str("\n");
        }

        // Add images alt text (no URLs in scope)
        if !self.embeds.is_empty() {
            for embed in &self.embeds {
                if let Embed::Images { images } = embed {
                    markdown.push_str("**Images:**\n");
                    for (i, img) in images.iter().enumerate() {
                        let default_alt = format!("Image {}", i + 1);
                        let alt_text = img.alt.as_deref().unwrap_or(&default_alt);
                        markdown.push_str(&format!("- {}\n", alt_text));
                    }
                    markdown.push_str("\n");
                }
            }
        }

        markdown
    }
}

/// Highlight query matches in text with **bold** markdown
fn highlight_text(text: &str, query: &str) -> String {
    if query.is_empty() {
        return text.to_string();
    }

    // Simple case-insensitive highlighting
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    
    if !lower_text.contains(&lower_query) {
        return text.to_string();
    }

    let mut result = String::new();
    let mut last_end = 0;
    
    while let Some(start) = lower_text[last_end..].find(&lower_query) {
        let absolute_start = last_end + start;
        let absolute_end = absolute_start + query.len();
        
        // Add text before match
        result.push_str(&text[last_end..absolute_start]);
        
        // Add highlighted match
        result.push_str("**");
        result.push_str(&text[absolute_start..absolute_end]);
        result.push_str("**");
        
        last_end = absolute_end;
    }
    
    // Add remaining text
    result.push_str(&text[last_end..]);
    
    result
}