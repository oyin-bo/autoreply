//! Stub implementation of SentencePiece functionality
//!
//! This stub is active when the `experimental-sentencepiece` feature is NOT enabled.
//! It provides minimal API compatibility to allow the codebase to compile without the feature.

#![cfg(not(feature = "experimental-sentencepiece"))]

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SentencePieceError {
    #[error(
        "SentencePiece support is not enabled. Rebuild with --features experimental-sentencepiece"
    )]
    NotEnabled,
}

/// Stub tokenizer that returns an error when used
pub struct Tokenizer;

impl Tokenizer {
    pub fn new() -> Result<Self, SentencePieceError> {
        Err(SentencePieceError::NotEnabled)
    }

    pub fn from_file<P: AsRef<Path>>(_path: P) -> Result<Self, SentencePieceError> {
        Err(SentencePieceError::NotEnabled)
    }

    pub fn encode(&self, _text: &str) -> Result<Vec<u32>, SentencePieceError> {
        Err(SentencePieceError::NotEnabled)
    }

    pub fn decode(&self, _ids: &[u32]) -> Result<String, SentencePieceError> {
        Err(SentencePieceError::NotEnabled)
    }
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self
    }
}
