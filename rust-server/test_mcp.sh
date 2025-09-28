#!/bin/bash
# Test script for the Bluesky MCP Server

cd "$(dirname "$0")"

echo "Testing MCP Server..."

# Test tools/list request
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | ./target/debug/bluesky-mcp-server

echo ""
echo "---"
echo ""

# Test profile tool with mock data (since we have a mock implementation)
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "profile", "arguments": {"account": "test.bsky.social"}}}' | ./target/debug/bluesky-mcp-server

echo ""
echo "---"  
echo ""

# Test search tool with mock data
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "search", "arguments": {"account": "test.bsky.social", "query": "hello"}}}' | ./target/debug/bluesky-mcp-server