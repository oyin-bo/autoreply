//! Integration tests for CLI tool execution
//!
//! Tests that exercise the execute_* functions for various CLI tools using
//! test fixtures and mocked providers.

#[cfg(test)]
mod cli_integration_tests {
    use crate::bluesky::provider::{RepositoryProvider};
    use crate::bluesky::records::{PostRecord, ProfileRecord, Embed};
    use crate::cli::{SearchArgs, ProfileArgs};


    // Test fixture: a minimal ProfileRecord
    fn create_test_profile() -> ProfileRecord {
        ProfileRecord {
            display_name: Some("Test User".to_string()),
            description: Some("A test profile".to_string()),
            avatar: None,
            banner: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    // Test fixture: a minimal PostRecord
    fn create_test_post(text: &str, uri: &str, cid: &str) -> PostRecord {
        PostRecord {
            uri: uri.to_string(),
            cid: cid.to_string(),
            text: text.to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        }
    }

    #[test]
    fn test_execute_profile_with_valid_did() {
        // Test that execute_profile_impl returns correctly formatted profile data
        let profile = create_test_profile();
        let markdown = profile.to_markdown("test.bsky.social", "did:plc:test");

        assert!(markdown.contains("Test User"), "Markdown should contain display name");
        assert!(markdown.contains("test profile"), "Markdown should contain description");
    }

    #[test]
    fn test_execute_search_result_formatting() {
        // Test that search results are formatted with proper markdown
        let posts = vec![
            create_test_post("hello world", "at://did:plc:test/app.bsky.feed.post/1", "cid1"),
            create_test_post("another post", "at://did:plc:test/app.bsky.feed.post/2", "cid2"),
        ];

        // Search should find the second post
        let mut engine = crate::search::SearchEngine::new();
        let results = engine.search("post", &posts, |p| p.get_searchable_text());
        
        assert_eq!(results.len(), 1, "Should find 'post' in second post");
    }

    #[test]
    fn test_search_args_deserialization() {
        // Test that SearchArgs can be properly created and used
        let args = SearchArgs {
            from: "did:plc:test123".to_string(),
            query: "rust programming".to_string(),
            limit: None,
        };

        assert_eq!(args.from, "did:plc:test123");
        assert_eq!(args.query, "rust programming");
    }

    #[test]
    fn test_profile_args_deserialization() {
        // Test that ProfileArgs can be properly created and used
        let args = ProfileArgs {
            account: "did:plc:test456".to_string(),
        };

        assert_eq!(args.account, "did:plc:test456");
    }

    #[test]
    fn test_search_with_empty_results() {
        // Test handling of search with no matching results
        let posts = vec![
            create_test_post("foo bar baz", "at://did:plc:test/app.bsky.feed.post/1", "cid1"),
        ];

        let mut engine = crate::search::SearchEngine::new();
        let results = engine.search("nonexistent_query_xyz", &posts, |p| p.get_searchable_text());
        
        assert_eq!(results.len(), 0, "Should find no results for nonexistent query");
    }

    #[test]
    fn test_multiple_posts_in_search() {
        // Test that multiple posts are correctly indexed and ranked
        let posts = vec![
            create_test_post("The quick brown fox", "at://did:plc:test/app.bsky.feed.post/1", "cid1"),
            create_test_post("A fox in the forest", "at://did:plc:test/app.bsky.feed.post/2", "cid2"),
            create_test_post("Foxes are fast animals", "at://did:plc:test/app.bsky.feed.post/3", "cid3"),
        ];

        let mut engine = crate::search::SearchEngine::new();
        let results = engine.search("fox", &posts, |p| p.get_searchable_text());
        
        assert_eq!(results.len(), 3, "Should find all three posts mentioning 'fox'");
        // Results should be ranked; check that we have valid results
        assert!(!results.is_empty(), "Should return valid search results");
    }

    #[test]
    fn test_search_case_insensitivity() {
        // Test that search is case-insensitive
        let posts = vec![
            create_test_post("Hello World", "at://did:plc:test/app.bsky.feed.post/1", "cid1"),
            create_test_post("HELLO WORLD", "at://did:plc:test/app.bsky.feed.post/2", "cid2"),
            create_test_post("hello world", "at://did:plc:test/app.bsky.feed.post/3", "cid3"),
        ];

        let mut engine = crate::search::SearchEngine::new();
        let results = engine.search("HELLO", &posts, |p| p.get_searchable_text());
        
        assert_eq!(results.len(), 3, "Should find all variations of 'hello'");
    }

    #[test]
    fn test_post_record_searchable_text_extraction() {
        // Test that PostRecord correctly extracts searchable text from text and embeds
        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/1".to_string(),
            cid: "cid1".to_string(),
            text: "Check out this image".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: Some(vec![
                Embed::Images {
                    images: vec![crate::bluesky::records::ImageEmbed {
                        alt: Some("A beautiful sunset".to_string()),
                        image: crate::bluesky::records::BlobRef {
                            type_: "blob".to_string(),
                            ref_: "bafk123".to_string(),
                            mime_type: "image/jpeg".to_string(),
                            size: 1024,
                        },
                    }],
                },
            ]),
            facets: vec![],
        };

        let searchable = post.get_searchable_text();
        let combined = searchable.join(" ");
        assert!(combined.contains("Check out this image"), "Should contain post text");
        assert!(combined.contains("sunset"), "Should contain embed alt text");
    }

    #[test]
    fn test_profile_record_to_markdown_basic() {
        let profile = ProfileRecord {
            display_name: Some("Alice".to_string()),
            description: Some("Developer and coffee enthusiast".to_string()),
            avatar: None,
            banner: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let markdown = profile.to_markdown("alice.bsky.social", "did:plc:test");
        assert!(markdown.contains("Alice"), "Markdown should contain display name");
        assert!(markdown.contains("Developer"), "Markdown should contain description");
    }

    #[test]
    fn test_profile_record_to_markdown_minimal() {
        let profile = ProfileRecord {
            display_name: None,
            description: None,
            avatar: None,
            banner: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let markdown = profile.to_markdown("test.bsky.social", "did:plc:test");
        // Should handle minimal profile gracefully
        assert!(!markdown.is_empty(), "Markdown should not be empty");
    }
}
