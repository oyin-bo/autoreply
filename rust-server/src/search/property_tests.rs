use proptest::prelude::*;
use proptest::char::range as char_range;
use crate::bluesky::records::{PostRecord, Facet, FacetFeature, FacetIndex};
use crate::tools::search::format_search_results;

// Property test: facet Link features should appear in searchable text
proptest! {
    #[test]
    fn facet_link_feature_in_searchable_text(uri in proptest::collection::vec(any::<char>(), 1..20).prop_map(|v| v.into_iter().collect::<String>())) {
        // Build a facet that represents a link feature
        let facet = Facet {
            index: FacetIndex { byte_start: 0u32, byte_end: 0u32 },
            features: vec![FacetFeature::Link { uri: uri.clone() }],
        };

        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/prop1".to_string(),
            cid: "cid_prop1".to_string(),
            text: "".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![facet],
        };

        let texts = post.get_searchable_text();
        prop_assert!(texts.contains(&uri));
    }
}

// Property test: single newline between matches should allow merge, double newline should not
proptest! {
    #[test]
    fn highlight_newline_merge_behaviour(a in char_range('a'..='z'), b in char_range('a'..='z')) {
        let a_s = a.to_string();
        let b_s = b.to_string();

        // Single newline
        let text_single = format!("{}\n{}", a_s, b_s);
        let post_single = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/prop1".to_string(),
            cid: "cid_prop1".to_string(),
            text: text_single.clone(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let md_single = format_search_results(&vec![&post_single], "me", &format!("{} {}", a_s, b_s));
        let expected_single = format!("**{}\n{}**", a_s, b_s);
        prop_assert!(md_single.contains(&expected_single));

        // Double newline (paragraph break)
        let text_double = format!("{}\n\n{}", a_s, b_s);
        let post_double = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/prop2".to_string(),
            cid: "cid_prop2".to_string(),
            text: text_double.clone(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            embeds: None,
            facets: vec![],
        };
        let md_double = format_search_results(&vec![&post_double], "me", &format!("{} {}", a_s, b_s));
        let expected_double = format!("**{}\n\n{}**", a_s, b_s);
        // Should NOT contain merged span with double newline
        prop_assert!(!md_double.contains(&expected_double));
    }
}
use proptest::prelude::*;
use proptest::char::range as char_range;
use crate::bluesky::records::{PostRecord, Facet, FacetFeature, FacetIndex};
use crate::tools::search::format_search_results;

// Property test: facet Link features should appear in searchable text
proptest! {
    #[test]
    fn facet_link_feature_in_searchable_text(uri in proptest::collection::vec(any::<char>(), 1..20).prop_map(|v| v.into_iter().collect::<String>())) {
        // Build a facet that represents a link feature
        let facet = Facet {
            index: FacetIndex { byte_start: 0u32, byte_end: 0u32 },
            features: vec![FacetFeature::Link { uri: uri.clone() }],
        };

        let post = PostRecord {
            uri: "at://did:plc:test/app.bsky.feed.post/prop1".to_string(),
            use proptest::prelude::*;
            use proptest::char::range as char_range;
            use crate::bluesky::records::{PostRecord, Facet, FacetFeature, FacetIndex};
            use crate::tools::search::format_search_results;

            // Property test: facet Link features should appear in searchable text
            proptest! {
                #[test]
                fn facet_link_feature_in_searchable_text(uri in proptest::collection::vec(any::<char>(), 1..20).prop_map(|v| v.into_iter().collect::<String>())) {
                    // Build a facet that represents a link feature
                    let facet = Facet {
                        index: FacetIndex { byte_start: 0u32, byte_end: 0u32 },
                        features: vec![FacetFeature::Link { uri: uri.clone() }],
                    };

                    let post = PostRecord {
                        uri: "at://did:plc:test/app.bsky.feed.post/prop1".to_string(),
                        cid: "cid_prop1".to_string(),
                        text: "".to_string(),
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        embeds: None,
                        facets: vec![facet],
                    };

                    let texts = post.get_searchable_text();
                    prop_assert!(texts.contains(&uri));
                }
            }

            // Property test: single newline between matches should allow merge, double newline should not
            proptest! {
                #[test]
                fn highlight_newline_merge_behaviour(a in char_range('a'..='z'), b in char_range('a'..='z')) {
                    let a_s = a.to_string();
                    let b_s = b.to_string();

                    // Single newline
                    let text_single = format!("{}\n{}", a_s, b_s);
                    let post_single = PostRecord {
                        uri: "at://did:plc:test/app.bsky.feed.post/prop1".to_string(),
                        cid: "cid_prop1".to_string(),
                        text: text_single.clone(),
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        embeds: None,
                        facets: vec![],
                    };
                    let md_single = format_search_results(&vec![&post_single], "me", &format!("{} {}", a_s, b_s));
                    let expected_single = format!("**{}\n{}**", a_s, b_s);
                    prop_assert!(md_single.contains(&expected_single));

                    // Double newline (paragraph break)
                    let text_double = format!("{}\n\n{}", a_s, b_s);
                    let post_double = PostRecord {
                        uri: "at://did:plc:test/app.bsky.feed.post/prop2".to_string(),
                        cid: "cid_prop2".to_string(),
                        text: text_double.clone(),
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        embeds: None,
                        facets: vec![],
                    };
                    let md_double = format_search_results(&vec![&post_double], "me", &format!("{} {}", a_s, b_s));
                    let expected_double = format!("**{}\n\n{}**", a_s, b_s);
                    // Should NOT contain merged span with double newline
                    prop_assert!(!md_double.contains(&expected_double));
                }
            }
    proptest! {
