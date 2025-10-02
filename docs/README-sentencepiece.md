# SentencePiece Fuzzy Search Documentation

This directory contains comprehensive documentation for implementing SentencePiece-based semantic fuzzy search in the Autoreply MCP server.

## 📚 Documents

### [12-sentencepiece-fuzzy-search-analysis.md](./12-sentencepiece-fuzzy-search-analysis.md)
**Main Document** - Comprehensive technical analysis (~1200 lines)

**Contents:**
1. **Executive Summary** - Key findings and recommendations
2. **Architecture Overview** - Core components and pipeline
3. **Algorithm Analysis** - Detailed breakdown of normalization, Viterbi, BPE
4. **Memory Allocation** - Zero-allocation strategies and buffer pooling
5. **Semantic Search Strategy** - Embedding generation and similarity calculation
6. **Language-Specific Guides** - Implementation details for Go, Rust, JS
7. **Testing Strategy** - Correctness, performance, edge cases
8. **Implementation Roadmap** - 5-phase plan with deliverables
9. **Risk Assessment** - Technical and operational risks with mitigation
10. **Appendices** - Code snippets, references, alternative approaches

**Target Audience:** Technical architects, senior engineers planning implementation

---

### [12.1-sentencepiece-quick-start.md](./12.1-sentencepiece-quick-start.md)
**Quick Reference** - Practical implementation guide (~400 lines)

**Contents:**
- TL;DR summary of the approach
- Quick architecture diagram
- Core components to implement
- Implementation order by language
- Data files needed
- Testing checklist
- Example usage code
- Key algorithms (simplified pseudocode)
- Performance tips
- Common pitfalls
- Next steps timeline

**Target Audience:** Developers ready to start implementation

---

## 🎯 Problem Statement

**Current State:** The Autoreply MCP server uses Unicode-normalized case-insensitive string matching for search.

**Limitation:** This doesn't match semantically similar words:
- "foot" doesn't match "feet"
- "run" doesn't match "running"
- "child" doesn't match "children"

**Desired State:** Semantic fuzzy search that understands linguistic similarity.

---

## 💡 Solution Overview

```
┌──────────────┐
│  Query Text  │
└──────┬───────┘
       │
       ▼
┌──────────────────────┐
│ 1. SentencePiece     │  Text → Token IDs
│    Tokenization      │  (Viterbi algorithm)
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ 2. Static Embedding  │  Token IDs → 64D vector
│    Lookup            │  (average + normalize)
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│ 3. Cosine Similarity │  Query vector vs Post vectors
│    Ranking           │  → Top-K results
└──────────────────────┘
```

**Key Innovation:** Use static embeddings (Model2Vec approach) for fast, semantic-aware matching without runtime ML inference.

---

## 📊 Key Metrics

### Implementation Effort
- **Rust (reference):** 2 weeks
- **Go (server):** 1 week
- **JavaScript (web):** 1 week
- **Total:** 4-6 weeks for all three languages

### Performance Targets
- **Tokenization:** <1ms per tweet (280 chars)
- **Embedding lookup:** <0.1ms per 50 tokens
- **Full search:** <100ms for 100 posts
- **Memory:** <50KB per concurrent request

### Quality Goals
- **Semantic matching:** "foot" ↔ "feet" similarity >0.7
- **Multilingual:** Support English, CJK, emoji
- **Robustness:** Handle malformed UTF-8, edge cases

---

## 🔧 Technical Approach

### Direct Port (Recommended)
✅ **Port SentencePiece inference algorithm to target languages**

**Rationale:**
- Inference-only is relatively simple (~1000 lines)
- Full control over memory and performance
- No C/C++ dependencies
- Cross-platform consistency

**Components:**
1. Normalizer (Unicode + whitespace handling)
2. Tokenizer (Viterbi or BPE)
3. Double-Array Trie (vocabulary lookup)
4. Embedding table (memory-mapped)

### Alternative: FFI Bindings (Rejected)
❌ **Use official SentencePiece C++ library via FFI**

**Why rejected:**
- Brittle cross-compilation
- Platform-specific issues
- Large binary size
- Less control over memory

---

## 📖 How to Use This Documentation

### For Architects / Decision Makers
1. Read the Executive Summary in [12-sentencepiece-fuzzy-search-analysis.md](./12-sentencepiece-fuzzy-search-analysis.md)
2. Review Section 7 (Implementation Roadmap) for timeline
3. Check Section 9 (Risk Assessment) for risks

