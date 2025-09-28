# Go/Rust detour for WASM-first MCP server

Short evaluation of a Rust vs Go detour for building a WASM-first MCP server (stdio or HTTP) that consumes BlueSky/AT Protocol data in CARs, repacks to Cap'n Proto for fast access, and exposes compact AI-friendly outputs.

## Assumptions

- Target: compile to WASM (wasm32-unknown-unknown or wasi) and native binaries.
- I/O modes: stdio (CLI/WASM stdio host) and HTTP (small MCP server).
- Storage: chunked CAR files (IPLD/CAR) as primary archive; Cap'n Proto as fast on-disk/in-memory representation.
- No implementation details here — only high-level skeleton and library assessment.

## Rust approach

- Core: small async runtime (optional) + modular crates.
- Components:
	- atproto client/identity: HTTP client + DID/OAuth helpers (crate-based).
	- CAR ingestion: streaming CAR iterator, indexer, random-access reader.
	- Repack layer: transform selected CAR blocks -> Cap'n Proto message(s), persist mmap-able files.
	- WASM/host glue: compile library to WASM with thin shim exposing stdio/HTTP handler hooks.
	- MCP server: lightweight native HTTP (w/ hyper/axum) and stdio adapter for WASI.
	- Optional: Jetstream/WebSocket consumer for live events.

### Tradeoffs: Rust

- Pros: strong type-safety, zero-cost abstractions, excellent Cap'n Proto crates, good CAR crates on crates.io, predictable performance, small stable WASM output.
- Cons: steeper contributor ramp; async ecosystem fragmentation (but stable options exist).

## Go approach

- Core: single binary, goroutine concurrency, standard library HTTP.
- Components:
	- atproto client/identity: HTTP client + OAuth/PKCE helpers (Go package).
	- CAR ingestion: go-car v2 for indexed/random access and streaming.
	- Repack layer: Cap'n Proto bindings, write mmap-friendly files or keep in-memory caches.
	- WASM/host glue: build wasm with GOOS=js/wasm for browser-like hosts or tiny native server binary.
	- MCP server: net/http or chi/fasthttp server; stdio adapter via os.Stdin/os.Stdout.
	- Optional: Jetstream consumer with goroutines and channels.

### Tradeoffs: Go

- Pros: fastest developer onboarding, single-binary deployment, mature go-car, Cap'n Proto bindings available, straightforward concurrency.
- Cons: larger binaries, less zero-copy memory tricks compared to Rust, WASM support is workable but larger runtime.

## Library and ecosystem review

### AT Protocol / BlueSky
- Go: **indigo** (bluesky-social/indigo) contains Go server components; general HTTP clients exist but no single official Go client library. go ecosystem: use standard HTTP + lexicons as JSON.
- Rust: multiple crates on crates.io (e.g., **atproto, atproto-client, atproto-identity, atproto-oauth, atproto-jetstream**). Recent, actively published crates (see crates.io). Good for native client + OAuth.

### OAuth support: Rust vs Go (specifics)

This section lists concrete crates/repos, the OAuth primitives they implement for AT Protocol (DPoP / PKCE / PAR / private_key_jwt), short maturity notes, and brief WASM suitability guidance.

#### Rust

