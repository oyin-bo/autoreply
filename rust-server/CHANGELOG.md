# Changelog

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