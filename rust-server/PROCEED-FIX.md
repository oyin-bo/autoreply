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
- Check CAR for that DID exists locally and if so, parse CAR and return `Repo`.
- Resolve the DID via `DidResolver` to the correct PDS endpoint.
- Download CAR.
- Store CAR in local cache, atomically.
- After async IO completes, parse the CAR synchronously using atrium-repo:
    - Call *atrium_repo::car::**CarRepoReader::new**(std::fs::File).read_repo()* directly.
    - The provider MUST NOT call `spawn_blocking` to offload parsing — callers invoke `get_repo` and use results.


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