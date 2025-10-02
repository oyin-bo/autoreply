# SentencePiece-based Fuzzy Search: Implementation Analysis and Roadmap

## Executive Summary

This document analyzes the SentencePiece algorithm for inference-only implementation to enable semantic similarity-based fuzzy search in the Autoreply MCP server. The goal is to match search queries semantically (e.g., "foot" and "feet" are considered similar) rather than just using Unicode-normalized case-insensitive matching.

**Key Findings:**
- SentencePiece inference is suitable for direct porting to Go/Rust/JS
- The algorithm is relatively simple for inference (no training required)
- Memory allocation can be made zero-allocation with pre-allocated buffer pools
- For semantic search, we need post-tokenization embedding generation
- Total implementation effort: Medium complexity, high reward

**Recommended Approach:** Port SentencePiece inference directly to target languages, then use static embeddings (like Model2Vec approach in `docs/3-detour-model2vec.md`) for semantic similarity.

---

## 1. SentencePiece Architecture Overview

### 1.1 Core Components (Inference Only)

The SentencePiece inference pipeline consists of three main stages:

```
Input Text → Normalization → Tokenization → Token IDs
```

#### Component Breakdown:

1. **Normalizer** (`normalizer.h/cc`)
   - Unicode normalization (NFKC)
   - Whitespace handling (replace with U+2581 ▁)
   - Character mapping via Double-Array Trie
   - Pre-compiled normalization rules

2. **Model Interface** (`model_interface.h/cc`)
   - Vocabulary management
   - Piece-to-ID and ID-to-piece mapping
   - Score retrieval for pieces
   - Special token handling (UNK, BOS, EOS, PAD)

3. **Tokenization Models** (choose one):
   - **Unigram Model** (`unigram_model.h/cc`) - Most common, uses Viterbi
   - **BPE Model** (`bpe_model.h/cc`) - Byte-Pair Encoding
   - **Word Model** (`word_model.h/cc`) - Simple whitespace splitting
   - **Char Model** (`char_model.h/cc`) - Character-level

4. **Supporting Structures**:
   - **Double-Array Trie** (Darts) - Fast prefix matching for vocabulary
   - **FreeList** (`freelist.h`) - Pre-allocated memory pools
   - **ModelProto** - Serialized model format (Protocol Buffers)

---

## 2. Detailed Algorithm Analysis

### 2.1 Normalization Phase

**Purpose:** Transform input text into a canonical form for consistent tokenization.

**Algorithm:**
```pseudocode
function Normalize(input_text):
    normalized = ""
    position_map = []  // Maps normalized position → original position
    
    // Skip leading whitespace if configured
    if remove_extra_whitespaces:
        skip leading spaces
    
    // Add prefix whitespace symbol if configured
    if add_dummy_prefix and not treat_whitespace_as_suffix:
        normalized += "▁"  // U+2581
    
    // Process each character
    for each char in input_text:
        // Use trie to find longest matching normalization rule
        normalized_char = trie.LongestPrefixMatch(char)
        
        // Replace whitespace with ▁ if escape_whitespaces enabled
        if escape_whitespaces and normalized_char == ' ':
            normalized_char = "▁"
        
        normalized += normalized_char
        position_map.append(current_original_position)
    
    // Remove trailing whitespace if configured
    if remove_extra_whitespaces:
        trim trailing ▁ or spaces
    
    return normalized, position_map
```

**Key Data Structures:**
- **Trie (Darts::DoubleArray):** O(k) lookup where k is match length
- **Precompiled charsmap:** Binary blob of normalization rules
- **Position map:** Vector mapping normalized → original positions

**Memory Requirements:**
- Trie: Fixed size, loaded once from model
- Normalization buffer: Input size × 3 (worst case for UTF-8 expansion)
- Position map: Input size × sizeof(size_t)

---

### 2.2 Unigram Tokenization (Viterbi Algorithm)

**Purpose:** Find the highest-probability segmentation of normalized text into sentence pieces.

**Core Concept:**
- Build a lattice (DAG) where nodes = character positions, edges = possible tokens
- Use Viterbi algorithm to find best path through lattice
- Each token has a score (log probability from training)

**Detailed Algorithm:**

