# End-to-End Testing with Gemini CLI and a Unified Test Harness

This document outlines the streamlined testing strategy for verifying the MCP functionality of both the Go and Rust `autoreply` servers. It uses Gemini CLI as the execution engine and a single, unified Node.js script to orchestrate tests and verify outcomes.

## Table of Contents

- [Overview](#overview)
- [Test Architecture](#test-architecture)
- [Setup Requirements](#setup-requirements)
- [MCP Server Configuration](#mcp-server-configuration)
- [Test Implementation](#test-implementation)
- [Integration with CI/CD](#integration-with-cicd)
- [Troubleshooting](#troubleshooting)

## Overview

We use a unified Node.js test harness (`mcp-test.js`) to drive Gemini CLI for end-to-end verification of our MCP servers. This approach ensures consistency and maintainability.

The testing strategy relies on:
- **A Single Test Runner**: `mcp-test.js` is the single source of truth for test logic.
- **Static Configuration**: Each server directory (`go-server/`, `rust-server/`) contains a checked-in `.gemini/settings.json` file.
- **Directory-Based Execution**: The test harness is run from within the directory of the server being tested, allowing Gemini CLI to automatically discover the correct MCP server.
- **Embedded Test Cases**: Prompts and assertions are defined directly within the `mcp-test.js` script for simplicity.

## Test Architecture

The test architecture is simple and robust:

```
┌─────────────────────────────────────────┐
│   Node.js Test Harness (mcp-test.js)    │
│  - Contains embedded test cases         │
│  - Executes Gemini CLI for each prompt  │
│  - Matches regex assertions against     │
│    the full JSON output                 │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│         Gemini CLI (MCP Client)         │
│  - Discovers MCP server via local       │
│    `.gemini/settings.json`              │
│  - Executes prompts in headless mode    │
└─────────────────────────────────────────┘
                  ↓ MCP Protocol (stdio)
┌─────────────────────────────────────────┐
│    autoreply MCP Server (Go/Rust)       │
│  - Responds to tool discovery & calls   │
└─────────────────────────────────────────┘
```

## Setup Requirements

### Prerequisites

1.  **Node.js 20+**: Required for the test harness.
2.  **Gemini CLI Installed**: `npm install -g @google/gemini-cli`.
3.  **Gemini CLI Authenticated**: The user running the tests must have already authenticated the CLI (e.g., by running `gemini` once).
4.  **Server Binaries Built**:
    *   Go: `cd go-server && go build -o autoreply ./cmd/autoreply`
    *   Rust: `cd rust-server && cargo build --release`
5.  **Test Credentials (Optional)**: For tests involving authentication, set `BSKY_TEST_HANDLE` and `BSKY_TEST_PASSWORD` as environment variables.

## MCP Server Configuration

### Static Settings Files

Each server directory contains a static, version-controlled configuration file that tells Gemini CLI how to run it. This makes the setup for each server explicit and reproducible.

#### Go Server Configuration

**File**: `go-server/.gemini/settings.json`

```json
{
  "mcpServers": {
    "autoreply-go": {
      "type": "stdio",
      "command": "./autoreply",
      "args": [],
      "cwd": ".",
      "timeout": 30000,
      "trust": true
    }
  },
  "allowMCPServers": ["autoreply-go"]
}
```

#### Rust Server Configuration

**File**: `rust-server/.gemini/settings.json`

```json
{
  "mcpServers": {
    "autoreply-rust": {
      "type": "stdio",
      "command": "./target/release/autoreply",
      "args": [],
      "cwd": ".",
      "timeout": 30000,
      "trust": true
    }
  },
  "allowMCPServers": ["autoreply-rust"]
}
```

**Note**: The `"trust": true` setting is crucial for automated testing as it bypasses interactive prompts.

## Test Implementation

### Unified Test Harness

A single Node.js script, `mcp-test.js`, located in the project root, serves as the universal test runner.

### Test Case Format

Test cases are defined inside a multi-line string within `mcp-test.js`. The format is simple and readable:

-   Lines starting with `>` define a prompt to be sent to Gemini CLI.
-   Lines starting with `<` define a regular expression to be asserted against the entire JSON output from the CLI.
-   Lines starting with `#` are comments.

**Example from `mcp-test.js`:**

```javascript
# Test Case: Successful Profile Lookup
> Use the autoreply profile tool to get information about the BlueSky account "bsky.app"
< /"totalSuccess":\s*[1-9]/
< /bsky\.app/i
```

### Running the Tests

The test harness is executed from the directory of the server you wish to test. This ensures Gemini CLI picks up the correct local configuration.

**To test the Go server:**

```bash
cd go-server
node ../mcp-test.js
```

**To test the Rust server:**

```bash
cd rust-server
node ../mcp-test.js
```

The script will execute all defined test cases against the selected server and provide a summary of passed and failed tests.

## Integration with CI/CD

### GitHub Actions Workflow

The unified test harness simplifies the CI workflow.

**File**: `.github/workflows/e2e-tests.yml`

```yaml
name: End-to-End Tests

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        server: [go-server, rust-server]
    steps:
      - uses: actions/checkout@v4

      - name: Setup Go
        if: matrix.server == 'go-server'
        uses: actions/setup-go@v4
        with:
          go-version: '1.22'

      - name: Setup Rust
        if: matrix.server == 'rust-server'
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install Gemini CLI
        run: npm install -g @google/gemini-cli

      - name: Authenticate Gemini CLI
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
        run: gemini -p "hello" --output-format json # A quick command to ensure auth works

      - name: Build Server
        working-directory: ${{ matrix.server }}
        run: |
          if [ "${{ matrix.server }}" = "go-server" ]; then
            go build -o autoreply ./cmd/autoreply
          else
            cargo build --release
          fi

      - name: Run E2E Tests
        working-directory: ${{ matrix.server }}
        env:
          BSKY_TEST_HANDLE: ${{ secrets.BSKY_TEST_HANDLE }}
          BSKY_TEST_PASSWORD: ${{ secrets.BSKY_TEST_PASSWORD }}
        run: node ../mcp-test.js
```

**Required GitHub Secrets**:
*   `GEMINI_API_KEY`: For authenticating Gemini CLI.
*   `BSKY_TEST_HANDLE`: For running authentication-related tests.
*   `BSKY_TEST_PASSWORD`: For running authentication-related tests.

### Makefile Integration

You can add convenience targets to each server's `Makefile`.

**Example for `go-server/Makefile`**:

```makefile
.PHONY: test-e2e

test-e2e:
	go build -o autoreply ./cmd/autoreply
	node ../mcp-test.js
```

## Troubleshooting

### Common Issues

#### 1. Gemini CLI Not Found

**Symptom**: `command not found: gemini` or `node: command not found`
**Solution**: Ensure Node.js is installed and run `npm install -g @google/gemini-cli`.

#### 2. MCP Server Not Discovered

**Symptom**: Prompts do not trigger any tool calls (`"totalCalls": 0`).
**Solution**:
1.  Ensure you are running `node ../mcp-test.js` from within the correct server directory (`go-server/` or `rust-server/`).
2.  Verify that `.gemini/settings.json` exists in that directory and is valid.
3.  Confirm the server binary has been built and is executable at the path specified in `settings.json`.

#### 3. Authentication Issues

**Symptom**: `Authentication required` errors from Gemini CLI.
**Solution**: Run `gemini` interactively once to complete the authentication flow, or ensure the `GEMINI_API_KEY` secret is correctly configured in your CI environment.

#### 4. Test Timeouts

**Symptom**: Tests fail with a timeout error.
**Solution**:
1.  Check your internet connection.
2.  Increase the timeout in the relevant `settings.json` file (e.g., `"timeout": 60000`).
3.  Run with `gemini --debug` to see if the server is hanging.