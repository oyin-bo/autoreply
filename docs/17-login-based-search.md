# Login-Based Search Implementation Plan

## Overview

This document defines the implementation plan for adding a `login` parameter to the search tool, enabling authenticated BlueSky API searches alongside the existing CAR-based repository searches.

## Current State

### Existing Search Implementation

All three implementations (JavaScript, Rust, Go) currently support:
- **CAR-based search**: Downloads and searches through user's repository CAR files
- **Required parameter**: `account` (handle or DID)
- **Required parameter**: `query` (search terms)
- **Optional parameter**: `limit` (max results)
- **Output format**: Markdown following /docs/16-mcp-schemas.md conventions

### Authentication/Credential Storage

All implementations have credential storage:
- **Rust**: `CredentialStorage` in `rust-server/src/auth/storage.rs`
  - Supports keyring and file-based storage
  - Stores credentials with handle as key
  - Stores sessions (access/refresh tokens)
  
- **Go**: `CredentialStore` in `go-server/internal/auth/credentials.go`
  - Uses keyring library with file fallback
  - Stores credentials by handle key pattern: `user:{handle}`
  
- **JavaScript**: Uses keytar/keychain
  - Stores password by handle
  - Stores default handle separately
  - Caches authenticated agents in `_clientLoggedInByHandle`

## New Feature: Login-Based Search

### Parameters

The search tool will accept a new optional parameter:

- `login` (string, optional): A login account name that was previously stored and cached
  - When provided, enables authenticated BlueSky API search
  - Must be normalized before matching against stored credentials
  - If the login cannot be resolved to cached credentials, return an error

### Search Behavior

#### When `login` is NOT specified:
- **Behavior unchanged**: Use existing CAR-based search only
- `account` parameter remains required

#### When `login` IS specified:

1. **Normalize login parameter**: Apply handle normalization (remove @, add .bsky.social if needed, lowercase)

2. **Resolve cached credentials**: 
   - Look up normalized login in credential storage
   - If not found, return error: "Login '{login}' not found. Please login first using the login tool."

3. **Account parameter handling**:
   - If `account` is provided:
     - Perform BOTH CAR-based search AND authenticated API search
     - Merge and deduplicate results
   - If `account` is NOT provided (made optional when login is specified):
     - Perform ONLY authenticated API search
     - No CAR-based search

4. **Authenticated API search**:
   - Use the BlueSky API endpoint: `app.bsky.feed.searchPosts`
   - Authenticate using stored credentials for the resolved login
   - Pass the query parameter to the API
   - Respect limit parameter
   - **Note**: API search results are NOT cached (unlike CAR files)

