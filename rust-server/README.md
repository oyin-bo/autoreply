# autoreply MCP Server & CLI (Rust)

A dual-mode application for Bluesky profile and post search functionality, written in Rust.

## Overview

This application supports two operational modes:

1. **MCP Server Mode** (default): Model Context Protocol server using stdio
2. **CLI Mode**: Command-line utility for direct tool execution

Both modes implement the same two tools:
- `profile(account)` - Retrieve user profile information  
- `search(account, query)` - Search posts within a user's repository

## Features

✅ **Complete MCP Protocol Implementation**
- JSON-RPC 2.0 compliant
- Stdio communication
- Proper error handling with MCP error codes

✅ **Bluesky Integration**
- DID resolution (handle → DID)
- CAR file caching with atomic operations
- Fast streaming CAR file parsing with iterator-based processing
- AT Protocol record parsing and filtering
- Profile and post extraction

✅ **Advanced Functionality**
- Streaming CAR reader (20-80x times faster than atrium-repo)
- Text search with highlighting
- Unicode normalization (NFKC)
- Comprehensive input validation  
- Atomic file operations with locking
- System proxy support via environment variables (HTTP(S)_PROXY, NO_PROXY)

✅ **Quality Assurance**
- Comprehensive test suite with 101+ tests
- Full error handling and edge case coverage
- Performance optimized for ~2s repo processing

## Building

```bash
cd rust-server
cargo build --release
```

## Usage

### MCP Server Mode (Default)

The server communicates via stdio using the MCP protocol:

```bash
./target/release/autoreply
```

### CLI Mode

When invoked with arguments, the binary operates as a command-line utility:

```bash
# Get profile information
autoreply profile --account alice.bsky.social

# Search posts
autoreply search --account bob.bsky.social --query "rust programming" --limit 10

# Get help
autoreply --help
autoreply profile --help
autoreply search --help
```

For complete CLI usage documentation, see [CLI-USAGE.md](./CLI-USAGE.md).

### Proxy support

This server honors system proxy environment variables via reqwest’s system proxy detection:

- HTTP_PROXY / http_proxy
- HTTPS_PROXY / https_proxy
- ALL_PROXY / all_proxy
- NO_PROXY / no_proxy

Examples:

```bash
# HTTPS over an HTTP proxy (CONNECT):
export HTTPS_PROXY=http://application-proxy.blackrock.com:9443

# Exclude local addresses or specific hosts:
export NO_PROXY=localhost,127.0.0.1,::1

# Run the server
./target/release/autoreply
```

Notes:

- Credentials (if required) may be provided in the proxy URL, e.g. http://user:pass@proxy.example.com:8080
- TLS uses the OS trust store (native TLS). If your proxy performs TLS interception, ensure your corporate root CA is installed in the OS trust store.

### Example MCP Requests

**List available tools:**
```json
{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}
```

**Get user profile:**
```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "profile", "arguments": {"account": "alice.bsky.social"}}}
```

**Search user's posts:**
```json
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "search", "arguments": {"account": "alice.bsky.social", "query": "hello world"}}}