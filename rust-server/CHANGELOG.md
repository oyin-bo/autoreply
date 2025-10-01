# Changelog

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