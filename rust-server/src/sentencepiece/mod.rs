//! SentencePiece tokenization support (experimental)
//!
//! This module is only available when the `experimental-sentencepiece` feature is enabled.

#![cfg(feature = "experimental-sentencepiece")]
#![allow(dead_code, unused_imports, unused_variables)]

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/sentencepiece.rs"));
}

pub mod loader;
pub mod normalizer;
pub mod tokenizer;
pub mod trie;
