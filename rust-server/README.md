# autoreply MCP Server (Rust)

A Model Context Protocol (MCP) server implementation for Bluesky profile and post search functionality, written in Rust.

## Overview

This server implements two MCP tools:
- `profile(account)` - Retrieve user profile information  
- `search(account, query)` - Search posts within a user's repository

## Features

✅ **Complete MCP Protocol Implementation**
- JSON-RPC 2.0 compliant
- Stdio communication
- Proper error handling with MCP error codes

✅ **Bluesky Integration**
- DID resolution (handle → DID)
- CAR file caching with two-tier directory structure
- AT Protocol record parsing
- Profile and post extraction

✅ **Advanced Functionality**
- Text search with highlighting
- Unicode normalization (NFKC)
- Comprehensive input validation  
- Atomic file operations with locking
 - System proxy support via environment variables (HTTP(S)_PROXY, NO_PROXY)

## Building

```bash
cd rust-server
cargo build --release
```

## Usage

The server communicates via stdio using the MCP protocol:

```bash
./target/release/autoreply
```

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