#### 2.2.1 Lattice Construction

```pseudocode
class Lattice:
    nodes: array[N+1]  // N = character count
    sentence: string_view
    surface: array of char*  // Pointers to each character position
    begin_nodes: array[N+1] of list<Node*>  // Nodes starting at position
    end_nodes: array[N+1] of list<Node*>    // Nodes ending at position
    node_allocator: FreeList<Node>

struct Node:
    piece: string_view        // The actual token text
    pos: int                  // Starting position (in characters)
    length: int               // Length (in characters)
    id: int                   // Vocabulary ID
    score: float              // Log probability
    backtrace_score: float    // Accumulated score (used in Viterbi)
    prev: Node*               // Best previous node (for backtracking)

function SetSentence(text):
    Clear()
    sentence = text
    
    // Build surface array (character boundaries)
    while not text.empty():
        mblen = UTF8CharLength(text[0])
        surface.append(text.data())
        text.remove_prefix(mblen)
    
    // Add BOS and EOS nodes
    bos = NewNode()
    bos.id = -1
    bos.pos = 0
    end_nodes[0].append(bos)
    
    eos = NewNode()
    eos.id = -1
    eos.pos = length
    begin_nodes[length].append(eos)
```

#### 2.2.2 Populating Lattice with Candidate Tokens

```pseudocode
function PopulateNodes(lattice):
    unk_score = min_score - 10.0  // Penalty for unknown tokens
    
    for pos from 0 to lattice.size():
        begin_text = lattice.surface(pos)
        
        // Use trie to find all tokens that start at this position
        matches = vocab_trie.CommonPrefixSearch(begin_text)
        
        has_single_char = false
        for each match in matches:
            token_id = match.value
            token_length = match.length  // in characters
            
            if IsUnused(token_id):
                continue
            
            // Insert node into lattice
            node = lattice.Insert(pos, token_length)
            node.id = token_id
            
            // Score: use vocab score, or give bonus to user-defined symbols
            if IsUserDefined(token_id):
                node.score = token_length * max_score - 0.1
            else:
                node.score = vocab_scores[token_id]
            
            if token_length == 1:
                has_single_char = true
        
        // Always ensure single-character fallback (UNK handling)
        if not has_single_char:
            node = lattice.Insert(pos, 1)
            node.id = unk_id
            node.score = unk_score
```

**Key Insight:** The trie enables efficient matching of all vocabulary tokens that could start at each position, with O(k) complexity where k is the longest token length.

#### 2.2.3 Viterbi Path Finding

```pseudocode
function Viterbi():
    // Forward pass: compute best score to reach each position
    for pos from 0 to length:
        for each rnode in begin_nodes[pos]:  // Nodes starting at pos
            best_score = -infinity
            best_node = null
            
            // Check all nodes ending at pos
            for each lnode in end_nodes[pos]:
                score = lnode.backtrace_score + rnode.score
                if score > best_score:
                    best_score = score
                    best_node = lnode
            
            rnode.prev = best_node
            rnode.backtrace_score = best_score
    
    // Backward pass: backtrack to extract best path
    results = []
    node = eos_node.prev
    while node.prev != null:
        results.prepend(node)
        node = node.prev
    
    return results, eos_node.backtrace_score
```

**Complexity:**
- Time: O(N × M × K) where:
  - N = input length in characters
  - M = average number of tokens starting at each position (~10-30)
  - K = average number of tokens ending at each position (~10-30)
- Space: O(N × M) for nodes in lattice

---

### 2.3 Optimized Viterbi (Zero Lattice)

The codebase includes an optimized version (`EncodeOptimized`) that:
- Doesn't build explicit Lattice structure
- Works directly on UTF-8 bytes instead of character positions
- Stores only best path at each position
- Reduces memory by ~70%

