mod loader;
mod engine;

pub use loader::{EmbeddingTable, load_embeddings};
pub use engine::embed_text;
