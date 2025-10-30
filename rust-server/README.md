# autoreply MCP Server & CLI (Rust)

A dual-mode application for Bluesky profile and post search functionality, written in Rust.

## Overview

This application supports two operational modes:

1. **MCP Server Mode** (default): Model Context Protocol server using stdio
2. **CLI Mode**: Command-line utility for direct tool execution

Both modes implement the same tools:
- `profile(account)` - Retrieve user profile information  
- `search(account, query)` - Search posts within a user's repository
- `feed` - Get the latest feed from BlueSky (Discovery feed or curated feeds)
- `thread(postURI)` - Fetch a complete thread with all replies
- `login(...)` - Authenticate accounts and manage stored credentials (OAuth + app password)
  - Supports **interactive elicitation** for missing credentials when used via MCP clients that support the elicitation capability
  - Falls back to clear error messages with instructions for non-supporting clients

Authentication support via app passwords allows storing and managing credentials for future authenticated operations.

## Features

✅ **Complete MCP Protocol Implementation**
- JSON-RPC 2.0 compliant
- Stdio communication
- Proper error handling with MCP error codes
- **Bidirectional RPC support** for server-to-client requests (elicitation)
- Client capability detection and negotiation

✅ **Authentication & Credential Management**
- OAuth Browser Flow - Interactive OAuth with PKCE and callback server
- App password authentication via AT Protocol
- Secure credential storage (OS keyring with file fallback)
- Multi-account support with default selection
- Token refresh and lifecycle management
- CLI commands: `login` (with subcommands: `list`, `default <handle>`, `delete`)
- MCP `login` tool with interactive **elicitation** for handles and app passwords
  - Automatically prompts for missing credentials when MCP client supports elicitation
  - Provides clear instructions when elicitation is not available
- Defaults to OAuth and allows for app passwords

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
- Comprehensive test suite with 124+ tests (including 15+ tests for elicitation)
- Full error handling and edge case coverage
- Performance optimized for ~2s repo processing
- Clean builds with zero clippy warnings (default features)

🧪 **Experimental Features** (opt-in via feature flags)
- SentencePiece tokenization support (`experimental-sentencepiece`)
- Quantized embedding table support for semantic search prototypes
- Not production-ready - for research and experimentation only

## Building

```bash
cd rust-server
cargo build --release
```

### Building with Experimental Features

```bash
# Build with SentencePiece support (requires protobuf compiler)
cargo build --release --features experimental-sentencepiece
```

**Note:** Experimental features require additional dependencies and are not recommended for production use.

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

# Authentication commands

# OAuth browser flow (default - recommended!)
autoreply login --handle alice.bsky.social

# App password authentication
autoreply login --handle alice.bsky.social --password app-password-here

# Account management
autoreply login list
autoreply login default alice.bsky.social
autoreply login delete --handle alice.bsky.social

# Get help
autoreply --help
autoreply profile --help
autoreply search --help
autoreply login --help
```

For complete CLI usage documentation, see [CLI-USAGE.md](./CLI-USAGE.md).

For authentication details and examples, see [src/auth/README.md](./src/auth/README.md).

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
```

**Get feed:**
```json
{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "feed", "arguments": {"feed": "at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot", "limit": 20}}}
```

**Get thread:**
```json
{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "thread", "arguments": {"postURI": "at://did:plc:example/app.bsky.feed.post/123"}}}
```

**Login / manage credentials:**
```json
{"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "login", "arguments": {"handle": "alice.bsky.social"}}}