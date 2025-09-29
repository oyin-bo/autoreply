# Cap'n Proto Store Specification (V1)

This document specifies a Cap'n Proto–based per‑account store that merges multiple incremental CAR downloads and attaches AppView enrichment (counts, viewer state). It replaces raw CAR persistence while keeping ingestion simple and fast.

The design targets both `rust-server/` and `go-server/` with identical on-disk layout and a shared `.capnp` schema. Where we need true in‑place updates, we use a fixed‑size counters sidecar that allows safe, constant-time increments without rewriting full records.

## Goals
- **Merge increments**: Append multiple CAR downloads into one store per account.
- **Selective capture**: Persist only essential fields from CAR records; simplify embeds/cids.
- **Enrichment**: Maintain like/reply/repost/quote counts and viewer flags; optionally fetched from AppView.
- **Fast updates**: Support in‑place updates for fixed-size counters via a dedicated sidecar.
- **Cross-account propagation**: Dispersion of likes/replies/etc. to the subject author's store.
- **Idempotent**: Safe on replays and partial failures.
- **Versioned & compactable**: Append-only with periodic compaction and schema versioning.

## Non-goals (V1)
- Full fidelity of every ATProto record detail (we intentionally simplify embeds, facets, etc.).
- Random-write growth of variable-sized fields in place (Cap'n Proto doesn't support arbitrary in-file reallocation).

## Storage layout (per account DID)
Directory: `…/accounts/<did>/`

- `repo.capnp.log` (append-only)
  - Sequence of Cap'n Proto messages of type `Envelope` (see schema). Each envelope holds a normalized record or a tombstone.
- `repo.index` (sidecar, binary)
  - Keyed by `(collection, rkey)` and by `cid` -> offset in `repo.capnp.log` and optional `countersSlotId`.
  - Append-only journal of index entries; compacted periodically into a dense map.
- `repo.counters` (sidecar, mmap'able, fixed-size slots)
  - Array of fixed-size `CountsSlot` records updated in place (64-bit counters, timestamps, flags).
  - Slots are referenced by `countersSlotId` from the index; allocate on first need.

Notes:
- The single, merged artifact is the append-only `repo.capnp.log`. Sidecars are small, rebuildable, and enable in-place counter updates.
- Optional: enable Cap'n Proto "packed" encoding for `repo.capnp.log` to reduce size.

## Minimal data model captured from CAR
We normalize ATProto records to a small set of kinds and fields. Additional CAR details can be recovered on demand from the network if necessary.

- **Common**: `repoDid`, `collection`, `rkey`, `cid?`, `createdAt`, `deleted?`.
- **Post** (`app.bsky.feed.post`): `text`, `langs[]`, `replyRef? {rootUri,parentUri}`, `embeds[]` (URLs only), `labels[]` (strings), `facets[]` simplified to `links[]`/`mentions[]`.
- **Like** (`app.bsky.feed.like`): `subjectUri`.
- **Repost** (`app.bsky.feed.repost`): `subjectUri`.
- **Follow** (`app.bsky.graph.follow`): `subjectDid`.
- **Block** (`app.bsky.graph.block`): `subjectDid`.
- **Label**: minimal `{uri,val,neg?}`.
- **Tombstone**: marks deletion of a prior record.

Enrichment (nullable): `likeCount`, `replyCount`, `repostCount`, `quoteCount`, `viewer {liked?, reposted?, following?, muted?}`, `updatedAtMs`. These are not rewritten into the envelope; they live in `repo.counters` for in-place updates, referenced via `countersSlotId`.

## Propagation/dispersion rules
When ingesting or fetching via AppView, we propagate side-effects to the subject author's store:

- **Like**: increment `likeCount` for the subject post in the subject author's store. Also store the like envelope in the liker’s store.
- **Repost**: increment `repostCount` for the subject post in the subject author's store. Store repost envelope in the reposter’s store.
- **Reply**: on a post with `replyRef.parentUri`, increment `replyCount` in the parent author’s store.
- **Quote**: if detected via embed semantics, increment `quoteCount` in the subject author’s store.

Idempotency: all increments are keyed by a stable op key `(opKind, actorDid, subjectUri, rkey/cid)` and deduplicated before applying to `repo.counters`.

## Operations and merge semantics
- **Ingest CAR**
  1) Parse CAR blocks, extract records of interest, build `Envelope` messages.
  2) Append envelopes to `repo.capnp.log`.
  3) Update `repo.index`: map `(collection,rkey)` and `cid?` -> offset; allocate a counters slot if the record is a post or another count-bearing object.
  4) Apply any dispersion increments (likes/reposts/replies) to the target author’s `repo.counters` using the index to resolve/create the slot.

- **Fetch feeds/threads/search (AppView)**
  - For posts returned, upsert or refresh counters in `repo.counters`; optionally append a lightweight `Envelope` if we captured a new object.

- **Compaction**
  - Rebuild `repo.capnp.log` with only the latest version per key, prune tombstoned entries, and rewrite `repo.index`. `repo.counters` persists as-is (slots may be remapped/deduped during compaction).