**Key Differences:**
```pseudocode
struct ViterbiNode:
    best_path_score: float
    starts_at: int      // Position where best token started
    token_id: int

function EncodeOptimized(normalized_text):
    nodes = array[byte_length + 1]
    nodes[0] = {score: 0, starts_at: -1, token_id: -1}
    
    byte_pos = 0
    char_pos = 0
    
    while byte_pos < byte_length:
        // Find all tokens starting at current position
        matches = vocab_trie.CommonPrefixSearch(text[byte_pos:])
        
        for each match in matches:
            end_byte_pos = byte_pos + match.byte_length
            new_score = nodes[byte_pos].score + GetScore(match.id)
            
            if new_score > nodes[end_byte_pos].best_path_score:
                nodes[end_byte_pos].best_path_score = new_score
                nodes[end_byte_pos].starts_at = byte_pos
                nodes[end_byte_pos].token_id = match.id
        
        byte_pos += UTF8CharLength(text[byte_pos])
    
    // Backtrack
    results = []
    pos = byte_length
    while pos > 0:
        node = nodes[pos]
        results.prepend(node.token_id)
        pos = node.starts_at
    
    return results
```

**Memory Savings:**
- No Lattice structure
- No Node allocation
- Only O(N) space instead of O(N × M)

---

### 2.4 BPE Tokenization (Alternative)

**Purpose:** Byte-Pair Encoding - iteratively merge most frequent adjacent pairs.

**Algorithm:**
```pseudocode
function BPE_Encode(normalized_text):
    // Split into initial symbols (characters or user-defined)
    symbols = SplitIntoCharacters(normalized_text)
    
    // Build priority queue of adjacent pairs
    agenda = PriorityQueue()
    for i in 1..symbols.length:
        pair = symbols[i-1] + symbols[i]
        if pair in vocab:
            agenda.push({
                left: i-1,
                right: i,
                score: vocab_scores[pair]
            })
    
    // Iteratively merge best pairs
    while not agenda.empty():
        top_pair = agenda.pop()
        
        // Check if still valid
        if symbols[left] or symbols[right] is merged:
            continue
        
        // Merge: replace left and right with merged symbol
        symbols[left] = symbols[left] + symbols[right]
        symbols[right] = <empty>
        
        // Add new adjacent pairs to agenda
        if left-1 exists:
            TryAddPair(left-1, left, agenda)
        if right+1 exists:
            TryAddPair(left, right+1, agenda)
    
    return [symbol for symbol in symbols if not empty]
```

**Differences from Unigram:**
- Greedy merge strategy vs global optimization
- No Viterbi algorithm needed
- Simpler, but potentially less optimal

---

## 3. Memory Allocation Analysis

### 3.1 Current Allocation Patterns

**Read-Only (Loaded Once):**
- Model proto (vocabulary, scores, config)
- Normalization trie
- Vocabulary trie
- All static data structures

**Per-Request Allocations:**

1. **Normalization:**
   - Output buffer: ~3× input size (max UTF-8 expansion)
   - Position map: 1× input size × sizeof(size_t)

2. **Unigram Lattice (Full):**
   - Nodes: N × M × sizeof(Node) (~50-100 bytes per node)
   - Node lists: 2 × (N+1) × sizeof(vector<Node*>)
   - FreeList chunks: Pre-allocated, reused

3. **Unigram Optimized:**
   - ViterbiNode array: N × sizeof(ViterbiNode) (~20 bytes)
   - Trie results buffer: Fixed small size (1024 × sizeof(result_pair))

4. **BPE:**
   - Symbol array: ~N × sizeof(Symbol) (~40 bytes)
   - Symbol pair allocator: Pre-allocated chunks
   - Priority queue: O(N) pairs max

### 3.2 Zero-Allocation Strategy

**Goal:** Pre-allocate all buffers, reuse for subsequent requests.

**Implementation:**
```pseudocode
class SentencePieceInference:
    // Read-only model data
    model_proto: ModelProto
    vocab_trie: Trie
    norm_trie: Trie
    
    // Reusable buffers (per-thread or pooled)
    norm_buffer: String (capacity: max_input_size × 3)
    position_map: Vec<size_t> (capacity: max_input_size × 3)
    viterbi_nodes: Vec<ViterbiNode> (capacity: max_input_size)
    trie_results: Vec<TrieResult> (capacity: 1024)
    output_tokens: Vec<TokenId> (capacity: max_input_size)
    
    function Encode(input_text, reuse_buffers=true):
        // Clear buffers instead of reallocating
        norm_buffer.clear()  // Keeps capacity
        position_map.clear()
        viterbi_nodes.clear()
        output_tokens.clear()
        
        // Use pre-sized buffers
        Normalize(input_text, &norm_buffer, &position_map)
        TokenizeOptimized(norm_buffer, &viterbi_nodes, &output_tokens)
        
        return output_tokens
```

