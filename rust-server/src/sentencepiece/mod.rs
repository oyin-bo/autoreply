pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/sentencepiece.rs"));
}

pub mod loader;
pub mod normalizer;
pub mod trie;
pub mod tokenizer;
