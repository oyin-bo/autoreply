#!/bin/bash
# Build script for autoreply MCP Server

set -e

echo "Building autoreply MCP Server..."

cd "$(dirname "$0")"

# Build in release mode for optimal performance
cargo build --release

echo ""
echo "✅ Build completed successfully!"
echo ""
echo "Binary location: ./target/release/autoreply"
echo "Size: $(du -h ./target/release/autoreply | cut -f1)"
echo ""
echo "To test the server, run: ./test_mcp.sh"
echo "To use with MCP client, run: ./target/release/autoreply"