# Proposal: The Semantic Annealing Engine for Fast, Compiler-like Embeddings

**Author:** mihailik
**Date:** 2025-10-03
**Status:** Final

## 1. Goal & Context

The dominant paradigm for generating high-quality semantic embeddings involves large, opaque neural network models (LLMs). While powerful, these models are computationally expensive and their reasoning is not inspectable.

This document proposes an alternative architecture: the **Semantic Annealing Engine**. The goal is to build a system that can produce medium-to-high-quality semantic embeddings with the following characteristics:
*   **Fast:** The final algorithm should be "fucking fast," suitable for execution on a GPU with a performance profile similar to a highly optimized compiler.
*   **Compiler-like:** The process should be systematic, inspectable, and based on transformations and optimizations rather than purely statistical inference.
*   **Zero-Allocation:** The core algorithm must be designed for a high-performance environment, avoiding dynamic memory allocation in its main loop to ensure cache-friendliness and GPU compatibility.

The core insight is to treat semantic interpretation not as parsing, but as a **Just-In-Time (JIT) compilation and optimization problem**. The input text is considered a naive, inefficient program that is progressively optimized into a compact, semantically rich program.

## 2. The Two-Layer System

The system is composed of two distinct layers:

1.  **The Runtime Engine (Semantic Annealing):** A deterministic, self-contained process that takes an input text and a given set of rules (an "algorithm") and produces a single output embedding. This is the fast, operational part of the system.
2.  **The Evolutionary Engine (Algorithm Discovery):** A meta-process that runs offline. Its purpose is to find the optimal set of rules for the Runtime Engine to use. This is the "learning" part of the system.

---

## 3. The Runtime Engine: Semantic Annealing

This is the core algorithm that runs for each piece of input text. It has no knowledge of any external benchmarks or teacher models. Its logic is entirely self-contained.

### 3.1 Building Blocks
*   **SentencePiece Tokenizer:** Breaks raw text into a sequence of tokens.
*   **The `ProgramStream`:** A single, linear buffer of `ProgramItem` structs. It is the unified representation of the program at all stages (input, intermediate, output).
*   **The Semantic Forth VM:** A tiny, stack-based virtual machine that executes a final `ProgramStream` to produce an embedding vector.
*   **The Pre-Packaged Algorithm:** A complete set of **Rewrite Rules** (patterns, actions, and scores) that are provided to the engine. This is its "DNA."

### 3.2 The Core Annealing Loop: Optimization with Integrated Fallback
The heart of the parsing process is the Semantic Annealing loop. **The de-optimization and refinement mechanism is a necessary and non-negotiable part of this loop.** It is not an error-handling condition; it is the primary mechanism for navigating the complex, non-linear search space of natural language interpretation.

The process for each step in the loop is as follows:

1.  **Find Opportunities:** The set of Rewrite Rules is used to scan the current `ProgramStream` and identify all possible optimizations ("Bids").

2.  **Select Best "Horse":** The system identifies the single `Bid` with the highest **internal score**.

3.  **Tentatively Apply Rewrite:** The highest-scoring optimization is provisionally applied to the stream. This may involve **reordering** items and **updating** their instructions.

4.  **Evaluate Internal Heuristic:** The system immediately evaluates the "health" or "potential" of this new, tentative program state. This is not a final embedding score. It is an internal, forward-looking heuristic, such as the sum of scores of all possible *future* rewrites that can now be applied.

5.  **Commit or Revert (The "Struggle and Regain Bearings" Step):**
    *   **If the heuristic score has improved or is acceptable:** The rewrite is **committed**. The change becomes the new baseline for the next iteration.
    *   **If the heuristic score has dropped significantly:** The rewrite has led to a dead end. The system **must** revert the change. This **de-optimization** is a productive step:
        *   The `ProgramStream` is restored to its previous state.
        *   The rule that led to the failure is temporarily inhibited.
        *   The system proceeds to try the next-best "horse" (the second-highest-scoring Bid from step 1), effectively exploring an alternative path.

This integrated cycle of greedy optimization and necessary backtracking is what allows the deterministic runtime engine to handle ambiguity and escape the local optima that would trap a simpler algorithm.

### 3.3 Final Output
After a fixed number of iterations, the loop terminates. The final, optimized `ProgramStream` is executed once by the Semantic Forth VM to produce the output embedding vector.

---

## 4. The Evolutionary Engine: Algorithm Discovery

This is the offline meta-process responsible for finding the optimal set of Rewrite Rules.

### 4.1 Granularity
The unit of evolution is one complete **"Algorithm"**—a full set of Rewrite Rules, Tries, weights, and parameters. The engine manages a population of these Algorithms.

### 4.2 The Competitive Fitness Evaluation
This is the only place where the teacher LLM is used.

1.  **Select an Algorithm:** Pick an Algorithm from the current population.
2.  **Run on Corpus:** Execute the **Runtime Engine** using this Algorithm's rules on a large corpus of texts. This generates a large set of output embeddings.
3.  **Calculate Fitness:** Compare the generated embeddings to the pre-computed, high-quality embeddings from the "teacher" LLM for that same corpus. The resulting aggregate similarity score is the **fitness** of that entire Algorithm.
4.  **Repeat:** Do this for every Algorithm in the population.

### 4.3 Evolution
Based on the fitness scores, the engine performs standard genetic operations (Selection, Crossover, Mutation) to create the next generation of Algorithms. This process competitively evolves a highly optimized set of rules that, when used by the self-correcting Runtime Engine, produce embeddings that are consistently similar to the teacher LLM's output.