**Benefits:**
- No heap allocations during inference
- Predictable memory usage
- Cache-friendly (same buffers reused)
- Thread-safe with per-thread buffers or pooling

**Memory Budget (example for 280 char tweet):**
- Normalization buffer: ~1KB
- Position map: ~2KB
- Viterbi nodes: ~20KB
- Trie results: ~16KB
- Output tokens: ~2KB
- **Total: ~41KB per concurrent request**

---

## 4. Semantic Search Strategy

### 4.1 Problem Statement

SentencePiece alone produces **token IDs**, not semantic embeddings. To enable fuzzy semantic search (e.g., "foot" ≈ "feet"), we need embeddings.

**Two-Stage Approach:**

```
Text → SentencePiece Tokenization → Token IDs → Embedding Generation → Vector
```

### 4.2 Embedding Generation Options

#### Option A: Static Token Embeddings (Recommended)

**Concept:** Pre-compute embeddings for each token in vocabulary, then average.

**Implementation:**
```pseudocode
// Offline: Generate embeddings table
embedding_table = {}
for token in vocabulary:
    // Use teacher model (e.g., multilingual-e5-base)
    embedding = teacher_model.encode(token)
    embedding_table[token] = embedding

// Save as memory-mapped file
save_embeddings(embedding_table, "embeddings.bin")

// Online: Fast lookup and averaging
function GetEmbedding(token_ids):
    embeddings = []
    for token_id in token_ids:
        embeddings.append(embedding_table[token_id])
    
    // Average pooling
    result = mean(embeddings)
    
    // L2 normalization
    result = normalize(result)
    
    return result
```

**Pros:**
- Fast inference (<100µs per text)
- No runtime ML models needed
- Works in WASM/JS
- Consistent with `docs/3-detour-model2vec.md` approach

**Cons:**
- No contextualization (static embeddings)
- Quality depends on teacher model and PCA reduction

**Recommendation:** Use this approach. It aligns with Model2Vec strategy already documented.

---

#### Option B: Contextual Embeddings (Higher Quality, Slower)

**Concept:** Run a transformer model on tokens for contextualized embeddings.

**Options:**
- ONNX runtime with quantized model
- llama.cpp-style implementation
- TensorFlow Lite

**Pros:**
- Higher quality semantic matching
- Contextual understanding

**Cons:**
- Slow (100ms+ per text)
- Large model files (100MB+)
- Complex runtime dependencies

**Verdict:** Not suitable for MCP server use case (needs fast responses).

---

### 4.3 Similarity Calculation

Once embeddings are generated:

```pseudocode
function FuzzySearch(query, posts):
    query_tokens = SentencePiece.Encode(query)
    query_embedding = GetEmbedding(query_tokens)
    
    results = []
    for post in posts:
        post_tokens = SentencePiece.Encode(post.text)
        post_embedding = GetEmbedding(post_tokens)
        
        // Cosine similarity (since L2-normalized)
        similarity = dot_product(query_embedding, post_embedding)
        
        results.append({post: post, score: similarity})
    
    // Sort by similarity descending
    results.sort(key=lambda x: x.score, reverse=True)
    
    return results[:top_k]
```

**Optimization:** Pre-compute post embeddings and store in index for O(N) search instead of O(N²).

---

## 5. Language-Specific Implementation Guide

### 5.1 Go Implementation

**Recommended Libraries:**
- JSON/Protobuf parsing: `encoding/json`, `google.golang.org/protobuf`
- Trie: Implement Double-Array or use `github.com/darts-clone/go-darts` (if exists)
- Memory-mapped I/O: `syscall.Mmap` or `github.com/edsrzf/mmap-go`

**Key Considerations:**
- Use slices for buffers with pre-allocated capacity
- Struct pooling with `sync.Pool` for concurrent requests
- Memory-map embedding table for zero-copy access

