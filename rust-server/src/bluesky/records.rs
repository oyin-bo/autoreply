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

        // Link (web URL) and timestamp
        if !self.uri.is_empty() {
            let post_url = format!(
                "https://bsky.app/profile/{}/post/{}",
                handle,
                self.uri.split('/').next_back().unwrap_or("")
            );
            markdown.push_str(&format!("**Link:** {}\n", post_url));
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
            for line in link_lines {
                markdown.push_str(&line);
            }
            markdown.push('\n');
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
                    markdown.push('\n');
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_profile() -> ProfileRecord {
        ProfileRecord {
            display_name: Some("Test User".to_string()),
            description: Some("A test user profile\nwith multiline description".to_string()),
            avatar: Some("https://example.com/avatar.jpg".to_string()),
            banner: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn create_test_post() -> PostRecord {
        PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/123".to_string(),
            cid: "bafy123test".to_string(),
            text: "Hello world! Check out this link: https://example.com".to_string(),
            created_at: "2024-01-01T12:00:00Z".to_string(),
            embeds: vec![],
            facets: vec![],
        }
    }

    #[test]
    fn test_profile_record_to_markdown() {
        let profile = create_test_profile();
        let markdown = profile.to_markdown("alice.bsky.social", "did:plc:test123");

        assert!(markdown.contains("# @alice.bsky.social (did:plc:test123)"));
        assert!(markdown.contains("**Display Name:** Test User"));
        assert!(markdown.contains("**Description:**"));
        assert!(markdown.contains("A test user profile"));
        assert!(markdown.contains("**Avatar:** ![Avatar](https://example.com/avatar.jpg)"));
        assert!(markdown.contains("**Stats:**"));
        assert!(markdown.contains("- Created: 2024-01-01T00:00:00Z"));
        assert!(markdown.contains("Raw Profile Data"));
    }

    #[test]
    fn test_profile_record_to_markdown_minimal() {
        let minimal_profile = ProfileRecord {
            display_name: None,
            description: None,
            avatar: None,
            banner: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let markdown = minimal_profile.to_markdown("minimal.bsky.social", "did:plc:minimal");

        assert!(markdown.contains("# @minimal.bsky.social (did:plc:minimal)"));
        assert!(!markdown.contains("**Display Name:**"));
        assert!(!markdown.contains("**Description:**"));
        assert!(!markdown.contains("**Avatar:**"));
        assert!(markdown.contains("**Stats:**"));
        assert!(markdown.contains("- Created: 2024-01-01T00:00:00Z"));
    }

    #[test]
    fn test_post_record_get_searchable_text_basic() {
        let post = create_test_post();
        let searchable = post.get_searchable_text();

        assert_eq!(searchable.len(), 1);
        assert_eq!(
            searchable[0],
            "Hello world! Check out this link: https://example.com"
        );
    }

    #[test]
    fn test_post_record_get_searchable_text_with_embeds() {
        let mut post = create_test_post();

        // Add external embed
        post.embeds.push(Embed::External {
            external: ExternalEmbed {
                uri: "https://example.com/article".to_string(),
                title: "Amazing Article".to_string(),
                description: "This is a great article about Rust".to_string(),
                thumb: None,
            },
        });

        // Add images embed
        post.embeds.push(Embed::Images {
            images: vec![
                ImageEmbed {
                    alt: Some("A beautiful sunset".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "bafy123".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 1024,
                    },
                },
                ImageEmbed {
                    alt: None,
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "bafy456".to_string(),
                        mime_type: "image/png".to_string(),
                        size: 2048,
                    },
                },
            ],
        });

        let searchable = post.get_searchable_text();

        assert_eq!(searchable.len(), 4);
        assert!(searchable
            .contains(&"Hello world! Check out this link: https://example.com".to_string()));
        assert!(searchable.contains(&"Amazing Article".to_string()));
        assert!(searchable.contains(&"This is a great article about Rust".to_string()));
        assert!(searchable.contains(&"A beautiful sunset".to_string()));
    }

    #[test]
    fn test_post_record_get_searchable_text_with_facets() {
        let mut post = create_test_post();

        // Add facets with links
        post.facets.push(Facet {
            index: FacetIndex {
                byte_start: 35,
                byte_end: 55,
            },
            features: vec![
                FacetFeature::Link {
                    uri: "https://example.com".to_string(),
                },
                FacetFeature::Tag {
                    tag: "rust".to_string(),
                },
            ],
        });

        let searchable = post.get_searchable_text();

        assert_eq!(searchable.len(), 2);
        assert!(searchable
            .contains(&"Hello world! Check out this link: https://example.com".to_string()));
        assert!(searchable.contains(&"https://example.com".to_string()));
    }

    #[test]
    fn test_post_record_to_markdown() {
        let post = create_test_post();
        let markdown = post.to_markdown("alice.bsky.social", "hello");

    assert!(markdown.contains("**Link:** https://bsky.app/profile/alice.bsky.social/post/123"));
        assert!(markdown.contains("**Created:** 2024-01-01T12:00:00Z"));
        assert!(markdown.contains("**Hello** world!"));
    }

    #[test]
    fn test_post_record_to_markdown_with_links() {
        let mut post = create_test_post();

        // Add external embed
        post.embeds.push(Embed::External {
            external: ExternalEmbed {
                uri: "https://example.com/article".to_string(),
                title: "Great Article".to_string(),
                description: "Amazing content".to_string(),
                thumb: None,
            },
        });

        // Add facet link
        post.facets.push(Facet {
            index: FacetIndex {
                byte_start: 0,
                byte_end: 5,
            },
            features: vec![FacetFeature::Link {
                uri: "https://facet-link.com".to_string(),
            }],
        });

        let markdown = post.to_markdown("alice.bsky.social", "hello");

        assert!(markdown.contains("**Links:**"));
        assert!(markdown.contains("- [Great Article](https://example.com/article)"));
        assert!(markdown.contains("- https://facet-link.com"));
    }

    #[test]
    fn test_post_record_to_markdown_with_images() {
        let mut post = create_test_post();

        post.embeds.push(Embed::Images {
            images: vec![
                ImageEmbed {
                    alt: Some("Sunset photo".to_string()),
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "bafy123".to_string(),
                        mime_type: "image/jpeg".to_string(),
                        size: 1024,
                    },
                },
                ImageEmbed {
                    alt: None,
                    image: BlobRef {
                        type_: "blob".to_string(),
                        ref_: "bafy456".to_string(),
                        mime_type: "image/png".to_string(),
                        size: 2048,
                    },
                },
            ],
        });

        let markdown = post.to_markdown("alice.bsky.social", "hello");

        assert!(markdown.contains("**Images:**"));
        assert!(markdown.contains("- Sunset photo"));
        assert!(markdown.contains("- Image 2"));
    }

    #[test]
    fn test_highlight_text_basic() {
        let text = "Hello world, this is a test";
        let result = highlight_text(text, "world");
        assert_eq!(result, "Hello **world**, this is a test");
    }

    #[test]
    fn test_highlight_text_case_insensitive() {
        let text = "Hello World, this is a TEST";
        let result = highlight_text(text, "world");
        assert_eq!(result, "Hello **World**, this is a TEST");

        let result = highlight_text(text, "test");
        assert_eq!(result, "Hello World, this is a **TEST**");
    }

    #[test]
    fn test_highlight_text_multiple_matches() {
        let text = "test test test";
        let result = highlight_text(text, "test");
        assert_eq!(result, "**test** **test** **test**");
    }

    #[test]
    fn test_highlight_text_no_match() {
        let text = "Hello world";
        let result = highlight_text(text, "xyz");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_highlight_text_empty_query() {
        let text = "Hello world";
        let result = highlight_text(text, "");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_highlight_text_partial_word() {
        let text = "programming";
        let result = highlight_text(text, "gram");
        assert_eq!(result, "pro**gram**ming");
    }

    #[test]
    fn test_blob_ref_serialization() {
        let blob = BlobRef {
            type_: "blob".to_string(),
            ref_: "bafy123456789".to_string(),
            mime_type: "image/jpeg".to_string(),
            size: 1024,
        };

        let json = serde_json::to_string(&blob).unwrap();
        assert!(json.contains("\"$type\":\"blob\""));
        assert!(json.contains("\"ref\":\"bafy123456789\""));
        assert!(json.contains("\"mimeType\":\"image/jpeg\""));
        assert!(json.contains("\"size\":1024"));
    }

    #[test]
    fn test_facet_feature_serialization() {
        let link = FacetFeature::Link {
            uri: "https://example.com".to_string(),
        };
        let mention = FacetFeature::Mention {
            did: "did:plc:test123".to_string(),
        };
        let tag = FacetFeature::Tag {
            tag: "rust".to_string(),
        };

        let link_json = serde_json::to_string(&link).unwrap();
        assert!(link_json.contains("\"$type\":\"app.bsky.richtext.facet#link\""));

        let mention_json = serde_json::to_string(&mention).unwrap();
        assert!(mention_json.contains("\"$type\":\"app.bsky.richtext.facet#mention\""));

        let tag_json = serde_json::to_string(&tag).unwrap();
        assert!(tag_json.contains("\"$type\":\"app.bsky.richtext.facet#tag\""));
    }

    #[test]
    fn test_embed_serialization() {
        let external = Embed::External {
            external: ExternalEmbed {
                uri: "https://example.com".to_string(),
                title: "Test".to_string(),
                description: "Test desc".to_string(),
                thumb: None,
            },
        };

        let json = serde_json::to_string(&external).unwrap();
        assert!(json.contains("\"$type\":\"app.bsky.embed.external\""));
        assert!(json.contains("\"uri\":\"https://example.com\""));
    }
}
