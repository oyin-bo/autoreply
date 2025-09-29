# Go BlueSky MCP Server

A Model Context Protocol (MCP) server implementation for Bluesky profile and post search functionality, written in Go.

## Overview

This is a POC implementation of the BlueSky MCP server following the specifications in `docs/7.2-go.md`. The server implements two MCP tools:
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
- Cache management with TTL
- Atomic file operations

## Building

```bash
go build -o bin/bluesky-mcp ./cmd/bluesky-mcp
```

## Usage

The server communicates via stdio using the MCP protocol:

```bash
./bin/bluesky-mcp
```

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

## Architecture

The implementation follows the specifications in `docs/7.2-go.md`:

- **Two-tier cache structure**: `{cache_dir}/{2-letter-prefix}/{full-did}/`
- **Timeout configuration**: 10s DID resolution, 60s CAR download, 120s total
- **Error codes**: `invalid_input`, `did_resolve_failed`, `repo_fetch_failed`, etc.
- **Platform-specific cache locations**: Uses `os.UserCacheDir()` with `bluesky-mcp` subdirectory

## Testing

Run the test script to verify functionality:

```bash
./test_go_mcp.sh
```

## Implementation Status

This is a POC implementation of the Go BlueSky MCP Server specification with working:
- ✅ MCP protocol handling
- ✅ DID resolution 
- ✅ Cache management
- ✅ Profile tool
- ✅ Search tool
- ✅ Error handling
- ⚠️ CAR parsing (simplified for POC - uses basic iteration through CAR blocks)

## Dependencies

Key Go modules used:
- `github.com/ipld/go-car/v2` - CAR file parsing
- `github.com/fxamacker/cbor/v2` - CBOR encoding/decoding
- `golang.org/x/text/unicode/norm` - Unicode normalization
- `golang.org/x/sync` - Concurrency utilities

## File Structure

```
cmd/
└── bluesky-mcp/
    └── main.go          // Application entry point

internal/
├── mcp/
│   ├── server.go        // MCP protocol server
│   └── types.go         // MCP protocol types
├── bluesky/
│   ├── did.go          // DID resolution
│   ├── car.go          // CAR file operations  
│   ├── records.go      // AT Protocol record types
│   └── client.go       // HTTP client utilities
├── cache/
│   └── manager.go      // Cache management
├── tools/
│   ├── profile.go      // Profile tool implementation
│   ├── search.go       // Search tool implementation
│   └── common.go       // Shared tool utilities
└── config/
    └── config.go       // Configuration management

pkg/
└── errors/
    └── errors.go       // Error types and utilities
```

This follows the exact file structure specified in the Go implementation documentation.