**Example Structure:**
```go
type SentencePieceProcessor struct {
    model       *ModelProto
    vocabTrie   *DoubleArrayTrie
    normTrie    *DoubleArrayTrie
    embeddings  []float32 // Memory-mapped
    
    // Buffer pools (thread-safe)
    normBufPool *sync.Pool
    nodePool    *sync.Pool
}

func (sp *SentencePieceProcessor) Encode(text string) ([]int, error) {
    // Get buffers from pool
    normBuf := sp.normBufPool.Get().(*bytes.Buffer)
    defer sp.normBufPool.Put(normBuf)
    
    normBuf.Reset()
    
    // Normalize
    sp.normalize(text, normBuf)
    
    // Tokenize
    tokens := sp.tokenize(normBuf.Bytes())
    
    return tokens, nil
}
```

---

### 5.2 Rust Implementation

**Recommended Crates:**
- Protobuf: `prost` or `protobuf`
- Memory mapping: `memmap2`
- Trie: Implement or use `trie-rs`
- SIMD operations: `std::simd` (for dot products)

**Key Considerations:**
- Zero-copy with `&str` and `&[u8]` where possible
- Arena allocators (`bumpalo`) for nodes
- Unsafe code for raw pointer manipulation in trie
- Rayon for parallel search across posts

**Example Structure:**
```rust
pub struct SentencePieceProcessor {
    model: ModelProto,
    vocab_trie: DoubleArrayTrie,
    norm_trie: DoubleArrayTrie,
    embeddings: Mmap, // Memory-mapped embeddings
}

impl SentencePieceProcessor {
    pub fn encode(&self, text: &str) -> Result<Vec<u32>, Error> {
        // Stack-allocated buffers for small inputs
        let mut norm_buf = String::with_capacity(text.len() * 3);
        let mut pos_map = Vec::with_capacity(text.len() * 3);
        
        // Normalize
        self.normalize(text, &mut norm_buf, &mut pos_map)?;
        
        // Tokenize
        let tokens = self.tokenize(&norm_buf)?;
        
        Ok(tokens)
    }
}
```

**Performance Optimizations:**
- Use `SmallVec` for token output (most tweets < 64 tokens)
- SIMD for embedding operations
- `#[inline]` for hot path functions

---

### 5.3 JavaScript/TypeScript Implementation

**Recommended Libraries:**
- Protobuf: `protobufjs`
- Typed arrays: `Uint32Array`, `Float32Array`
- WASM for performance-critical parts: `wasm-bindgen`

**Key Considerations:**
- TypedArrays for buffers (avoid array resizing)
- ArrayBuffers for embeddings (memory-efficient)
- Consider WASM compilation of Rust/Go code
- WebWorker for async processing

**Example Structure:**
```typescript
class SentencePieceProcessor {
    private model: ModelProto;
    private vocabTrie: DoubleArrayTrie;
    private normTrie: DoubleArrayTrie;
    private embeddings: Float32Array;
    
    // Reusable buffers
    private normBuffer: Uint8Array;
    private posMap: Uint32Array;
    private tokenBuffer: Uint32Array;
    
    encode(text: string): Uint32Array {
        // Reset buffer positions
        let normLen = 0;
        let posLen = 0;
        
        // Normalize
        normLen = this.normalize(text, this.normBuffer, this.posMap);
        
        // Tokenize
        const tokenCount = this.tokenize(
            this.normBuffer.subarray(0, normLen),
            this.tokenBuffer
        );
        
        return this.tokenBuffer.subarray(0, tokenCount);
    }
}
```

**WASM Integration:**
If performance is critical, compile core algorithm in Rust:
```typescript
import init, { encode_text, get_embedding } from './sentencepiece_wasm.js';

await init();

const tokens = encode_text(text);
const embedding = get_embedding(tokens);
```

---

## 6. Testing Strategy

### 6.1 Algorithm Correctness

**Goal:** Ensure ported implementation matches original.

**Approach:**
1. **Test Data:** Use `self_test_data` from official SentencePiece models
2. **Golden Files:** Export test cases from original C++ implementation
3. **Cross-validation:** Compare outputs across languages

**Example Test:**
```yaml
test_cases:
  - input: "Hello world"
    expected_tokens: [72, 8661, 99, 934]
    expected_pieces: ["▁He", "llo", "▁", "world"]
  
  - input: "足球"  # Soccer in Chinese
    expected_tokens: [98234, 10943]
    expected_pieces: ["▁足", "球"]
```

