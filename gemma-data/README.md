# Gemma Data Directory

This directory contains the tokenizer and model assets for Gemma-3-270m, along with tools for preparing embeddings.

## Contents

### Essential Files (checked in)
- `tokenizer.model` (4.5 MB) - SentencePiece tokenizer
- `tokenizer.json`, `tokenizer_config.json` - Tokenizer metadata
- `added_tokens.json`, `special_tokens_map.json` - Token mappings
- `generation_config.json` - Model generation parameters
- `rust-prep/` - Embedding extraction and quantization tool

### Generated Files (not checked in - see `.gitignore`)
- `model.safetensors` (536 MB) - Model weights, download separately
- `embeddings.npy` (655 MB) - Raw float32 embeddings
- `embeddings_64d_q8.bin` (17 MB) - Quantized embeddings for runtime

## Getting Started

1. **Download model weights:**
   ```bash
   wget https://huggingface.co/google/gemma-3-270m/resolve/main/model.safetensors
   ```

2. **Generate embeddings:**
   ```bash
   cd rust-prep
   cargo run --release
   ```

3. **Use in Rust:**
   ```rust
   use autoreply::embeddings::{load_embeddings, embed_text};
   
   let table = load_embeddings()?;
   let embedding = embed_text("hello world", &tokenizer, &table);
   ```

## Performance

- **Embedding dimension**: 64 (reduced from 640)
- **Quantization**: 8-bit per dimension
- **Size**: 17 MB (38× compression)
- **Load time**: ~90-170ms
- **Inference**: ~20-35µs per text

## See Also

- `rust-prep/README.md` - Detailed tool documentation
- `../rust-server/src/embeddings/` - Runtime inference engine
- `../rust-server/examples/embed_demo.rs` - Usage example
