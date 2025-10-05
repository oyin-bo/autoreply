use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

/// Embedding table loaded from EMB8 format
#[derive(Debug)]
pub struct EmbeddingTable {
    pub vocab_size: usize,
    pub embed_dim: usize,
    pub original_dim: usize,
    pub pca_mean: Vec<f32>,
    pub scales: Vec<f32>,
    pub embeddings: Vec<u8>, // flattened: vocab_size × embed_dim
}

impl EmbeddingTable {
    /// Get embedding for a token ID (returns u8 slice, caller must dequantize)
    #[inline]
    pub fn get_quantized(&self, token_id: usize) -> &[u8] {
        let start = token_id * self.embed_dim;
        let end = start + self.embed_dim;
        &self.embeddings[start..end]
    }

    /// Get scale factor for a token
    #[inline]
    pub fn get_scale(&self, token_id: usize) -> f32 {
        self.scales[token_id]
    }

    /// Dequantize a token embedding into the provided buffer
    #[inline]
    pub fn dequantize_into(&self, token_id: usize, out: &mut [f32]) {
        let quantized = self.get_quantized(token_id);
        let scale = self.get_scale(token_id);
        for (i, &q) in quantized.iter().enumerate() {
            out[i] = (q as f32 - 128.0) * scale;
        }
    }
}

/// Load embeddings from embedded binary (compile-time included)
#[cfg(feature = "embed-model")]
pub fn load_embeddings() -> Result<EmbeddingTable> {
    const DATA: &[u8] = include_bytes!("../../../gemma-data/embeddings_64d_q8.bin");
    parse_emb8(DATA)
}

/// Load embeddings from external file (runtime load, memory-mapped)
#[cfg(not(feature = "embed-model"))]
pub fn load_embeddings() -> Result<EmbeddingTable> {
    use std::fs;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("gemma-data")
        .join("embeddings_64d_q8.bin");

    let data =
        fs::read(&path).with_context(|| format!("failed to read embeddings from {:?}", path))?;

    parse_emb8(&data)
}

/// Parse EMB8 binary format
fn parse_emb8(data: &[u8]) -> Result<EmbeddingTable> {
    let mut cursor = Cursor::new(data);

    // Read header (32 bytes)
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != b"EMB8" {
        bail!("invalid magic bytes: expected EMB8");
    }

    let version = cursor.read_u32::<LittleEndian>()?;
    if version != 1 {
        bail!("unsupported version: {}", version);
    }

    let vocab_size = cursor.read_u32::<LittleEndian>()? as usize;
    let embed_dim = cursor.read_u32::<LittleEndian>()? as usize;
    let original_dim = cursor.read_u32::<LittleEndian>()? as usize;

    // Skip reserved bytes
    cursor.set_position(cursor.position() + 12);

    // Read PCA mean (original_dim floats)
    let mut pca_mean = vec![0.0f32; original_dim];
    for i in 0..original_dim {
        pca_mean[i] = cursor.read_f32::<LittleEndian>()?;
    }

    // Read scales (vocab_size floats)
    let mut scales = vec![0.0f32; vocab_size];
    for i in 0..vocab_size {
        scales[i] = cursor.read_f32::<LittleEndian>()?;
    }

    // Read quantized embeddings (vocab_size × embed_dim bytes)
    let emb_size = vocab_size * embed_dim;
    let pos = cursor.position() as usize;
    let embeddings = data[pos..pos + emb_size].to_vec();

    Ok(EmbeddingTable {
        vocab_size,
        embed_dim,
        original_dim,
        pca_mean,
        scales,
        embeddings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_embeddings() {
        let table = load_embeddings().expect("failed to load embeddings");
        assert_eq!(table.embed_dim, 64);
        assert!(table.vocab_size > 0);

        // Test dequantization
        let mut out = vec![0.0f32; table.embed_dim];
        table.dequantize_into(0, &mut out);

        // Should have reasonable values
        assert!(out.iter().any(|&x| x.abs() > 1e-6));
    }
}