- Key crates to evaluate:
	- [atproto-oauth](https://docs.rs/atproto-oauth) — provides modules: `dpop`, `pkce`, `jwk`, `jwt`, `workflow`, `storage`, `resources`.
	- [atproto-client](https://docs.rs/atproto-client) — HTTP client with DPoP authentication support and CLI helpers (atproto-client-dpop, atproto-client-auth).
	- [atproto-oauth-aip](https://docs.rs/atproto-oauth-aip) / provider helpers (docs.rs listings) — useful if you need AIP/provider-side workflows (PAR/session exchange helpers).
	- [atproto-identity](https://docs.rs/atproto-identity) / DID utilities (docs.rs) — DID resolution and key utilities used by the OAuth flow.

- Feature coverage (what's implemented):
	- DPoP: supported via the `dpop` module (DPoP JWT creation and verification helpers).
	- PKCE: supported via the `pkce` module (S256 code challenge/verifier helpers).
	- PAR (Pushed Authorization Requests): workflow and storage modules provide request storage and helpers for composing/sending PARs.
	- private_key_jwt (client assertion): `jwk` + `jwt` modules provide JWK generation/parsing and JWT creation needed for confidential client assertions.

- Maturity and activity:
	- These crates are published and documented on docs.rs; they present a complete set of OAuth primitives for AT Protocol flows. Docs.rs presence + crate packaging indicates the libraries are intended for reuse. Verify the crate version and recent publish dates on crates.io for the most up-to-date activity signal.

- WASM suitability:
	- Rust is the strongest option for compact WASM artifacts. Most of these crates are written in pure Rust and can be compiled to wasm32 targets, but you must audit crypto dependencies (e.g., usage of native C libs or ring/openssl) — ES256 (ECDSA P-256) signing must be available in the target (use Rust crates that are wasm-compatible or feature-gated to use pure-Rust implementations). Prefer wasm32-wasi or wasm32-unknown-unknown with small adapters; expect smallest artifacts from Rust.

#### Go

- Key repositories / packages to evaluate:
	- [haileyok/atproto-oauth-golang](https://github.com/haileyok/atproto-oauth-golang) — an experimental but practical implementation that implements the full client flow (PAR, PKCE, DPoP, client assertion, token exchange, XRPC client integration). Note: repository archived by the owner (read-only) as of Sep 7, 2025.
	- [streamplace/atproto-oauth-golang](https://github.com/streamplace/atproto-oauth-golang) — a fork of the above (GitHub), similar codebase.
	- [bluesky-social/indigo](https://github.com/bluesky-social/indigo) — the canonical Go ecosystem for AT Protocol; contains `xrpc`, `identity`, `crypto` packages and is actively maintained, but it does not provide a single, consolidated OAuth client package — you'll often combine `indigo` primitives with community OAuth helpers.
	- Community packages on pkg.go.dev (example: https://pkg.go.dev/tangled.sh/icyphox.sh/atproto-oauth) provide helpers and examples.

- Feature coverage (what's implemented):
	- DPoP: helper functions to create per-request DPoP JWTs and utilities to manage the PDS nonce (e.g., `PdsDpopJwt`, `AuthServerDpopJwt`).
	- PKCE: helpers to create code challenge/verifier pairs and persist the verifier across the redirect.
	- PAR: functions to make pushed authorization requests (`SendParAuthRequest`) and return `request_uri`/state/nonce metadata.
	- private_key_jwt: client assertion helpers (`ClientAssertionJwt`) for confidential client authentication (ES256).
	- XRPC integration: `XrpcClient.Do` with `XrpcAuthedRequestArgs` to make authenticated requests using DPoP + access token.

- Maturity and activity:
	- [haileyok/atproto-oauth-golang](https://github.com/haileyok/atproto-oauth-golang) is the most feature-complete example and documents all needed primitives; however it is flagged experimental and was archived (read-only) on Sep 7, 2025 — good as a reference or short-term dependency, but treat it as community/experimental code.
	- [indigo](https://github.com/bluesky-social/indigo) is actively maintained (many contributors, ~1.2k stars) and contains robust building blocks (crypto, identity, xrpc), but you will need to implement or glue the OAuth client flow on top of it (or adapt community implementations).

- WASM suitability:
	- Go supports compiling to WebAssembly via the standard toolchain (GOOS=js GOARCH=wasm) and TinyGo for smaller binaries, but Go WASM artifacts are typically larger than Rust equivalents due to Go runtime size.
	- Crypto: ES256 (ECDSA P-256) signing is available in Go's `crypto/ecdsa` on native toolchain; TinyGo may lack full crypto/elliptic support depending on target/versions — test signing on your chosen WASM toolchain before committing. Overall, Go can work for WASM but expect larger outputs and more caveats around crypto support if you use TinyGo.

#### Quick verdict for OAuth support

- Rust: Very good. The `atproto-*` crate family exposes DPoP, PKCE, JWK/JWT primitives, PAR/workflow helpers and an authenticated client. If you need compact WASM artifacts and want a full-featured, crate-based OAuth flow out-of-the-box, Rust is preferable — just audit crypto backends for your chosen wasm target.

- Go: Good (practical, mature building blocks). The official `indigo` repo provides the core atproto primitives (xrpc, identity, crypto), and community repos (e.g., `haileyok/atproto-oauth-golang`) implement complete OAuth flows including DPoP/PKCE/PAR/private_key_jwt. The main caveat: the most complete community example is archived/experimental, and Go WASM builds tend to be larger and may need TinyGo testing for crypto support.

### CAR / IPLD
- Go: **ipld/go-car (v2)** — production-ready, index support, blockstore API, examples. Strong choice for large CAR files and random access.
- Rust: **rust-car /** related crates exist but less centralised; crates.io shows multiple 'atproto' and 'car' crates. Expect more polishing work than Go.

### Cap'n Proto
- Rust: **capnproto-rust** — mature, codegen (capnpc), capnp-futures, capnp-rpc. Supports no_std and no-alloc, good for WASM targets.
- Go: **go-capnp / go-capnp v3** — maintained, codegen + runtime, supports RPC level1. Good tooling and stable.

### Alternatives to Cap'n Proto
- FlatBuffers: zero-copy read, multi-language, mature (Google). Good for schema evolution, smaller runtime than Protobuf. Rust + Go support exists.
- Protocol Buffers (protobuf): ubiquitous tooling, compact, wide support. Less zero-copy, more encode/decode overhead.
- Apache Arrow: columnar, excellent for analytics/large-scale vectorised reads (useful if doing heavy vector embedding pipelines). Heavier and different use-case.
- MessagePack / CBOR: simple compactness, schema-less; lower structural guarantees than Cap'n Proto.

#### How they stack up for this project

Need: fast random access, mmap-friendly on-disk formats, stable cross-language codegen, small runtime for WASM.

- **Cap'n Proto:** best fit for zero-copy, small runtime, and direct memory persistence. Strong Rust and decent Go bindings. Good for writing compact AI-ready records.
- **FlatBuffers:** good alternative if schema evolution and language support are top priority; slightly higher runtime for some targets but still zero-copy.
- **Protobuf:** safe fallback when broad interoperability and tool maturity matter more than zero-copy performance.
- **Arrow:** only if heavy columnar analytics/embedding pipelines are primary.

## Recommendations (one-liners)

- If **raw performance,** small WASM artifacts, and low-level control matter -> Rust + capnproto + crates for atproto (use Rust CAR crates or implement light iterator). Prefer Rust for production-sensitive, low-latency MCP server.
- If **fast development,** single-binary deployment, and mature CAR tooling matter -> Go + go-car v2 + go-capnp. Prefer Go for faster iteration and simpler ops.