### For Developers Starting Implementation
1. Start with [12.1-sentencepiece-quick-start.md](./12.1-sentencepiece-quick-start.md)
2. Follow the "Implementation Order" section
3. Use pseudocode examples as reference
4. Refer back to full analysis for algorithm details

### For Code Reviewers
1. Check "Testing Checklist" in quick start guide
2. Review "Success Metrics" in Section 11 of main doc
3. Validate against official SentencePiece outputs

---

## 🗂️ Related Documentation

- **[3-detour-model2vec.md](./3-detour-model2vec.md)** - Static embedding approach (aligns with this strategy)
- **[5-detour-tokenisation-stencepiece.md](./5-detour-tokenisation-stencepiece.md)** - Viterbi algorithm explanation
- **[`-sentence-piece-inference-tmp/`](../-sentence-piece-inference-tmp/)** - Original C++ reference implementation

---

## 🔍 Reference Code

The repository includes the original SentencePiece C++ inference code for reference:

```
-sentence-piece-inference-tmp/
├── sentencepiece_processor.cc/.h    # Main processor
├── unigram_model.cc/.h              # Unigram + Viterbi
├── bpe_model.cc/.h                  # BPE algorithm
├── normalizer.cc/.h                 # Text normalization
├── model_interface.cc/.h            # Vocabulary interface
├── freelist.h                       # Memory pool
└── sentencepiece_model.proto        # Model format
```

**Study these files to understand:**
- Lattice construction (`unigram_model.cc`)
- Viterbi algorithm (`Lattice::Viterbi()`)
- Normalization rules (`Normalizer::NormalizePrefix()`)
- Trie usage (`commonPrefixSearch`)

---

## 🚀 Getting Started

**Week 1: Setup and Study**
```bash
# 1. Study the analysis document
open docs/12-sentencepiece-fuzzy-search-analysis.md

# 2. Review original C++ code
cd -sentence-piece-inference-tmp/
grep -n "Viterbi" unigram_model.cc

# 3. Set up development environment
# (Rust, Go, or JS based on your target)
```

**Week 2-3: Core Implementation**
```bash
# Follow the quick start guide
open docs/12.1-sentencepiece-quick-start.md

# Implement in order:
# 1. Protobuf parser
# 2. Trie structure
# 3. Normalizer
# 4. Tokenizer
```

**Week 4-5: Embeddings and Search**
```bash
# 1. Generate embedding table
python scripts/generate_embeddings.py

# 2. Implement embedding lookup
# 3. Build search API
# 4. Test quality
```

**Week 6-8: Ports and Polish**
```bash
# 1. Port to other languages
# 2. Optimize performance
# 3. Integration testing
# 4. Documentation
```

---

## ✅ Success Criteria

### Must Have
- [ ] Tokenization matches official SentencePiece output
- [ ] Handles Unicode (Latin, CJK, emoji) correctly
- [ ] <1ms tokenization latency (p95)
- [ ] Semantic matching works (verified on test cases)

### Should Have
- [ ] Zero heap allocations per request
- [ ] <100ms full search for 100 posts
- [ ] Multilingual support (3+ scripts)
- [ ] WASM build for browser use

### Nice to Have
- [ ] <0.5ms tokenization (stretch goal)
- [ ] <50ms search for 1000 posts
- [ ] Approximate nearest neighbor for scale
- [ ] Custom model trained on social media

---

## 💭 Design Decisions

### Why SentencePiece?
✅ Subword tokenization (handles "foot" → "feet" better than words)  
✅ Language-agnostic (works for any Unicode)  
✅ Small vocabulary (~32k) fits in memory  
✅ Fast inference with Viterbi  

### Why Static Embeddings?
✅ <100µs inference (no transformer runtime)  
✅ Works in WASM/browser  
✅ Predictable memory (<10MB)  
✅ Good-enough quality for search  

### Why Port vs FFI?
✅ No C++ dependency chain  
✅ Full memory control  
✅ Cross-platform consistency  
✅ Easier debugging  

---

## 🤝 Contributing

When implementing or improving this system:

1. **Test thoroughly** against official SentencePiece
2. **Profile before optimizing** - measure don't guess
3. **Document edge cases** you discover
4. **Update this documentation** if approach changes
5. **Share learnings** with the team

---

## 📞 Questions?

If something is unclear:
1. Check the full analysis document first
2. Study the original C++ implementation
3. Run experiments with official SentencePiece
4. Document your findings for others

---

**Version:** 1.0  
**Last Updated:** 2024  
**Status:** Ready for Implementation  
**Next Step:** Begin Phase 1 (Core Inference)
