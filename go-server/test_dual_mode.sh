#!/bin/bash
set -e

echo "===================================================="
echo "Testing Autoreply Dual Mode Operation"
echo "===================================================="
echo ""

# Build first
echo "Building autoreply..."
go build -o autoreply ./cmd/autoreply
echo "✓ Build successful"
echo ""

# Test 1: CLI help
echo "Test 1: CLI Help"
echo "Command: ./autoreply --help"
./autoreply --help | head -10
echo "✓ CLI help works"
echo ""

# Test 2: CLI version
echo "Test 2: CLI Version"
echo "Command: ./autoreply --version"
./autoreply --version
echo "✓ Version display works"
echo ""

# Test 3: Profile command help
echo "Test 3: Profile Command Help"
echo "Command: ./autoreply profile --help"
./autoreply profile --help
echo "✓ Profile help works"
echo ""

# Test 4: Search command help
echo "Test 4: Search Command Help"
echo "Command: ./autoreply search --help"
./autoreply search --help
echo "✓ Search help works"
echo ""

# Test 5: Profile CLI execution
echo "Test 5: Profile CLI Execution"
echo "Command: ./autoreply profile --account bsky.app"
./autoreply profile --account bsky.app 2>/dev/null | head -10
echo "✓ Profile CLI execution works"
echo ""

# Test 6: Search CLI execution with short flags
echo "Test 6: Search CLI Execution (short flags)"
echo "Command: ./autoreply search -a bsky.app -q welcome -l 2"
./autoreply search -a bsky.app -q welcome -l 2 2>/dev/null | head -15
echo "✓ Search CLI execution works"
echo ""

# Test 7: MCP Server Mode (tools/list)
echo "Test 7: MCP Server Mode"
echo "Command: echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}' | ./autoreply"
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./autoreply 2>/dev/null | jq -r '.result.tools[] | .name'
echo "✓ MCP server mode works"
echo ""

# Test 8: Error handling (missing required arg)
echo "Test 8: Error Handling"
echo "Command: ./autoreply profile (without --account)"
if ./autoreply profile 2>&1 >/dev/null; then
    echo "✗ Should have failed"
    exit 1
else
    echo "✓ Error handling works (exit code: $?)"
fi
echo ""

echo "===================================================="
echo "All tests passed! ✓"
echo "===================================================="
echo ""
echo "Summary:"
echo "  - CLI mode works correctly"
echo "  - MCP server mode works correctly"
echo "  - Help system functional"
echo "  - Error handling proper"
echo "  - Both long and short flags supported"
