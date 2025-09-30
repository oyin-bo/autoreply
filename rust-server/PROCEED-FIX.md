# Atrium-repo crate integration

## Overview
- Tools (for example `src/tools/search.rs`, `src/tools/profile.rs`) must remain agnostic about how repositories are fetched or parsed. They only request a ready-to-iterate `Repo` handle.
- `RepositoryProvider` (defined in `src/bluesky/provider.rs`) owns all responsibilities previously split across the cache manager and other car parsing code.

## Responsibilities
1. **Tool layer**
	- Inject or construct a `RepositoryProvider`.
	- Call `get_repo(did)` to obtain a parsed `atrium_repo::Repository` value.
	- Iterate/filter records as needed; determining which collections matter is entirely tool-specific.

2. **Provider layer**
	- Resolve the DID via `DidResolver` to determine the correct PDS endpoint when necessary.
	- Use `CacheManager` to check for an existing CAR, download if missing/stale, and persist both bytes and metadata.
	- Parse the CAR synchronously (e.g. within `spawn_blocking` if invoked from async) into an `atrium_repo::Repository` and hand it back to the caller.

3. **IO separation**
	- Network fetches and disk IO remain async to avoid blocking the runtime.
	- Parsing of the CAR into a `Repo` uses the synchronous atrium-repo APIs, isolated from the async path.

## Provider method shape

  ```rust
  pub async fn get_repo(&self, did: &str) -> Result<atrium_repo::repo::Repo, AppError>
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
use atrium_repo::{car::CarRepoReader, repo::Repo};
use std::fs::File;

fn main() {
    // Open the CAR file
    let file = File::open("repo.car").unwrap();

    // Parse the repo from CAR
    let repo: Repo = CarRepoReader::new(file).unwrap().read_repo().unwrap();

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