**Verification Method:**
```pseudocode
function TestCorrectness():
    official_model = LoadOfficialSPModel()
    ported_model = LoadPortedImplementation()
    
    for test_case in test_cases:
        official_result = official_model.encode(test_case.input)
        ported_result = ported_model.encode(test_case.input)
        
        assert official_result == ported_result, 
               f"Mismatch for '{test_case.input}'"
```

---

### 6.2 Performance Benchmarks

**Metrics to Track:**
- Throughput: tokens/second
- Latency: ms per request (p50, p95, p99)
- Memory: bytes allocated per request
- Allocations: count of heap allocations

**Test Scenarios:**
```python
benchmarks = [
    {"name": "short", "input": "Hello", "length": 5},
    {"name": "tweet", "input": tweet_text, "length": 280},
    {"name": "thread", "input": thread_text, "length": 1000},
    {"name": "unicode", "input": mixed_unicode, "length": 200},
]
```

**Target Performance:**
- Tokenization: <1ms for 280 chars (p95)
- Embedding lookup: <0.1ms for 50 tokens
- Total semantic search: <100ms for 100 posts

---

### 6.3 Edge Cases

**Critical Edge Cases to Test:**

1. **Empty input:** `""`
2. **Single character:** `"a"`, `"中"`
3. **Unknown tokens:** `"�����"` (invalid UTF-8)
4. **Very long input:** 10,000+ chars
5. **All whitespace:** `"   \n\t  "`
6. **Mixed scripts:** `"Hello世界🌍"`
7. **Emoji sequences:** `"👨‍👩‍👧‍👦"` (family emoji with ZWJ)
8. **Byte-fallback scenarios:** Rare Unicode characters not in vocab

**Robustness Tests:**
```python
def test_edge_cases():
    assert encode("") == []
    assert encode("a").length >= 1
    assert encode("�" * 100).length >= 1  # Should not crash
    assert encode(" " * 1000).length < 10  # Should handle whitespace
```

---

## 7. Implementation Roadmap

### Phase 1: Core Inference (2-3 weeks)

**Deliverables:**
- [ ] **Protobuf parser** for ModelProto
  - Load vocabulary, scores, config
  - Parse normalization rules
  - Validate model integrity

- [ ] **Double-Array Trie implementation**
  - Build from sorted vocab
  - CommonPrefixSearch operation
  - ExactMatchSearch operation
  - Memory-efficient storage

- [ ] **Normalizer**
  - Trie-based character mapping
  - Whitespace handling
  - Position tracking

- [ ] **Unigram tokenizer (optimized version)**
  - Viterbi algorithm
  - Direct UTF-8 processing
  - Backtracking

- [ ] **BPE tokenizer** (optional, for completeness)
  - Priority queue merging
  - Symbol management

**Testing:**
- Unit tests for each component
- Integration tests against official models
- Cross-language validation

---

### Phase 2: Memory Optimization (1 week)

**Deliverables:**
- [ ] **Buffer pooling**
  - Per-thread or global pools
  - Capacity management
  - Thread-safety

- [ ] **Zero-allocation inference**
  - Pre-sized buffers
  - Reuse strategy
  - Memory profiling

- [ ] **Benchmarking suite**
  - Throughput tests
  - Latency percentiles
  - Memory allocation tracking

**Success Criteria:**
- Zero heap allocations per request
- <1ms p95 latency for 280 chars
- <50KB memory per concurrent request

---

### Phase 3: Embedding Integration (1-2 weeks)

**Deliverables:**
- [ ] **Static embedding table generation**
  - Use teacher model (multilingual-e5-base)
  - PCA dimensionality reduction to 64D
  - Quantization to int8
  - Binary serialization format

- [ ] **Embedding lookup implementation**
  - Memory-mapped file loading
  - Token ID → vector lookup
  - Average pooling
  - L2 normalization

- [ ] **Similarity calculation**
  - Dot product (cosine similarity)
  - SIMD optimization (Rust)
  - Batch processing

