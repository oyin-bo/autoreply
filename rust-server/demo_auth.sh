#!/bin/bash
# Example script demonstrating autoreply authentication workflow

set -e  # Exit on error

echo "=== autoreply Authentication Demo ==="
echo

# Check if autoreply is built
if [ ! -f "./target/release/autoreply" ]; then
    echo "Building autoreply..."
    cargo build --release
    echo
fi

AUTOREPLY="./target/release/autoreply"

echo "This script demonstrates the authentication features."
echo "Note: This is a demonstration script - do not use with real credentials"
echo

# Function to print section headers
section() {
    echo
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  $1"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo
}

# 1. Show help
section "1. Viewing Available Commands"
echo "Command: autoreply --help"
echo
$AUTOREPLY --help

# 2. Show login help
section "2. Login Command Help"
echo "Command: autoreply login --help"
echo
$AUTOREPLY login --help

# 3. Show accounts help
section "3. Accounts Management Help"
echo "Command: autoreply accounts --help"
echo
$AUTOREPLY accounts --help

# 4. Try listing accounts (should show none initially)
section "4. Listing Accounts (Initially Empty)"
echo "Command: autoreply accounts list"
echo
$AUTOREPLY accounts list || true

# 5. Show profile command
section "5. Profile Query (No Authentication Required)"
echo "Command: autoreply profile --account jay.bsky.team"
echo "Note: Public profiles can be queried without authentication"
echo
$AUTOREPLY profile --account jay.bsky.team 2>/dev/null | head -20 || echo "(Note: Network required for this command)"
echo "... (truncated)"

# 6. Show search command
section "6. Post Search (No Authentication Required)"
echo "Command: autoreply search --account jay.bsky.team --query bluesky --limit 3"
echo "Note: Public posts can be searched without authentication"
echo
$AUTOREPLY search --account jay.bsky.team --query bluesky --limit 3 2>/dev/null | head -30 || echo "(Note: Network required for this command)"
echo "... (truncated)"

section "Summary"
echo "✓ Authentication commands available:"
echo "  - autoreply login        : Authenticate with BlueSky"
echo "  - autoreply logout       : Remove credentials"
echo "  - autoreply accounts list: List stored accounts"
echo
echo "✓ Current features work without authentication:"
echo "  - autoreply profile      : Query public profiles"
echo "  - autoreply search       : Search public posts"
echo
echo "✓ Storage:"
echo "  - Uses OS keyring when available (secure)"
echo "  - Falls back to file storage: ~/.config/autoreply/credentials.json"
echo
echo "✓ Multi-account support:"
echo "  - Store multiple accounts"
echo "  - Set default account"
echo "  - Switch between accounts"
echo
echo "For actual login, use:"
echo "  autoreply login --handle your.handle.bsky.social --password your-app-password"
echo
echo "To create an app password:"
echo "  1. Go to BlueSky Settings"
echo "  2. Navigate to App Passwords"
echo "  3. Create a new app password"
echo "  4. Use that password (not your main account password)"
echo

section "Demo Complete"
echo "See CLI-USAGE.md for complete documentation"
echo "See src/auth/README.md for authentication details"
echo
