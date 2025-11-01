# Changelog

## [0.3.6] - 2025-11-01

### Added
- Support for various account formats: handles, @handles, DIDs, Bsky.app profile URLs, even shortDIDs.
- MST (Merkle Search Tree) Parsing: finished CID->rkey mapping extraction from CAR files, was missing earlier, added new `bluesky::mst` module.

### Changed
- Consistency and clarity across all account parameter names.
- Removed app password from feed and thread tools: login is where authentication happens.
- Pagination: batching is internal and allows any number of posts per page.
- Fixed bug where posts showed `@handle/unknown` instead of actual rkey.

## [0.3.5] - 2025-11-01

### Fixed
- **CAR File Download Timeouts**: Increased HTTP client timeouts from 30s to 120s across all modules to handle large (30-70MB) repository downloads.
  - Better error messages showing byte counts when connections are interrupted.
- **OAuth Callback Server**: Fixed loopback redirect URI to use root path `/` instead of `/callback` per RFC 8252 specification.

### Changed
- **Markdown Output Format**: Implemented compact formatting per `docs/16-mcp-schemas.md` specification.
- **Logging**: Changed tool execution logging from `info!` to `debug!` for cleaner CLI output.

### Added
- **Feed Name Search**: Feed tool now accepts feed names (e.g., "What's Hot") in addition to at:// URIs, with automatic resolution via popular feeds API.
- **Compact Post URIs**: Thread tool accepts `@handle/rkey` format in addition to full at:// URIs and bsky.app URLs.
- **OAuth Account Selection**: Login flow now supports handle-free OAuth, allowing users to select any account during authorization (passes `login_hint` when handle is provided).

## [0.3.4] - 2025-10-30

### Added
- **Feed Tool**: New `feed` tool for fetching BlueSky feeds.
  - Supports default "What's Hot" feed and custom feed URIs.
  - Pagination support with cursor parameter.
  - Limit parameter (default 20, max 100 posts per request).
  - Returns formatted markdown with post content, author info, and engagement stats.
- **Thread Tool**: New `thread` tool for fetching complete conversation threads.
  - Takes post URI (at:// format) as input.
  - Recursively fetches all replies and nested conversations.
  - Returns formatted markdown with indented reply structure.
  - Handles blocked and not-found posts gracefully.
- **Post Tool**: New `post` tool for creating posts and replies on BlueSky.
  - Create standalone posts with text content.
  - Reply to existing posts with proper threading (preserves reply chain root).
  - Automatic reply-to post fetching for CID and reply metadata.
  - Returns post URI and formatted markdown confirmation.
- **React Tool**: New `react` tool for interacting with BlueSky posts.
  - Batch operations support: like, unlike, repost, and delete in a single call.
  - Array-based parameters for each action type (can perform multiple reactions at once).
  - Post URI resolution (supports both `at://` URIs and `bsky.app` URLs).
  - Validation to prevent deleting posts that don't belong to the authenticated user.
  - Returns detailed markdown report of successful and failed operations.

## [0.3.2] - 2025-10-05

### Added
- **MCP Elicitation Support**: Implemented bidirectional JSON-RPC communication for interactive prompts.
  - `elicitation/create` requests for collecting user input (handles, passwords) during MCP tool execution.
  - Server-to-client RPC sender with async response handling and request ID management.
  - Client capability detection for elicitation support during initialization.
  - Comprehensive test suite (15+ new tests) covering request ID generation, concurrent operations, and error handling.
- **Experimental SentencePiece Integration**: Added optional tokenization support (behind `experimental-sentencepiece` feature flag).
  - Protobuf-based SentencePiece model loading and processing.
  - Embedding table loader for quantized embeddings (EMB8 format).
  - Note: Experimental feature, not production-ready.

### Changed
- **Login Tool Enhancement**: Interactive elicitation for missing credentials when MCP client supports it.
  - Prompts for BlueSky handle if not provided.
  - Prompts for app password with clear guidance and OAuth alternative suggestion.
  - Falls back to detailed error messages with instructions for clients without elicitation support.
- **PDS Discovery**: Implemented complete PDS (Personal Data Server) resolution for profiles.
  - Full `did:plc` support via plc.directory DID document resolution.
  - Full `did:web` support with .well-known/did.json and did.json fallback paths.
  - Proper `AtprotoPersonalDataServer` service endpoint extraction from DID documents.
  - Robust error handling for malformed or unavailable DID documents.
- **DID Resolution**: Enhanced edge case handling and code quality improvements.
- **Error Messages**: Enhanced user-facing error messages with clearer guidance and formatting.

### Fixed
- Code quality improvements: removed duplicated cfg attributes, unused imports, and compiler warnings.
- Formatting consistency across all Rust source files (`cargo fmt`).
- Test compilation errors in experimental features.
- All clippy warnings resolved for default feature set (clean build with `cargo clippy`).

### Development
- Added end-to-end MCP testing infrastructure with Gemini CLI integration.
- Zero-allocation optimizations in normalizer implementation.
- Improved Go server formatting and structure (parallel development track).

## [0.3.1] - 2025-10-03

### Fixed
- Documentation: Removed or clarified references to the unimplemented OAuth Device Flow and synchronized CLI docs to reflect `login` subcommands (`list`, `default`, `delete`).
- README: Corrected CLI command examples and descriptions for consistency with the implemented CLI.

## [0.3.0] - 2025-10-03

### Added
- **Authentication**: Implemented comprehensive authentication support.
  - OAuth Browser Flow with PKCE and automatic browser redirect.
  - App password authentication via AT Protocol.
  - Secure credential storage using OS keyring with file-based fallback.
  - Support for multiple accounts with default account selection.
  - Automatic token refresh and lifecycle management.
- **CLI**: Added new commands for authentication:
  - `autoreply login`: Authenticate using OAuth (default) or app password.
  - `autoreply login list`: List all authenticated accounts.
  - `autoreply login default <handle>`: Set the default account for commands.
  - `autoreply login delete`: Remove stored credentials.

### Changed
- Updated dependencies to support authentication features (atproto-oauth, keyring, etc.).
- OAuth browser flow is now the default authentication method (falls back to app password on failure).

## [0.2.0] - 2024-09-30

### Added
- Streaming iterator-based CAR file processing for memory efficiency
- Comprehensive test suite with 101+ tests covering all major modules
- Fast CAR reader implementation with proper AT Protocol record filtering
- Error handling for invalid CAR files and edge cases

### Changed
- Refactored from callback-based to iterator-based streaming architecture
- Improved performance: ~2s CAR file processing (from 2+ minute timeouts)
- Enhanced BlueSky/AT Protocol field preservation for future MCP tools
- Better separation of concerns between CAR parsing and record processing

### Removed
- Unused MST (Merkle Search Tree) module completely removed
- Callback-based streaming implementation
- Unused parsing position tracking fields

### Fixed
- Memory efficiency issues with large CAR files
- Compiler warnings cleanup with selective field preservation
- Test compatibility with DID resolver API changes

## [0.1.0] - Initial Release

### Added
- MCP Server implementation with stdio communication
- CLI mode for direct tool execution
- Bluesky profile and search tools
- DID resolution and CAR file caching
- System proxy support
- Unicode text normalization
- Input validation and error handling