//! Enhanced search functionality with fuzzy matching
//!
//! This module implements the search architecture described in docs/19.1-search.md

pub mod engine;
pub mod fuzzy;
pub mod parser;
pub mod ranking;

#[allow(unused_imports)]
pub use engine::{SearchEngine, SearchResult};
#[allow(unused_imports)]
pub use fuzzy::FuzzyMatcher;
#[allow(unused_imports)]
pub use parser::{ParsedQuery, QueryParser};
#[allow(unused_imports)]
pub use ranking::{MatchScore, ScoringWeights};

// Property tests for search (only compiled during tests)
#[cfg(test)]
mod property_tests2;
