use autoreply::embeddings::{load_embeddings, embed_text};
use autoreply::sentencepiece::tokenizer::SentencePieceProcessor;
use autoreply::sentencepiece::loader::SentencePieceModel;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    
    // Load embeddings
    println!("Loading embeddings...");
    let table = load_embeddings()?;
    println!("✓ Loaded {} tokens × {} dims ({:.1} MB)",
             table.vocab_size, table.embed_dim,
             (table.embeddings.len() as f64) / 1_000_000.0);
    println!("  Load time: {:?}", start.elapsed());
    
    // Load tokenizer
    let tokenizer_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("gemma-data")
        .join("tokenizer.model");
    
    let model = SentencePieceModel::load_from_file(&tokenizer_path)?;
    let tokenizer = SentencePieceProcessor::new(model);
    println!("✓ Loaded tokenizer");
    
    // Test embedding
    let texts = [
        "hello world",
        "the quick brown fox jumps over the lazy dog",
        "machine learning and artificial intelligence",
    ];
    
    for text in &texts {
        let start = Instant::now();
        let embedding = embed_text(text, &tokenizer, &table);
        let elapsed = start.elapsed();
        
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("\nText: {:?}", text);
        println!("  Embedding: [{:.4}, {:.4}, ..., {:.4}] (norm={:.4})",
                 embedding[0], embedding[1], embedding[embedding.len()-1], norm);
        println!("  Time: {:?}", elapsed);
    }
    
    Ok(())
}
