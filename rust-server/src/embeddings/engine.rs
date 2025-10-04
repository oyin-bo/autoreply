use super::loader::EmbeddingTable;
use crate::sentencepiece::tokenizer::SentencePieceProcessor;

/// Generate embedding for text (simple average of token embeddings)
pub fn embed_text(
    text: &str,
    tokenizer: &SentencePieceProcessor,
    table: &EmbeddingTable,
) -> Vec<f32> {
    let tokens = match tokenizer.encode(text) {
        Ok(t) => t,
        Err(_) => return vec![0.0; table.embed_dim],
    };
    
    if tokens.is_empty() {
        return vec![0.0; table.embed_dim];
    }
    
    // Accumulator
    let mut sum = vec![0.0f32; table.embed_dim];
    let mut buf = vec![0.0f32; table.embed_dim];
    
    for &token_id in &tokens {
        let tid = token_id as usize;
        if tid < table.vocab_size {
            table.dequantize_into(tid, &mut buf);
            for i in 0..table.embed_dim {
                sum[i] += buf[i];
            }
        }
    }
    
    // Average
    let count = tokens.len() as f32;
    for x in &mut sum {
        *x /= count;
    }
    
    // L2 normalize
    let norm = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for x in &mut sum {
            *x /= norm;
        }
    }
    
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // requires tokenizer setup
    fn test_embed_text() {
        let table = super::super::load_embeddings().unwrap();
        let tokenizer = SentencePieceProcessor::new("path/to/tokenizer.model").unwrap();
        
        let embedding = embed_text("hello world", &tokenizer, &table);
        assert_eq!(embedding.len(), 64);
        
        // Should be normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }
}
