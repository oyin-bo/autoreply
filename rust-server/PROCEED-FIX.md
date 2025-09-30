# Atrium-repo crate integration

## Overview
- Tools (for example `src/tools/search.rs`, `src/tools/profile.rs`) must remain agnostic about how repositories are fetched or parsed. They only request a ready-to-iterate `Repo` handle.
- `RepositoryProvider` (defined in `src/bluesky/provider.rs`) owns all responsibilities previously split across the cache manager and other car parsing code.

## Responsibilities
### Tool layer
- Inject or construct a `RepositoryProvider`.
- Call `get_repo(did)` to obtain a parsed `atrium_repo::Repository` value.
- Iterate/filter records as needed; determining which collections matter is entirely tool-specific.

### Provider layer
- If handle (not a DID) is provided, resolve it to DID via HTTP call.
- Resolve the DID via `DidResolver` to the correct PDS endpoint.
- Download CAR by streaming the HTTP response directly into a temporary file placed in the same directory where the final cache file will live.
    - The temp file must use a randomized suffix (for example pid+timestamp or random bytes) so concurrent writes cannot collide.
    - The implementation must stream bytes straight to disk as they arrive (do not buffer the full CAR in memory).
    - After the stream completes the temp file must be flushed and fsynced, then atomically renamed into the final cache path (atomic rename in the same directory).
    - There must be NO collection or use of cache metadata, ETags, Last-Modified, content-length checks, timeouts, or size limits in this flow; the provider performs a straightforward network->file transfer.
- After async IO completes, parse the CAR synchronously using atrium-repo:
    - Call `atrium_repo::car::CarRepoReader::new(std::fs::File).read_repo()` directly on the file you just wrote.
    - The provider MUST NOT call `spawn_blocking` to offload parsing — callers invoke `get_repo` and use the returned repo directly.


3. **IO separation**
    - Network fetches remain async and are streamed directly to disk to avoid buffering large CARs in memory.
    - Parsing of the CAR into a `Repo` uses the synchronous atrium-repo APIs and is performed after the file is fully written and atomically placed in the cache directory.

## Provider method shape

```rust
// Return the concrete repository type produced by the CAR reader: a
// Repository parameterised over a file-backed CarStore.
pub async fn get_repo(
        &self,
        did: &str,
) -> Result<atrium_repo::Repository<atrium_repo::blockstore::CarStore<std::fs::File>>, AppError>
```

## Expected flow
```
tool -> RepositoryProvider::get_repo(did)
	  -> check cache / download CAR if needed
	  -> parse CAR -> Repo
	  -> return Repo for iteration (records, MST traversal, etc.)
```

## Outcomes
- No tool performs CAR parsing or caching directly.
- `CarProcessor` is fully superseded by `RepositoryProvider`.
- Tools can iterate posts, profile records, or other collections without extra allocations; record selection logic lives alongside tool-specific filters/reporting.

# REQUIREMENTS

Pseudocode for parsing the repository and reading records:

```rust
use atrium_repo::{car::CarRepoReader, Repository, blockstore::CarStore};
use std::fs::File;

fn main() {
    // Open the CAR file
    let file = File::open("repo.car").unwrap();

    // Parse the repo from CAR. The reader produces a Repository whose blockstore
    // is a file-backed CarStore; express the concrete returned type explicitly.
    let repo: Repository<CarStore<File>> = CarRepoReader::new(file).unwrap().read_repo().unwrap();

    // Iterate over records
    for (key, record) in repo.records() {
        println!("Key: {key}");
        println!("Record: {:?}", record);
    }
}
```

# LIMITATIONS

* No spawn_blocking is allowed for parsing. Use the snippet above.
* No wrappers or adapters allowed. Use atrium-repo types directly.