**File Format:**
```
Header (64 bytes):
  - Magic number: "SPEM" (4 bytes)
  - Version: 1 (4 bytes)
  - Vocab size: N (4 bytes)
  - Embedding dim: D (4 bytes)
  - Data type: int8/float32 (4 bytes)
  - Reserved (44 bytes)

Embeddings:
  - N × D × sizeof(dtype) bytes
  - Row-major layout
  - Token ID i → offset = 64 + i × D × sizeof(dtype)
```

---

### Phase 4: Search Integration (1 week)

**Deliverables:**
- [ ] **Indexing pipeline**
  - Batch process all posts
  - Compute and store embeddings
  - Incremental updates

- [ ] **Search API**
  - Query → embedding
  - Similarity ranking
  - Top-K retrieval

- [ ] **Optimizations**
  - Post embedding cache
  - Parallel search
  - Early termination

**API Design:**
```rust
pub struct SemanticSearcher {
    sp: SentencePieceProcessor,
    embeddings: EmbeddingTable,
    post_index: Vec<(PostId, Vec<f32>)>,
}

impl SemanticSearcher {
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(PostId, f32)> {
        let query_tokens = self.sp.encode(query)?;
        let query_embedding = self.embeddings.get_embedding(&query_tokens);
        
        let mut scores = Vec::with_capacity(self.post_index.len());
        for (post_id, post_embedding) in &self.post_index {
            let score = cosine_similarity(&query_embedding, post_embedding);
            scores.push((*post_id, score));
        }
        
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(top_k);
        
        scores
    }
}
```

---

### Phase 5: Cross-Language Ports (2-3 weeks)

**Order of Implementation:**
1. **Rust** (reference implementation)
   - Most similar to C++ original
   - Best performance potential
   - Can compile to WASM

2. **Go** (server implementation)
   - For go-server MCP
   - Good concurrency support
   - Straightforward from Rust

3. **JavaScript/TypeScript** (web/WASM)
   - Pure JS for Node.js
   - WASM wrapper around Rust for browser

**Shared Components:**
- Model files (same .model format)
- Embedding tables (same binary format)
- Test suites (same test cases)

---

## 8. Model Selection and Training

### 8.1 Vocabulary Size Considerations

**Trade-offs:**
- **Small vocab (8k-16k):** Faster, more general, more UNK
- **Medium vocab (32k):** Balanced (recommended)
- **Large vocab (64k+):** Better coverage, slower, more memory

**Recommendation for Autoreply:**
- Use 32k vocabulary
- Multilingual coverage (English, common emojis, Unicode)
- Social media oriented (hashtags, @mentions)

---

### 8.2 Pre-trained Models

**Option A: Use Existing Models**
- Google's official models (CC-licensed)
- Multilingual models available
- Pre-trained on large corpora

**Option B: Train Custom Model**
- On social media data (Twitter, Bluesky)
- Include domain-specific tokens
- Optimized for short-form text

**Recommendation:** Start with existing multilingual model, train custom if needed.

---

### 8.3 Model Distribution

**Packaging:**
```
sentencepiece-fuzzy-search/
  ├── models/
  │   ├── sp-32k-multilingual.model   (model file ~5MB)
  │   └── sp-32k-multilingual.vocab   (text vocab for inspection)
  ├── embeddings/
  │   ├── sp-32k-embeddings-64d.bin   (embedding table ~2MB)
  │   └── sp-32k-embeddings-64d.meta  (metadata JSON)
  └── README.md
```

**Loading:**
- Embed small models (<10MB) in binary
- Memory-map large embeddings
- Lazy initialization on first request

---

## 9. Risk Assessment and Mitigation

### 9.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Incorrect tokenization** | High | Medium | Extensive testing against official impl |
| **Performance too slow** | Medium | Low | Optimized algorithm, profiling, SIMD |
| **Memory bloat** | Medium | Low | Buffer pooling, careful profiling |
| **Embedding quality poor** | High | Medium | Use proven teacher model, validate on examples |
| **Cross-language inconsistency** | High | Medium | Shared test suite, strict validation |

---

### 9.2 Operational Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Model file not available** | Medium | Low | Bundle with binary, fallback to URL |
| **Embedding table too large** | Low | Medium | Quantization, dimensionality reduction |
| **Search too slow at scale** | High | Medium | Indexing, caching, approximate search |

