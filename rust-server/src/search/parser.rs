//! Query Parser & Preprocessor
//!
//! Parses and tokenizes search queries, extracts quoted text,
//! identifies special patterns, and filters stop words.

use unicode_segmentation::UnicodeSegmentation;

/// Stop words that should be excluded from individual word searches
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is", "it",
    "its", "of", "on", "or", "that", "the", "to", "was", "will", "with", "i", "you",
];

/// Parsed and processed search query
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQuery {
    /// Original unmodified query
    pub original: String,
    /// Whole query for full-text search
    pub whole_query: String,
    /// Individual words (excluding stop words and quoted parts)
    pub individual_words: Vec<String>,
    /// Exact match requirements from quoted text
    pub quoted_phrases: Vec<String>,
}

/// Query parser and preprocessor
pub struct QueryParser;

impl QueryParser {
    /// Parse a search query into its components
    pub fn parse(query: &str) -> ParsedQuery {
        let original = query.to_string();

        // Extract quoted phrases
        let (quoted_phrases, query_without_quotes) = Self::extract_quoted_phrases(query);

        // The whole query is the original, but we'll use it for full-text matching
        let whole_query = query.to_string();

        // Tokenize the query without quoted parts
        let individual_words = Self::tokenize_and_filter(&query_without_quotes);

        ParsedQuery {
            original,
            whole_query,
            individual_words,
            quoted_phrases,
        }
    }

    /// Extract quoted phrases (both single and double quotes)
    /// Returns (quoted_phrases, query_with_quotes_removed)
    fn extract_quoted_phrases(query: &str) -> (Vec<String>, String) {
        let mut phrases = Vec::new();
        let mut remaining = String::new();
        let mut chars = query.chars().peekable();

        while let Some(&ch) = chars.peek() {
            if ch == '"' || ch == '\'' {
                let quote_char = ch;
                chars.next(); // consume opening quote

                let mut phrase = String::new();
                let mut found_closing = false;

                while let Some(&ch) = chars.peek() {
                    if ch == quote_char {
                        chars.next(); // consume closing quote
                        found_closing = true;
                        break;
                    } else if ch == '\\' {
                        // Handle escaped quotes
                        chars.next();
                        if let Some(&next_ch) = chars.peek() {
                            phrase.push(next_ch);
                            chars.next();
                        }
                    } else {
                        phrase.push(ch);
                        chars.next();
                    }
                }

                if found_closing && !phrase.is_empty() {
                    phrases.push(phrase);
                } else if !found_closing {
                    // Unclosed quote - treat the quote literally
                    remaining.push(quote_char);
                    remaining.push_str(&phrase);
                }
            } else {
                remaining.push(ch);
                chars.next();
            }
        }

        (phrases, remaining)
    }

    /// Tokenize text into words and filter stop words
    fn tokenize_and_filter(text: &str) -> Vec<String> {
        text.unicode_words()
            .map(|w| w.to_lowercase())
            .filter(|w| !Self::is_stop_word(w))
            .collect()
    }

    /// Check if a word is a stop word
    fn is_stop_word(word: &str) -> bool {
        STOP_WORDS.contains(&word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let parsed = QueryParser::parse("hello world");
        assert_eq!(parsed.original, "hello world");
        assert_eq!(parsed.whole_query, "hello world");
        assert_eq!(parsed.individual_words, vec!["hello", "world"]);
        assert!(parsed.quoted_phrases.is_empty());
    }

    #[test]
    fn test_quoted_phrases() {
        let parsed = QueryParser::parse(r#"hello "exact match" world"#);
        assert_eq!(parsed.quoted_phrases, vec!["exact match"]);
        assert_eq!(parsed.individual_words, vec!["hello", "world"]);
    }

    #[test]
    fn test_single_quotes() {
        let parsed = QueryParser::parse("hello 'exact match' world");
        assert_eq!(parsed.quoted_phrases, vec!["exact match"]);
        assert_eq!(parsed.individual_words, vec!["hello", "world"]);
    }

    #[test]
    fn test_stop_words_filtering() {
        let parsed = QueryParser::parse("the cat and the dog");
        // "the" and "and" should be filtered out
        assert_eq!(parsed.individual_words, vec!["cat", "dog"]);
    }

    #[test]
    fn test_multiple_quoted_phrases() {
        let parsed = QueryParser::parse(r#""first phrase" and "second phrase""#);
        assert_eq!(parsed.quoted_phrases, vec!["first phrase", "second phrase"]);
        // "and" is a stop word and should be excluded
        assert!(parsed.individual_words.is_empty());
    }

    #[test]
    fn test_escaped_quotes() {
        let parsed = QueryParser::parse(r#""quote with \" inside""#);
        assert_eq!(parsed.quoted_phrases, vec![r#"quote with " inside"#]);
    }

    #[test]
    fn test_unclosed_quote() {
        let parsed = QueryParser::parse(r#"hello "unclosed world"#);
        // Unclosed quote should be treated literally
        assert_eq!(parsed.individual_words, vec!["hello", "unclosed", "world"]);
    }

    #[test]
    fn test_empty_query() {
        let parsed = QueryParser::parse("");
        assert_eq!(parsed.original, "");
        assert!(parsed.individual_words.is_empty());
        assert!(parsed.quoted_phrases.is_empty());
    }

    #[test]
    fn test_only_stop_words() {
        let parsed = QueryParser::parse("the and or");
        assert!(parsed.individual_words.is_empty());
    }

    #[test]
    fn test_unicode_words() {
        let parsed = QueryParser::parse("café résumé");
        assert_eq!(parsed.individual_words, vec!["café", "résumé"]);
    }
}
