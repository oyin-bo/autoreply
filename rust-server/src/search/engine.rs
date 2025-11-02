//! Search Engine Integration
//!
//! Ties together query parsing, fuzzy matching, and ranking
//! to provide a complete search solution.

use super::fuzzy::FuzzyMatcher;
use super::parser::{ParsedQuery, QueryParser};
use super::ranking::{MatchScore, ScoringWeights};

/// Search result with content and score
#[derive(Debug, Clone)]
pub struct SearchResult<T> {
    /// The matched item
    pub item: T,
    /// Match score
    pub score: MatchScore,
    /// Which query terms matched
    #[allow(dead_code)]
    pub matched_terms: Vec<String>,
}

/// Search engine that combines parsing, matching, and ranking
pub struct SearchEngine {
    fuzzy_matcher: FuzzyMatcher,
    scoring_weights: ScoringWeights,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    /// Create a new search engine with default configuration
    pub fn new() -> Self {
        Self {
            fuzzy_matcher: FuzzyMatcher::new(),
            scoring_weights: ScoringWeights::default(),
        }
    }

    /// Create search engine with custom weights
    #[allow(dead_code)]
    pub fn with_weights(weights: ScoringWeights) -> Self {
        Self {
            fuzzy_matcher: FuzzyMatcher::new(),
            scoring_weights: weights,
        }
    }