5. **Result merging** (when both searches are performed):
   - Deduplicate posts by URI (at:// URI is unique identifier)
   - For duplicate posts, merge AppView stats (likes, reposts, replies, quotes) from API result
   - Prefer API result metadata over CAR result when available

6. **Output format**:
   - Use the same Markdown format defined in /docs/16-mcp-schemas.md
   - For posts that are replies (detected in either CAR or API results):
     - Format with thread indicators (└─, indentation)
     - Use super-compact refkey syntax for threaded replies (@handle/…refkey)
   - When a post and its reply appear side-by-side, display as threaded
   - Include stats: 👍 {likes}  ♻️ {reposts}  💬 {replies}  {timestamp}

## Implementation Details

### Rust Implementation

**File**: `rust-server/src/tools/search.rs`

1. Update `SearchArgs` struct:
   ```rust
   pub struct SearchArgs {
       #[arg(short = 'a', long)]
       pub account: Option<String>,  // Make optional
       
       #[arg(short = 'q', long)]
       pub query: String,
       
       #[arg(short = 'l', long)]
       pub limit: Option<usize>,
       
       #[arg(long)]
       pub login: Option<String>,  // Add login parameter
   }
   ```

2. Update `execute_search` function:
   - Add validation: require either `account` or `login`
   - If `login` is provided:
     - Normalize the login handle
     - Load credentials from `CredentialStorage`
     - Create authenticated session
     - Perform API search via `app.bsky.feed.searchPosts`
   - If both `account` and `login` provided:
     - Run CAR search and API search concurrently
     - Merge and deduplicate results
   - Format output according to /docs/16-mcp-schemas.md

3. Add helper functions:
   - `normalize_login_handle(login: &str) -> String`
   - `authenticate_from_login(login: &str) -> Result<Session>`
   - `search_via_api(session: &Session, query: &str, limit: usize) -> Result<Vec<PostRecord>>`
   - `merge_search_results(car_posts: Vec<PostRecord>, api_posts: Vec<PostRecord>) -> Vec<PostRecord>`
   - `deduplicate_by_uri(posts: Vec<PostRecord>) -> Vec<PostRecord>`

### Go Implementation

**File**: `go-server/internal/tools/search.go`

1. Update `SearchTool.InputSchema()`:
   ```go
   Properties: map[string]mcp.PropertySchema{
       "account": {
           Type:        "string",
           Description: "Handle or DID (optional when login is provided)",
       },
       "query": {
           Type:        "string",
           Description: "Search terms",
       },
       "limit": {
           Type:        "number",
           Description: "Maximum results (default 50, max 200)",
       },
       "login": {
           Type:        "string",
           Description: "Login account name for authenticated search",
       },
   }
   Required: []string{"query"},  // Only query is required
   ```

2. Update `validateInput` function:
   - Check for either `account` or `login` parameter
   - Extract and validate `login` if provided

3. Update `Call` function:
   - If `login` provided:
     - Normalize login handle
     - Load credentials from `CredentialStore`
     - Create authenticated BlueSky client
     - Perform API search
   - If both `account` and `login`:
     - Execute both searches
     - Merge and deduplicate
   - Format output per /docs/16-mcp-schemas.md

4. Add helper functions:
   - `normalizeLoginHandle(login string) string`
   - `authenticateFromLogin(store *auth.CredentialStore, login string) (*bluesky.Client, error)`
   - `searchViaAPI(client *bluesky.Client, query string, limit int) ([]*PostRecord, error)`
   - `mergeSearchResults(carPosts, apiPosts []*PostRecord) []*PostRecord`
   - `deduplicateByURI(posts []*PostRecord) []*PostRecord`

## Output Format Details

Following /docs/16-mcp-schemas.md, search results will be formatted as:

```markdown
# Search Results · {count} posts

@alice/3kq8a3f1
> Hot take: Markdown > JSON for LLM tools
👍 234  ♻️ 89  💬 45  2024-10-06T10:15:33Z

└─@a/…a3f1 → @bob/3kq8b2e4
> Agree! But what about content escaping?
👍 12  2024-10-06T10:18:56Z
```

### Thread Formatting Rules

1. **Thread indicator**: `└─` or `  └─` with indentation for nested replies
2. **Reply reference**: `@{first-letter}/…{last-4-of-refkey} → @{handle}/{refkey}`
   - Use compact form only if parent is in current result set
   - Use full form if parent is not in result set
3. **Content**: Block-quoted with `>` prefix on each line
4. **Stats**: `👍 {likes}  ♻️ {reposts}  💬 {replies}  {timestamp}`
5. **Images**: Markdown image notation within blockquote: `> ![alt text](url)`

## Testing Requirements

### Rust Tests

**File**: `rust-server/src/tools/search.rs` (in `#[cfg(test)] mod tests`)

1. `test_login_search_only()`: Test search with login only (no account)
2. `test_login_search_with_account()`: Test search with both login and account
3. `test_login_not_found()`: Test error when login not in credentials
4. `test_login_normalization()`: Test handle normalization for login
5. `test_result_deduplication()`: Test deduplication of CAR + API results
6. `test_stats_merging()`: Test AppView stats merging from API
7. `test_thread_formatting()`: Test thread reply formatting
8. `test_compact_refkey()`: Test super-compact refkey syntax

### Go Tests

**File**: `go-server/internal/tools/search_test.go`

1. `TestLoginSearchOnly()`: Test search with login only (no account)
2. `TestLoginSearchWithAccount()`: Test search with both login and account
3. `TestLoginNotFound()`: Test error when login not in credentials
4. `TestLoginNormalization()`: Test handle normalization for login
5. `TestResultDeduplication()`: Test deduplication of CAR + API results
6. `TestStatsMerging()`: Test AppView stats merging from API
7. `TestThreadFormatting()`: Test thread reply formatting
8. `TestCompactRefkey()`: Test super-compact refkey syntax

## Error Messages

- **Login not found**: `"Login '{login}' not found. Please login first using the login tool."`
- **Neither account nor login provided**: `"Either 'account' or 'login' parameter must be provided"`
- **Authentication failed**: `"Failed to authenticate with login '{login}': {error}"`
- **API search failed**: `"Authenticated search failed: {error}"`

## Migration Notes

- This is a backward-compatible change
- Existing `account` + `query` searches continue to work unchanged
- The `account` parameter becomes optional only when `login` is provided
- No changes to output format for existing searches
- No changes to CAR-based search logic

## Implementation Checklist

- [ ] Create this plan document
- [ ] Update Rust SearchArgs to add login parameter and make account optional
- [ ] Implement Rust authenticated API search
- [ ] Implement Rust result merging and deduplication
- [ ] Add Rust test coverage (8 tests minimum)
- [ ] Update Go search input schema for login parameter
- [ ] Implement Go authenticated API search
- [ ] Implement Go result merging and deduplication
- [ ] Add Go test coverage (8 tests minimum)
- [ ] Verify Markdown output format matches /docs/16-mcp-schemas.md
- [ ] Test thread formatting with compact refkey syntax
- [ ] Document any edge cases or limitations
