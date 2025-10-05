This package contains an experimental implementation of SentencePiece tokenization.

Status
------
The implementation is intentionally present in-tree for reference and targeted testing, but it is not included in default builds or server binaries.

How to exercise locally
----------------------
- Run tests that include the SentencePiece implementation:
  go test -tags=experimental_sentencepiece ./...

- Build with SentencePiece enabled:
  go build -tags=experimental_sentencepiece ./...

Rationale
---------
Keeping the code in-tree helps reviewers and allows focused testing without affecting normal CI and release builds. To re-enable globally, remove callers or add build tags to files that depend on this package in the main server code.
