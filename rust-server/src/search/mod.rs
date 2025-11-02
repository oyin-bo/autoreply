//! Enhanced search functionality with fuzzy matching
//!
//! This module implements the search architecture described in docs/19.1-search.md

pub mod engine;
pub mod fuzzy;
pub mod parser;
pub mod ranking;

pub use engine::{SearchEngine, SearchResult};
pub use fuzzy::FuzzyMatcher;
pub use parser::{ParsedQuery, QueryParser};
pub use ranking::{MatchScore, ScoringWeights};
