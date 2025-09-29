#!/bin/bash

# test_mcp.sh - Test script for Go BlueSky MCP Server

set -e

echo "Testing Go BlueSky MCP Server..."

# Build the server
echo "Building server..."
go build -o bin/bluesky-mcp ./cmd/bluesky-mcp

# Test 1: Initialize
echo ""
echo "Test 1: Initialize"
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}' | ./bin/bluesky-mcp

# Test 2: Tools list
echo ""
echo "Test 2: Tools list"
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}' | ./bin/bluesky-mcp

# Test 3: Profile tool call (with timeout - this will likely fail without real network)
echo ""
echo "Test 3: Profile tool call (expect timeout/error)"
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "profile", "arguments": {"account": "test.bsky.social"}}}' | timeout 5 ./bin/bluesky-mcp || echo "Expected timeout/error occurred"

echo ""
echo "Basic MCP protocol tests completed successfully!"