---

## 10. Alternative Approaches (Considered and Rejected)

### 10.1 Use Official SentencePiece via FFI

**Pros:** Official implementation, maintained

**Cons:**
- C++ dependency for Go/Rust/JS
- Complex cross-compilation
- Brittle across platforms
- Large binary size

**Verdict:** Rejected due to complexity and brittleness.

---

### 10.2 Use Hugging Face Tokenizers

**Pros:** Well-maintained, fast Rust implementation

**Cons:**
- Heavier dependency
- Not zero-allocation
- Less control over memory
- Larger binary

**Verdict:** Could be fallback, but prefer custom implementation for control.

---

### 10.3 Full Transformer for Embeddings

**Pros:** Best quality semantic matching

**Cons:**
- 100ms+ latency (too slow)
- 100MB+ model files
- Complex runtime

**Verdict:** Rejected for performance reasons. Static embeddings sufficient.

---

## 11. Success Metrics

### 11.1 Functional

- [ ] **Correctness:** 100% match with official SentencePiece on test suite
- [ ] **Coverage:** Handles all Unicode scripts (Latin, CJK, Emoji, etc.)
- [ ] **Robustness:** No crashes on malformed input

### 11.2 Performance

- [ ] **Latency:** <1ms p95 for tokenization (280 chars)
- [ ] **Throughput:** >10k tokenizations/second (single thread)
- [ ] **Memory:** <50KB per concurrent request
- [ ] **Search:** <100ms for 100 posts (full semantic search)

### 11.3 Quality

- [ ] **Semantic search:** "foot" matches "feet" with >0.7 similarity
- [ ] **Multilingual:** Works for English, emoji, CJK
- [ ] **Noise tolerance:** Handles typos gracefully

---

## 12. Conclusion

**Feasibility:** High. SentencePiece inference is straightforward to port.

**Complexity:** Medium. Main challenges are:
1. Trie implementation (Darts)
2. Memory optimization
3. Embedding generation pipeline

**Effort Estimate:** 6-8 weeks for full implementation across 3 languages

**Recommended Next Steps:**
1. Start with Rust implementation (most similar to C++)
2. Generate embedding table using Model2Vec approach
3. Validate with extensive testing
4. Port to Go and JS once Rust is solid

**Key Success Factor:** Thorough testing against official implementation to ensure correctness.

---

## Appendix A: Code Snippets

### A.1 Trie Result Structure

```rust
pub struct TrieResult {
    pub length: usize,  // Length of matched prefix in bytes
    pub value: i32,     // Token ID
}
```

### A.2 ViterbiNode Structure

```rust
pub struct ViterbiNode {
    pub best_score: f32,
    pub starts_at: usize,
    pub token_id: i32,
}
```

### A.3 Embedding Lookup (Rust)

```rust
pub fn get_embedding(&self, token_ids: &[u32]) -> Vec<f32> {
    let mut sum = vec![0.0; self.embedding_dim];
    let mut count = 0;
    
    for &token_id in token_ids {
        let offset = token_id as usize * self.embedding_dim;
        let embedding = &self.embeddings[offset..offset + self.embedding_dim];
        
        for (i, &val) in embedding.iter().enumerate() {
            sum[i] += val;
        }
        count += 1;
    }
    
    // Average pooling
    for val in &mut sum {
        *val /= count as f32;
    }
    
    // L2 normalize
    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    for val in &mut sum {
        *val /= norm;
    }
    
    sum
}
```

---

## Appendix B: References

1. **SentencePiece Paper:** Kudo, T., & Richardson, J. (2018). "SentencePiece: A simple and language independent approach to subword tokenization."
2. **Model2Vec:** Distillation approach documented in `docs/3-detour-model2vec.md`
3. **Darts (Double-Array Trie):** Aoe, J. (1989). "An Efficient Digital Search Algorithm by Using a Double-Array Structure."
4. **BPE-Dropout:** Provilkov, I., et al. (2019). "BPE-Dropout: Simple and Effective Subword Regularization."

---

**Document Version:** 1.0  
**Last Updated:** 2024  
**Authors:** Copilot AI Analysis  
**Status:** Ready for Implementation
