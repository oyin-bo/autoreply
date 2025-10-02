#!/bin/bash
set -e

echo "=== OAuth Authentication Simulation Test ==="
echo ""

# Start OAuth in background
timeout 30 ./autoreply oauth-login --handle autoreply.ooo 2>&1 &
OAUTH_PID=$!

# Wait for server to start
echo "Waiting for callback server to start..."
sleep 3

# Check if server is listening
if netstat -tlnp 2>/dev/null | grep -q ":8080" || ss -tlnp 2>/dev/null | grep -q ":8080"; then
    echo "✓ Callback server is listening on port 8080"
else
    echo "✗ Callback server is NOT listening on port 8080"
    kill $OAUTH_PID 2>/dev/null || true
    exit 1
fi

# Test the callback endpoint with a simulated OAuth response
echo ""
echo "Simulating OAuth callback..."
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" "http://127.0.0.1:8080/callback?code=test_code&state=test_state" 2>&1)

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)

if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ Callback endpoint responded with HTTP 200"
    echo "✓ Server successfully handled the callback"
else
    echo "✗ Callback endpoint responded with HTTP $HTTP_CODE"
fi

# Clean up
kill $OAUTH_PID 2>/dev/null || true
wait $OAUTH_PID 2>/dev/null || true

echo ""
echo "=== Test Complete ==="
