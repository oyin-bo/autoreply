# Distill LLM Semantic Judgments into a Fast, Pattern-Based Adjective-Space Classifier

### 🔹 Goal
Build a runtime engine that maps short-form text (e.g. tweets, queries) to a fixed set of semantic dimensions (adjective phrases like “sad”, “chatty”, “office-like”) using only primitive code and static lookups. The engine must approximate the behavior of a large LLM while remaining interpretable, fast (<100µs), and deployable in JS/WASM.

---

### 🔹 Dimensions
Define a fixed set of 50–200 adjective phrases representing semantic and emotional axes. Each dimension must be clearly interpretable and semantically distinct. Dimensions must be frozen before pattern attribution begins.

---

### 🔹 LLM Role
Use a large LLM (e.g. Gemma, GPT-4, Claude 2) to:
- Assign scores to each sentence across all dimensions.
- Explicitly list the exact words, phrases, or patterns that justify each score.
- Forbid any explanations; demand concrete patterns from text.

---

### 🔹 Corpus
Use a large, diverse corpus of short-form texts (≥10M samples). In practice these often form conversations or threads, so each next text is contextualised by the previous ones. LLM will produce scores for each text in context (text+history before).

Each text (tweet?) must be annotated with:
- LLM-generated dimension scores.
- LLM-extracted contributing patterns.

---

### 🔹 Pattern Attribution
For each extracted pattern:
- Measure frequency across corpus.
- Quantify predictive strength for each dimension using statistical metrics (e.g. lift, mutual information, regression).
- Discard low-signal or ambiguous patterns.
- Normalize contributions to avoid over-weighting frequent patterns.

---

### 🔹 Runtime Engine
Implement a lightweight engine that:
- Scans input.
- Matches known patterns using a set of compiler-grade zero-allocation techniques such as lookup, partial match, interleaved match etc.
- Aggregates dimension contributions across the "tweet" short input.
- Mixes in historical context of the conversation in the form of:
  - Recent dimension vectors.
  - If the vector is by the analysed text author.
  - If the vector is by the author written the text immediately preceding the analysed one.
  - How many texts the author of the vector has written in the conversation so far.
- Outputs normalized semantic snapshot vector by weighted, tuned up formula.

Engine must:
- Be a straight algorithm, no LLM.
- Operate in <100µs per sentence.
- Be portable to JS/WASM.

---

### 🔹 Evaluation
Compare runtime output to LLM scores on held-out corpus. Measure divergence per dimension. Optimize pattern weights to minimize error. Apply guardrails to suppress noisy or misleading patterns.

---

### 🔹 Deliverables
- Pattern–dimension lookup table.
- Runtime engine (Python or JS).
- Evaluation suite.
- Documentation of dimensions, prompts, attribution logic.
