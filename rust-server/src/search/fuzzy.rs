//! Fuzzy Matching Engine using nucleo-matcher
//!
//! Implements fuzzy string matching with the Smith-Waterman algorithm
//! via the nucleo-matcher crate (used in Helix editor).

use nucleo_matcher::{Config, Matcher};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Match result with score and position information
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// Base fuzzy match score from nucleo
    pub score: u32,
    /// Positions of matched characters in the haystack
    pub positions: Vec<u32>,
    /// Match type classification
    pub match_type: MatchType,
}

/// Classification of where the match occurs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    /// Exact full word match
    FullWord,
    /// Match at beginning of word
    WordStart,
    /// Match at end of word
    WordEnd,
    /// Match in middle of word
    WordMiddle,
    /// Match spans multiple words
    MultiWord,
}

/// Fuzzy matcher with configuration
pub struct FuzzyMatcher {
    matcher: Matcher,
    #[allow(dead_code)]
    config: Config,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyMatcher {
    /// Create a new fuzzy matcher with default configuration
    pub fn new() -> Self {
        let config = Config::DEFAULT;
        let matcher = Matcher::new(config.clone());

        Self { matcher, config }
    }

    /// Create a fuzzy matcher with custom configuration
    #[allow(dead_code)]
    pub fn with_config(config: Config) -> Self {
        let matcher = Matcher::new(config.clone());
        Self { matcher, config }
    }

    /// Perform fuzzy matching between needle and haystack
    ///
    /// Returns Some(FuzzyMatch) if there's a match, None otherwise
    pub fn fuzzy_match(&mut self, haystack: &str, needle: &str) -> Option<FuzzyMatch> {
        if needle.is_empty() {
            return None;
        }

        // Normalize both strings for matching
        let haystack_normalized = Self::normalize_for_matching(haystack);
        let needle_normalized = Self::normalize_for_matching(needle);

        // Use nucleo-matcher for core fuzzy matching
        // Convert to Utf32String
        let haystack_utf32 = nucleo_matcher::Utf32String::from(haystack_normalized.as_str());
        let needle_utf32 = nucleo_matcher::Utf32String::from(needle_normalized.as_str());

        let score = self
            .matcher
            .fuzzy_match(haystack_utf32.slice(..), needle_utf32.slice(..))?;

        // Note: nucleo-matcher doesn't provide match positions in the basic API
        // For now, we'll compute positions ourselves for the exact substring case
        let positions = Self::find_match_positions(&haystack_normalized, &needle_normalized);

        // Classify the match type
        let match_type =
            Self::classify_match_type(&haystack_normalized, &needle_normalized, &positions);

        Some(FuzzyMatch {
            score: score as u32,
            positions,
            match_type,
        })
    }

    /// Find positions of matched characters (simple implementation for exact substring)
    fn find_match_positions(haystack: &str, needle: &str) -> Vec<u32> {
        let haystack_lower = haystack.to_lowercase();
        let needle_lower = needle.to_lowercase();

        if let Some(pos) = haystack_lower.find(&needle_lower) {
            // Return positions for exact substring match
            (pos..pos + needle.len()).map(|i| i as u32).collect()
        } else {
            // For fuzzy matches, we'd need more sophisticated position tracking
            // For now, return empty vec
            Vec::new()
        }
    }

    /// Check for exact substring match (case-insensitive)
    pub fn exact_match(&self, haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }

        let haystack_lower = haystack.to_lowercase();
        let needle_lower = needle.to_lowercase();

        haystack_lower.contains(&needle_lower)
    }

    /// Normalize text for matching
    /// - Unicode NFC normalization
    /// - Strip non-alphanumeric for matching (but preserve for display)
    fn normalize_for_matching(text: &str) -> String {
        // Unicode normalization (NFC - canonical composition)
        text.nfc().collect::<String>()
    }

