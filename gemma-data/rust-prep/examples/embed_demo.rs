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
        .join("tokenizer.model");
    
    let model = SentencePieceModel::load_from_file(&tokenizer_path)?;
    let tokenizer = SentencePieceProcessor::new(model);
    println!("✓ Loaded tokenizer");
    
    // Test embedding
    let texts = [
        "May the Force be with you",
        "I'll be back",
        "Here's looking at you, kid",
        "You can't handle the truth",
        "Go ahead, make my day",
        "I'm king of the world",
        "You talking to me",
        "What we've got here is failure to communicate",
        "I love the smell of napalm in the morning",
        "Love means never having to say you're sorry",
        "The stuff that dreams are made of",
        "E.T. phone home",
        "They may take our lives, but they'll never take our freedom",
        "I'm as mad as hell, and I'm not going to take this anymore",
        "After all, tomorrow is another day",
        "Round up the usual suspects",
        "I'll have what she's having",
        "You know how to whistle, don't you",
        "There's no place like home",
        "I am big, it's the pictures that got small",
        "Show me the money",
        "Why don't you come up sometime and see me",
        "I'm walking here",
        "Play it, Sam",
        "You had me at hello",
        "A census taker once tried to test me",
        "Life is like a box of chocolates",
        "Open the pod bay doors, HAL",
        "Soylent Green is people",
        "Here's Johnny",
        "Elementary, my dear Watson",
        "Get your stinking paws off me",
        "Nobody puts Baby in a corner",
    ];
    
    // Unicode arrows for visualization: 8 directions
    let arrows = ["🡐", "🡑", "🡒", "🡓", "🡔", "🡕", "🡖", "🡗"];
    
    // Reference word for similarity comparison
    let reference_text = "stench";
    let reference_embedding = embed_text(reference_text, &tokenizer, &table);
    
    // Compute cosine similarity helper
    let cosine_similarity = |a: &[f32], b: &[f32]| -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    };
    
    // Compute embeddings and similarities using max-word approach
    let mut results: Vec<(String, Vec<f32>, f32, f32, u128)> = texts.iter().map(|text| {
        let start = Instant::now();
        let embedding = embed_text(text, &tokenizer, &table);
        let elapsed = start.elapsed();
        
        // Compute similarity for the whole phrase
        let phrase_similarity = cosine_similarity(&embedding, &reference_embedding);
        
        // Also compute max similarity across individual words
        let words: Vec<&str> = text.split_whitespace().collect();
        let max_word_similarity = words.iter()
            .map(|word| {
                let word_emb = embed_text(word, &tokenizer, &table);
                cosine_similarity(&word_emb, &reference_embedding)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        
        (text.to_string(), embedding, phrase_similarity, max_word_similarity, elapsed.as_micros())
    }).collect();
    
    // Sort by max word similarity (descending)
    results.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    
    println!("\nSorted by max-word similarity to \"{}\":\n", reference_text);
    
    for (text, embedding, phrase_sim, max_word_sim, micros) in results {
        print!("{:6.1}µs  ", micros as f64 / 1000.0);
        for (i, &val) in embedding.iter().take(8).enumerate() {
            if val >= 0.0 {
                print!("{} {:.3} ", arrows[i % arrows.len()], val);
            } else {
                print!("{}{:.3} ", arrows[i % arrows.len()], val);
            }
        }
        println!(" [max: {:.3} phrase: {:.3}] \"{}\"", max_word_sim, phrase_sim, text);
    }
    
    Ok(())
}