## Index and counters details
- `repo.index` entry: `{ key=(collection,rkey), cid?, offset, len?, deleted?, countersSlotId? }`.
- `repo.counters` slot (little-endian, fixed-size 56 bytes):
  - `likeCount: u64`
  - `replyCount: u64`
  - `repostCount: u64`
  - `quoteCount: u64`
  - `updatedAtMs: i64`
  - `flags: u64` (bit 0..3: known bits for each count; other bits reserved)

Unknown vs 0: a count is "unknown" until its bit is set in `flags`. This models nullability cleanly while keeping in-place updates trivial.

## Cap'n Proto schema sketch
The full schema will live under `schema/bsrepo.capnp` and be codegen'd for Rust and Go.

```capnp
@0xbf32d60fcf353fc3;

struct Envelope {
  ts @0 :Int64;                # monotonic source timestamp (ms)
  repoDid @1 :Text;            # DID of the repository the record belongs to
  collection @2 :Text;         # NSID (e.g., "app.bsky.feed.post")
  rkey @3 :Text;               # record key within the collection
  cid @4 :Text;                # optional CID string if known
  kind @5 :Kind;               # record kind
  deleted @6 :Bool;            # tombstone marker
  object @7 :Object;           # union payload for the record
}

enum Kind { post @0; like @1; repost @2; follow @3; block @4; label @5; tombstone @6; unknown @7; }

union Object {
  post @0 :Post;
  like @1 :Like;
  repost @2 :Repost;
  follow @3 :Follow;
  block @4 :Block;
  label @5 :Label;
  tombstone @6 :Void;
  unknown @7 :Void;
}

struct Post {
  text @0 :Text;
  createdAt @1 :Text;
  reply @2 :ReplyRef;          # optional
  embeds @3 :List(Text);       # simplified URLs only
  langs @4 :List(Text);
  labels @5 :List(Text);
  links @6 :List(Text);        # extracted URLs from facets
  mentions @7 :List(Text);     # DIDs from facets
}

struct ReplyRef { rootUri @0 :Text; parentUri @1 :Text; }
struct Like    { subjectUri @0 :Text; createdAt @1 :Text; }
struct Repost  { subjectUri @0 :Text; createdAt @1 :Text; }
struct Follow  { subjectDid  @0 :Text; createdAt @1 :Text; }
struct Block   { subjectDid  @0 :Text; createdAt @1 :Text; }
struct Label   { uri @0 :Text; val @1 :Text; neg @2 :Bool; }
```

Rationale: the envelope is immutable once written. All mutable enrichment lives in `repo.counters` (sidecar) addressed by index.

## Nullability of counts
- Counts are absent until first observed from AppView or inferred via local ops.
- Unknown counts are encoded as flags bit=0; zero counts set bit=1 with value=0.

## Versioning
- `storeVersion: u16` tracked in a small header prelude (first bytes of `repo.capnp.log`) and in the index file footer.
- `minReaderVersion` reserved for forward compatibility.

## Concurrency
- Single-writer per account store; readers use mmap + offsets from the index.
- File locks: advisory lock on `repo.capnp.log` and `repo.index` for writers.

## Error handling & integrity
- Each appended message in `repo.capnp.log` is length-prefixed; partial writes are ignored on next open (truncate to last valid message).
- Optional CRC32C per message for early corruption detection.
- Index rebuild path: full scan of the log to reconstruct `repo.index` and rebind counters.

## Implementation plan (Rust / Go)
- Rust: use `capnp` crate for schema I/O; implement `append_log()`, `scan_log()`, `index_load()`, `index_apply()`; counters sidecar via `memmap2` or `std::fs::File` with `seek`+`write`.
- Go: use `zombiezen.com/go/capnproto2`; same APIs mirrored; counters sidecar via `mmap` or `os.File.WriteAt`.
- Shared: schema under `schema/bsrepo.capnp`; codegen via `build.rs` (Rust) and `go generate` (Go).

## Cap'n Proto in-place updates — what is and isn't possible
- You can mutate fixed-size scalars in place if the object’s position in the file does not move (e.g., counters in a preallocated slot or sidecar). This is O(1) and safe with mmap/write-at.
- You cannot grow variable-sized fields (e.g., `Text`, `List`) in place without relocating objects. Cap'n Proto has no in-file allocator; growing requires rewrite/append and index update.
- Therefore, V1 uses an append-only log for records and a fixed-size counters sidecar for fast in-place updates.

## Open questions / future work
- Track viewer flags (liked/reposted/following) as separate bitsets in `repo.counters`.
- Optional zstd framing for the log segments.
- Background compaction heuristics (size or dead-record ratio).
- Richer embed modeling if needed (images, quotes, external embeds).

---
Summary: we merge CARs into a single append-only Cap'n Proto log per account, index keys to offsets, and maintain enrichment in a fixed-size counters sidecar that supports in-place updates. This yields fast ingestion and updates without whole-file rewrites.