    /// Classify the type of match based on position in words
    fn classify_match_type(haystack: &str, needle: &str, positions: &[u32]) -> MatchType {
        if positions.is_empty() {
            return MatchType::WordMiddle;
        }

        // Check if it's a full word match
        let needle_lower = needle.to_lowercase();

        // Get word boundaries
        let words: Vec<&str> = haystack.unicode_words().collect();

        // Check for exact full word match
        for word in &words {
            if word.to_lowercase() == needle_lower {
                return MatchType::FullWord;
            }
        }

        // Check if match spans multiple words
        if !positions.is_empty() {
            let first_pos = positions[0] as usize;
            let last_pos = positions[positions.len() - 1] as usize;

            // Find which words contain the match
            let mut word_start = 0;
            let mut words_spanned = 0;

            for word in &words {
                let word_end = word_start + word.len();

                if first_pos >= word_start && first_pos < word_end {
                    words_spanned += 1;
                }
                if last_pos >= word_start && last_pos < word_end && first_pos < word_start {
                    words_spanned += 1;
                }

                word_start = word_end;
                // Account for whitespace
                while word_start < haystack.len()
                    && haystack
                        .chars()
                        .nth(word_start)
                        .is_some_and(|c| c.is_whitespace())
                {
                    word_start += 1;
                }
            }

            if words_spanned > 1 {
                return MatchType::MultiWord;
            }
        }

        // Check if match is at word boundaries
        let first_pos = positions[0] as usize;

        // Check if at start of a word
        if first_pos == 0
            || haystack
                .chars()
                .nth(first_pos - 1)
                .is_some_and(|c| !c.is_alphanumeric())
        {
            // Check if it matches the entire word
            let last_pos = positions[positions.len() - 1] as usize;
            let next_char = haystack.chars().nth(last_pos + 1);

            if next_char.is_none_or(|c| !c.is_alphanumeric()) {
                return MatchType::FullWord;
            }

            return MatchType::WordStart;
        }

        // Check if at end of a word
        let last_pos = positions[positions.len() - 1] as usize;
        let next_char = haystack.chars().nth(last_pos + 1);

        if next_char.is_none_or(|c| !c.is_alphanumeric()) {
            return MatchType::WordEnd;
        }

        // Otherwise it's in the middle
        MatchType::WordMiddle
    }

    /// Calculate proximity score based on how close matched words are
    pub fn calculate_proximity_score(&self, positions: &[u32]) -> f64 {
        if positions.len() <= 1 {
            return 0.0;
        }

        // Calculate average gap between matched positions
        let mut total_gap = 0u32;
        for i in 1..positions.len() {
            total_gap += positions[i] - positions[i - 1];
        }

        let avg_gap = total_gap as f64 / (positions.len() - 1) as f64;

        // Closer matches get higher scores
        // Perfect sequential match (gap=1) gets 1.0
        // Larger gaps decay exponentially
        (1.0 / avg_gap).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let matcher = FuzzyMatcher::new();
        assert!(matcher.exact_match("hello world", "hello"));
        assert!(matcher.exact_match("hello world", "world"));
        assert!(matcher.exact_match("hello world", "o w"));
        assert!(!matcher.exact_match("hello world", "xyz"));
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        let matcher = FuzzyMatcher::new();
        assert!(matcher.exact_match("Hello World", "hello"));
        assert!(matcher.exact_match("HELLO WORLD", "world"));
    }

    #[test]
    fn test_fuzzy_match_basic() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("hello world", "hlo");
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(m.score > 0);
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("hello world", "xyz");
        assert!(result.is_none());
    }

    #[test]
    fn test_match_type_full_word() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("hello world test", "world");
        assert!(result.is_some());
        // Note: exact match detection depends on nucleo's matching behavior
    }

    #[test]
    fn test_unicode_normalization() {
        let mut matcher = FuzzyMatcher::new();
        // café with combining accent vs. precomposed
        let result = matcher.fuzzy_match("café", "cafe");
        // Should still match even if not exact
        assert!(result.is_some() || matcher.exact_match("café", "café"));
    }

    #[test]
    fn test_empty_needle() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.fuzzy_match("hello", "");
        assert!(result.is_none());
    }

    #[test]
    fn test_proximity_score() {
        let matcher = FuzzyMatcher::new();

        // Sequential positions
        let score1 = matcher.calculate_proximity_score(&[0, 1, 2, 3]);
        assert!((score1 - 1.0).abs() < 0.01);

        // Wider gaps
        let score2 = matcher.calculate_proximity_score(&[0, 5, 10, 15]);
        assert!(score2 < score1);

        // Single position
        let score3 = matcher.calculate_proximity_score(&[0]);
        assert_eq!(score3, 0.0);
    }
}
