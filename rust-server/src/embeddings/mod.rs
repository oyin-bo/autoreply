//! Embeddings module (experimental)
//!
//! This module is only available when the `experimental-sentencepiece` feature is enabled.

#![allow(dead_code)]

mod engine;
mod loader;

pub use engine::embed_text;
pub use loader::{load_embeddings, EmbeddingTable};
