# Changelog

## [Unreleased]

### Added
- **OAuth Authentication**: OAuth 2.0 authentication flows
  - OAuth Device Authorization Grant (fully implemented) - secure OAuth for headless/CLI environments
  - OAuth Browser Flow (infrastructure in place) - interactive OAuth with browser redirect
  - CLI flags: `--device` for device flow, `--oauth` for browser flow
  - Token polling and exchange for device flow
  - Support for multiple authentication methods in single CLI
- **Authentication System**: Complete app password authentication implementation
  - App password authentication via AT Protocol `com.atproto.server.createSession`
  - Secure credential storage with OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service)
  - File-based fallback storage with restricted permissions (0600)
  - Token refresh and lifecycle management (2h access token, 90d refresh token)
  - Multi-account support with default account selection
- **CLI Commands**: Enhanced authentication commands
  - `autoreply login --device` - OAuth device flow for headless environments
  - `autoreply login --oauth` - OAuth browser flow (placeholder)
  - `autoreply login` - App password authentication (default)
  - `autoreply logout` - Remove stored credentials
  - `autoreply accounts list` - List all authenticated accounts
  - `autoreply accounts default` - Set default account
- **Documentation**: Comprehensive documentation for authentication
  - `src/auth/README.md` - Authentication module documentation with OAuth examples
  - `CLI-USAGE.md` - Complete CLI usage guide with OAuth flows
  - `demo_auth.sh` - Interactive demonstration script
  - `docs/OAuth-Implementation-Plan.md` - OAuth implementation roadmap
- **Tests**: 12 new unit tests for authentication components (112 total)
  - Credential serialization and storage tests
  - Session expiry and lifecycle tests
  - Multi-account management tests
  - Storage backend tests
  - OAuth configuration and manager tests

### Changed
- Updated `README.md` to document OAuth authentication features
- Enhanced error handling with new error types: `Authentication`, `ConfigError`, `ParseError`
- Dependencies: Added `keyring`, `chrono`, `base64`, `atproto-oauth`, `atproto-client`, `atproto-identity`, `axum`, `webbrowser`, `rand` crates
- Login command now supports three authentication methods with conflict detection

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