# Gemma Embedding Preparation Tool

This tool extracts token embeddings from Gemma model weights, applies PCA dimensionality reduction, and quantizes them to 8-bit for efficient runtime inference.

## Prerequisites

1. Download the Gemma model weights:
   ```bash
   # From the gemma-data/ directory
   wget https://huggingface.co/google/gemma-3-270m/resolve/main/model.safetensors
   # or use huggingface-cli:
   # huggingface-cli download google/gemma-3-270m --include "model.safetensors" --local-dir .
   ```

2. Ensure `tokenizer.model` exists in `gemma-data/` (should already be present).

## Usage

From this directory (`gemma-data/rust-prep/`):

```bash
# Generate embeddings
cargo run --release

# Run demo (test the embeddings work)
cargo run --release --example embed_demo
```

## Output

Generates two files in `gemma-data/`:

1. **`embeddings.npy`** (655 MB)
   - Raw float32 embeddings: 262,144 tokens × 640 dimensions
   - Numpy format for inspection/debugging

2. **`embeddings_64d_q8.bin`** (17 MB)
   - PCA-reduced to 64 dimensions
   - Quantized to 8-bit per dimension
   - Custom binary format optimized for fast loading
   - **This is the file used by the Rust inference engine**

## Implementation Details

- **PCA**: Randomized projection for fast dimensionality reduction (640 → 64 dims)
- **Quantization**: Symmetric 8-bit quantization with per-token scale factors
- **Binary format**: Custom EMB8 format v1
  - Header (32 bytes): magic, version, vocab_size, embed_dim, original_dim
  - PCA mean vector (original_dim × 4 bytes)
  - Scale factors (vocab_size × 4 bytes)
  - Quantized embeddings (vocab_size × embed_dim bytes)

## Performance

- Extraction time: ~5-10 seconds
- Compression ratio: 38× (655 MB → 17 MB)
- Runtime loading: ~90-170ms
- Inference: ~20-35µs per text

## Notes

- The output files are **not checked into git** (too large)
- Run this tool whenever you update the Gemma model
- The 17MB `.bin` file can be embedded directly into the Rust binary using the `embed-model` feature flag
