# Changelog

## [Unreleased]

### Fixed
- **Search Results Display**: Fixed bug where posts showed `@handle/unknown` instead of actual rkey.

## [0.3.5] - 2025-11-01

### Added
- **Thread Tool**: New tool for fetching BlueSky conversation threads with recursive reply structure.
  - Accepts at:// URIs, bsky.app URLs, or compact `@handle/rkey` format.
  - Automatic handle resolution to DIDs for URL/compact format inputs.
  - Formatted markdown output with threading indicators and post counts.

### Changed
- **Markdown Output Format**: Updated search and thread tools to match compact formatting specification.

## [0.3.2] - 2025-10-05

### Added
- **MCP Elicitation Support**: Implemented bidirectional JSON-RPC communication for interactive prompts.
  - Server-to-client RPC for collecting user input during tool execution.
  - Client capability detection for elicitation support.
  - Interactive prompts for missing credentials in login tool.
- **Experimental SentencePiece Integration**: Added optional tokenization support (frozen/experimental).
  - SentencePiece model processing for semantic search research.
  - Note: Experimental feature, not production-ready.

### Changed
- **Code Formatting**: Applied `go fmt` across entire codebase for consistency.
- **Login Tool**: Enhanced with elicitation support for interactive credential collection.
  - Automatically prompts for handle and password when MCP client supports elicitation.
  - Falls back to error messages with instructions for non-supporting clients.

### Fixed
- **Testing**: Fixed authentication-related tests.
- Code quality improvements and standardization.

### Development
- Added end-to-end MCP testing infrastructure.
- Improved Go server structure and organization.

## [0.3.0] - 2025-10-03

### Added
- **Authentication**: Comprehensive authentication support.
  - OAuth 2.0 with PKCE and DPoP (most secure).
  - Device Authorization Grant for headless environments.
  - App password authentication.
  - Secure credential storage using OS keychain with encrypted fallback.
- **Multi-Account Support**: Manage multiple BlueSky accounts with default selection.
- **Account Management Tools**:
  - `login` - Authenticate with app password.
  - `oauth-login` - OAuth browser flow.
  - `device-login` - Device authorization for headless.
  - `accounts` - List and manage authenticated accounts.
  - `logout` - Remove credentials.

### Changed
- Improved credential storage with OS keychain integration.
- Enhanced error handling and user feedback.

## [0.2.0] - 2024-09-30

### Added
- Two-tier caching system with DID-based directory structure.
- Streaming CAR file processing for memory efficiency.
- Unicode normalization for text search.
- Profile and search tools.

### Changed
- Refactored cache management for better performance.
- Improved CAR file parsing.

## [0.1.0] - Initial Release

### Added
- MCP Server implementation with stdio communication.
- CLI mode for direct tool execution.
- BlueSky profile and search functionality.
- Basic caching system.
