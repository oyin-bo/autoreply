# SentencePiece/Tokenization: Viterbi Algorithm and Result Extraction

### 1. High-Level Inference Orchestration

The process starts in the `SentencePieceProcessor::Encode` method. This function orchestrates the steps but delegates the heavy lifting to the normalizer and the model.

```pseudocode
function SentencePieceProcessor_Encode(inputText):
  // Step 1: Normalize the input text.
  // This involves Unicode normalization (e.g., NFKC), replacing whitespace, etc.
  // It also keeps a map to link positions in the normalized text back to the original.
  normalizedText, positionMap = Normalizer_Normalize(inputText)

  // Step 2: Use the model to encode the normalized text into pieces.
  // This is where the core Viterbi algorithm is executed.
  // The result is already a linear sequence of the best tokens.
  tokenSequence = UnigramModel_Encode(normalizedText)

  // Step 3: Populate the final output structure.
  // This converts the internal token sequence into the final user-facing format,
  // handling special cases like unknown tokens and byte-fallback.
  finalOutput = PopulateSentencePieceText(inputText, normalizedText, positionMap, tokenSequence)

  return finalOutput
```

### 2. Unigram Model Encoding with Viterbi Algorithm

This is the heart of the tokenization process. The goal is to find the sequence of tokens (sentence pieces) that has the highest probability, given the input text. The Unigram model assumes that each token's probability is independent of the others. The Viterbi algorithm efficiently finds this best path.

The algorithm builds a lattice (a directed acyclic graph) where nodes represent positions in the input string, and edges represent possible tokens spanning between positions.

```pseudocode
function UnigramModel_Encode(normalizedText):
  // Let N be the length of normalizedText.
  // `nodes` array will store the best tokenization path up to each position.
  // Each element stores { best_score, token_id, starting_position_of_token }
  nodes = array of size (N + 1)
  initialize nodes with a very low score for all positions except the start.

  // The node at position 0 is the starting point.
  nodes[0] = { score: 0.0, token_id: -1, starts_at: -1 }

  // --- Forward Step: Build the Viterbi lattice ---
  // Iterate through each character position in the normalized text.
  for pos from 0 to N-1:
    // If no path can reach this position, we can't proceed from here.
    if nodes[pos].score is very_low:
      continue

    // From the current position `pos`, look for all possible tokens
    // in the vocabulary that start at this position.
    // The vocabulary is typically stored in a Trie for efficient prefix matching.
    matchedTokens = vocabularyTrie.FindAllTokensStartingAt(normalizedText, pos)

    // For each token found, calculate the score if we were to use it.
    for token in matchedTokens:
      tokenScore = GetScoreFromVocabulary(token.id)
      newScore = nodes[pos].score + tokenScore
      endPos = pos + token.length

      // If this new path is better than any previous path to `endPos`, update it.
      if newScore > nodes[endPos].score:
        nodes[endPos] = { score: newScore, token_id: token.id, starts_at: pos }

  // --- Backward Step: Backtrack to find the best path ---
  // The Viterbi algorithm finds the highest-scoring path ending at the last node.
  // Now, we trace it backward from the end to reconstruct the token sequence.
  tokenSequence = empty list
  currentPos = N // Start from the end of the text

  while currentPos > 0:
    // Get the best token that ended at `currentPos`.
    bestTokenEndingHere = nodes[currentPos]

    // Prepend the token to our result sequence.
    tokenPiece = GetPieceFromVocabulary(bestTokenEndingHere.token_id)
    tokenSequence.prepend({ piece: tokenPiece, id: bestTokenEndingHere.token_id })

    // Move to the starting position of this token to find the next one.
    currentPos = bestTokenEndingHere.starts_at

  return tokenSequence
```

### 3. Extracting Results into a Linear Sequence

As shown in the pseudo-code above, the Viterbi algorithm itself doesn't directly produce a linear sequence in the forward pass. It produces a `nodes` array that holds all the information needed to find the best path.

The **"Backward Step"** is the crucial part that extracts the final, linear sequence of tokens. It works as follows:

1.  It starts from the very last node in the `nodes` array (the one corresponding to the end of the string). This node represents the end of the highest-scoring tokenization path.
2.  It retrieves the token that led to this best final state. This token is added to the beginning of our result list.
3.  It then "jumps" backward to the node where that token began (`starts_at`).
4.  It repeats this process—finding the token that led to the current node, prepending it to the results, and jumping back—until it reaches the beginning of the string (position 0).

This backward traversal effectively reconstructs the single best path through the lattice, yielding the final, unambiguous, linear sequence of tokens.