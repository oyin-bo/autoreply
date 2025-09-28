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
- Need: fast random access, mmap-friendly on-disk formats, stable cross-language codegen, small runtime for WASM.
- Cap'n Proto: best fit for zero-copy, small runtime, and direct memory persistence. Strong Rust and decent Go bindings. Good for writing compact AI-ready records.
- FlatBuffers: good alternative if schema evolution and language support are top priority; slightly higher runtime for some targets but still zero-copy.
- Protobuf: safe fallback when broad interoperability and tool maturity matter more than zero-copy performance.
- Arrow: only if heavy columnar analytics/embedding pipelines are primary.

## Recommendations (one-liners)

- If raw performance, small WASM artifacts, and low-level control matter -> Rust + capnproto + crates for atproto (use Rust CAR crates or implement light iterator). Prefer Rust for production-sensitive, low-latency MCP server.
- If fast development, single-binary deployment, and mature CAR tooling matter -> Go + go-car v2 + go-capnp. Prefer Go for faster iteration and simpler ops.
