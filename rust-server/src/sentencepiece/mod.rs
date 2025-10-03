pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/sentencepiece_model.rs"));
    pub use sentencepiece::*;
}

pub mod loader;
