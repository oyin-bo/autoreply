# autoreply MCP Server (Go)

An autoreply Model Context Protocol (MCP) server for BlueSky profile and post search functionality, implemented in Go.

## Features

- **Dual-Mode Operation**: MCP server mode and CLI trial mode
- **Complete Authentication**: Three authentication methods:
  - App password (simple)
  - OAuth 2.0 with PKCE and DPoP (most secure)
  - Device Authorization Grant (headless)
- **Secure Credential Storage**: OS keychain integration with encrypted fallback
- **Profile Tool**: Retrieve user profile information from BlueSky
- **Search Tool**: Search posts within a user's repository
- **Account Management**: Multi-account support with login/logout/accounts commands
- **Two-tier Caching**: Efficient caching with DID-based directory structure
- **Unicode Support**: Proper Unicode normalization for text search
- **Streaming Downloads**: Memory-efficient CAR file processing

## Installation

```bash
# Clone the repository
git clone https://github.com/oyin-bo/autoreply.git
cd autoreply/go-server

# Build the binary
go build -o autoreply ./cmd/autoreply

# Run as MCP server (default)
./autoreply

# Run as CLI tool
./autoreply --help
```

## Usage

The binary operates in two modes:

### 1. MCP Server Mode (Default)

Run without arguments to start an MCP server that implements the JSON-RPC 2.0 protocol over stdio:

```bash
./autoreply
```

### 2. CLI Mode (Trial/Testing)

Run with commands for direct tool execution:

```bash
# Authentication - App Password (Simple)
./autoreply login --handle alice.bsky.social --password xxxx-xxxx-xxxx-xxxx

# Authentication - OAuth (Most Secure)
./autoreply oauth-login --port 8080

# Authentication - Device (Headless)
./autoreply device-login

# Account Management
./autoreply accounts
./autoreply logout

# Get profile information
./autoreply profile --account alice.bsky.social
./autoreply profile -a alice.bsky.social

# Search posts
./autoreply search --account bob.bsky.social --query "rust programming" --limit 10
./autoreply search -a bob.bsky.social -q "rust" -l 10

# Get help
./autoreply --help
./autoreply profile --help
```

**See [CLI_MODE.md](./CLI_MODE.md) for detailed CLI usage documentation.**
**See [docs/AUTHENTICATION_EXAMPLES.md](./docs/AUTHENTICATION_EXAMPLES.md) for authentication examples.**

### Available Tools

#### login

Authenticate with BlueSky using handle and app password.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "login",
    "arguments": {
      "handle": "alice.bsky.social",
      "password": "xxxx-xxxx-xxxx-xxxx"
    }
  }
}
```

#### accounts

List authenticated accounts and manage default account.

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "accounts",
    "arguments": {
      "action": "list"
    }
  }
}
```

#### logout

Remove stored credentials for a BlueSky account.

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "logout",
    "arguments": {
      "handle": "alice.bsky.social"
    }
  }
}
```

#### profile

Retrieve user profile information.

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "profile",
    "arguments": {
      "account": "alice.bsky.social"
    }
  }
}
```

#### search

Search posts within a user's repository.

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": {
      "account": "alice.bsky.social",
      "query": "machine learning"
    }
  }
}
```

## Configuration

Configure via environment variables:

- `CACHE_TTL_HOURS`: Repository cache TTL in hours (default: 24)
- `PROFILE_TTL_HOURS`: Profile cache TTL in hours (default: 1)
- `REQUEST_TIMEOUT`: HTTP request timeout (default: 10s)
- `DOWNLOAD_TIMEOUT`: CAR download timeout (default: 60s)
- `MAX_QUERY_LENGTH`: Maximum search query length (default: 500)
- `MAX_CONCURRENT_DOWNLOADS`: Max concurrent downloads (default: 4)

## Architecture

### Directory Structure

```
cmd/autoreply/     # Main application entry point
internal/
├── auth/            # Authentication and credential management
├── cli/             # CLI mode implementation (args, registry, runner)
├── mcp/             # MCP protocol implementation
├── bluesky/         # BlueSky API and CAR processing
├── cache/           # Cache management with two-tier structure
├── tools/           # Tool implementations (profile, search, login, etc.)
└── config/          # Configuration management
pkg/errors/          # Error types and utilities
```

### Cache Structure

```
{cache_dir}/
├── {2-letter-prefix-of-did-hash}/
│   └── {full-did}/
│       ├── repo.car
│       └── metadata.json
```

## Dependencies

- `github.com/ipld/go-car/v2` - CAR file parsing
- `github.com/spf13/cobra` - CLI framework
- `github.com/invopop/jsonschema` - JSON Schema generation
- `github.com/99designs/keyring` - Secure credential storage
- `golang.org/x/text` - Unicode normalization
- `golang.org/x/sync` - Concurrency utilities

## Development

```bash
# Install dependencies
go mod tidy

# Run tests
go test ./...

# Build for development
go run ./cmd/autoreply
```

## License

See LICENSE file.