    /// Search items with a query string
    ///
    /// The extract_text function should return the searchable text for each item.
    /// Returns results sorted by relevance (highest score first).
    pub fn search<T, F>(
        &mut self,
        query: &str,
        items: &[T],
        extract_text: F,
    ) -> Vec<SearchResult<T>>
    where
        T: Clone,
        F: Fn(&T) -> Vec<String>,
    {
        // Parse the query
        let parsed = QueryParser::parse(query);

        let mut results = Vec::new();

        // Search each item
        for item in items {
            let searchable_texts = extract_text(item);

            if let Some(result) = self.match_item(item.clone(), &parsed, &searchable_texts) {
                results.push(result);
            }
        }

        // Sort by score (descending)
        results.sort_by(|a, b| {
            b.score
                .final_score
                .partial_cmp(&a.score.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Match a single item against the parsed query
    fn match_item<T>(
        &mut self,
        item: T,
        parsed: &ParsedQuery,
        searchable_texts: &[String],
    ) -> Option<SearchResult<T>> {
        let mut best_score: Option<MatchScore> = None;
        let mut matched_terms = Vec::new();

        // Check quoted phrases first (exact match required)
        if !parsed.quoted_phrases.is_empty() {
            let mut all_quoted_found = true;

            for phrase in &parsed.quoted_phrases {
                let mut phrase_found = false;

                for text in searchable_texts {
                    if self.fuzzy_matcher.exact_match(text, phrase) {
                        phrase_found = true;
                        matched_terms.push(phrase.clone());

                        // Create a very high score for exact quoted match
                        let score = MatchScore::exact_match(text.len(), &self.scoring_weights);

                        best_score = Some(match best_score {
                            Some(existing) if existing.final_score > score.final_score => existing,
                            _ => score,
                        });

                        break;
                    }
                }

                if !phrase_found {
                    all_quoted_found = false;
                }
            }

            // If any quoted phrase is missing, this item doesn't match
            if !all_quoted_found {
                return None;
            }
        }

        // Try whole query match
        if let Some(score) = self.match_text(&parsed.whole_query, searchable_texts) {
            matched_terms.push(parsed.whole_query.clone());
            best_score = Some(match best_score {
                Some(existing) if existing.final_score > score.final_score => existing,
                _ => score,
            });
        }

        // Try individual word matches
        for word in &parsed.individual_words {
            if let Some(score) = self.match_text(word, searchable_texts) {
                matched_terms.push(word.clone());

                // Individual word matches get lower weight than whole query
                let mut adjusted_score = score;
                adjusted_score.final_score *= 0.7; // Penalty for word-only match

                best_score = Some(match best_score {
                    Some(existing) if existing.final_score > adjusted_score.final_score => existing,
                    _ => adjusted_score,
                });
            }
        }

        best_score.map(|score| SearchResult {
            item,
            score,
            matched_terms,
        })
    }

    /// Match text against searchable content
    fn match_text(&mut self, needle: &str, haystacks: &[String]) -> Option<MatchScore> {
        let mut best_match: Option<MatchScore> = None;

        for haystack in haystacks {
            // Try exact match first
            if self.fuzzy_matcher.exact_match(haystack, needle) {
                let score = MatchScore::exact_match(haystack.len(), &self.scoring_weights);

                best_match = Some(match best_match {
                    Some(existing) if existing.final_score > score.final_score => existing,
                    _ => score,
                });
                continue;
            }

            // Try fuzzy match
            if let Some(fuzzy_match) = self.fuzzy_matcher.fuzzy_match(haystack, needle) {
                let proximity_score = self
                    .fuzzy_matcher
                    .calculate_proximity_score(&fuzzy_match.positions);

                let score = MatchScore::calculate(
                    &fuzzy_match,
                    proximity_score,
                    false, // not exact
                    false, // not exact unicode (would need to check)
                    &self.scoring_weights,
                );

                best_match = Some(match best_match {
                    Some(existing) if existing.final_score > score.final_score => existing,
                    _ => score,
                });
            }
        }

        best_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestPost {
        text: String,
    }

    #[test]
    fn test_search_basic() {
        let mut engine = SearchEngine::new();

        let posts = vec![
            TestPost {
                text: "Hello world".to_string(),
            },
            TestPost {
                text: "Goodbye world".to_string(),
            },
            TestPost {
                text: "Hello there".to_string(),
            },
        ];

        let results = engine.search("hello", &posts, |p| vec![p.text.clone()]);

        assert_eq!(results.len(), 2);
        assert!(results[0].item.text.contains("Hello"));
    }

    #[test]
    fn test_search_quoted() {
        let mut engine = SearchEngine::new();

        let posts = vec![
            TestPost {
                text: "Hello world".to_string(),
            },
            TestPost {
                text: "Hello there world".to_string(),
            },
            TestPost {
                text: "World hello".to_string(),
            },
        ];

        let results = engine.search(r#""hello world""#, &posts, |p| vec![p.text.clone()]);

        // Only "Hello world" should match the exact phrase
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.text, "Hello world");
    }

    #[test]
    fn test_search_stop_words() {
        let mut engine = SearchEngine::new();

        let posts = vec![
            TestPost {
                text: "The cat sat on the mat".to_string(),
            },
            TestPost {
                text: "A dog ran".to_string(),
            },
        ];

        let results = engine.search("cat mat", &posts, |p| vec![p.text.clone()]);

        // Should find the cat post despite "on" and "the" being stop words
        assert_eq!(results.len(), 1);
        assert!(results[0].item.text.contains("cat"));
    }

    #[test]
    fn test_search_fuzzy() {
        let mut engine = SearchEngine::new();

        let posts = vec![
            TestPost {
                text: "programming".to_string(),
            },
            TestPost {
                text: "programmer".to_string(),
            },
            TestPost {
                text: "program".to_string(),
            },
        ];

        let results = engine.search("prog", &posts, |p| vec![p.text.clone()]);

        // All should match with fuzzy matching
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_no_match() {
        let mut engine = SearchEngine::new();

        let posts = vec![TestPost {
            text: "Hello world".to_string(),
        }];

        let results = engine.search("xyz", &posts, |p| vec![p.text.clone()]);

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_ranking() {
        let mut engine = SearchEngine::new();

        let posts = vec![
            TestPost {
                text: "car".to_string(),
            }, // Full word match
            TestPost {
                text: "carton".to_string(),
            }, // Beginning of word
            TestPost {
                text: "scar".to_string(),
            }, // End of word
            TestPost {
                text: "scary".to_string(),
            }, // Middle of word
        ];

        let results = engine.search("car", &posts, |p| vec![p.text.clone()]);

        assert_eq!(results.len(), 4);
        // Full word match should rank highest
        assert_eq!(results[0].item.text, "car");
    }
}
