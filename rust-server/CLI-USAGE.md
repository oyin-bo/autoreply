# CLI Usage Guide

This guide covers all CLI commands and options for the autoreply Rust server.

## Table of Contents

- [Overview](#overview)
- [Global Options](#global-options)
- [Authentication Commands](#authentication-commands)
  - [login](#login)
  - [logout](#logout)
  - [accounts](#accounts)
- [Data Commands](#data-commands)
  - [profile](#profile)
  - [search](#search)
- [Examples](#examples)

## Overview

The autoreply CLI provides commands for:
- **Authentication**: Manage BlueSky credentials
- **Data Access**: Query profiles and search posts

When no command is provided, the application runs in MCP server mode.

## Global Options

Available for all commands:

```
-v, --verbose    Enable verbose logging (to stderr)
    --quiet      Suppress non-error output
-h, --help       Print help information
-V, --version    Print version information
```

Examples:
```bash
autoreply --verbose profile --account alice.bsky.social
autoreply --quiet search --account bob.bsky.social --query rust
```

## Authentication Commands

### login

Authenticate with BlueSky using an app password.

**Usage:**
```bash
autoreply login [OPTIONS]
```

**Options:**
```
-u, --handle <HANDLE>        Handle (e.g., alice.bsky.social)
-p, --password <PASSWORD>    App password
-s, --service <SERVICE>      Service URL (defaults to https://bsky.social)
```

**Examples:**

Interactive login (prompts for credentials):
```bash
autoreply login
```

Specify credentials via flags:
```bash
autoreply login --handle alice.bsky.social --password app-password-here
```

Use custom service URL:
```bash
autoreply login --handle alice.bsky.social --service https://custom.pds.example
```

**Creating App Passwords:**
1. Go to BlueSky Settings
2. Navigate to App Passwords
3. Create a new app password
4. Use this password (not your main account password) with the CLI

**Storage:**
- Credentials are stored securely in your OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- If keyring is unavailable, falls back to encrypted file storage in `~/.config/autoreply/credentials.json`
- First account you add becomes the default account

**Output:**
```
✓ Successfully authenticated as @alice.bsky.social
  DID: did:plc:abc123...
  Using: OS keyring storage
```

---

### logout

Remove stored credentials for an account.

**Usage:**
```bash
autoreply logout [OPTIONS]
```

**Options:**
```
-u, --handle <HANDLE>    Handle to logout (defaults to current/default account)
```

**Examples:**

Logout from default account:
```bash
autoreply logout
```

Logout from specific account:
```bash
autoreply logout --handle alice.bsky.social
```

**Output:**
```
✓ Logged out from @alice.bsky.social
```

---

### accounts

Manage authenticated accounts.

**Usage:**
```bash
autoreply accounts <COMMAND>
```

**Subcommands:**
- `list` - List all stored accounts
- `default <HANDLE>` - Set default account

#### accounts list

List all authenticated accounts.

**Usage:**
```bash
autoreply accounts list
```

**Example output:**
```
Authenticated accounts (2):
  • @alice.bsky.social (default)
  • @bob.bsky.social
```

#### accounts default

Set the default account to use.

**Usage:**
```bash
autoreply accounts default <HANDLE>
```

**Examples:**
```bash
autoreply accounts default alice.bsky.social
```

**Output:**
```
✓ Set @alice.bsky.social as default account
```

---

## Data Commands

### profile

Retrieve user profile information from BlueSky.

**Usage:**
```bash
autoreply profile --account <ACCOUNT>
```

**Options:**
```
-a, --account <ACCOUNT>    Handle (alice.bsky.social) or DID (did:plc:...)
```

**Examples:**

Query by handle:
```bash
autoreply profile --account alice.bsky.social
```

Query by DID:
```bash
autoreply profile --account did:plc:abc123xyz
```

With verbose logging:
```bash
autoreply --verbose profile --account alice.bsky.social
```

**Output:**

Returns markdown-formatted profile information:
```markdown
# Profile: alice.bsky.social

**Display Name:** Alice Smith
**Bio:** Software engineer and Rust enthusiast 🦀
**Followers:** 1,234
**Following:** 567
**Posts:** 8,901

**Created:** 2023-01-15T10:30:00Z
```

**Exit Codes:**
- `0` - Success
- `1` - Invalid arguments or usage error
- `2` - Network or connection error
- `3` - Not found error
- `4` - Timeout error
- `5` - Other application error

---

### search

Search posts within a user's repository.

**Usage:**
```bash
autoreply search --account <ACCOUNT> --query <QUERY> [--limit <LIMIT>]
```

**Options:**
```
-a, --account <ACCOUNT>    Handle or DID
-q, --query <QUERY>        Search terms (case-insensitive)
-l, --limit <LIMIT>        Maximum number of results (default: 50, max: 200)
```

**Examples:**

Basic search:
```bash
autoreply search --account alice.bsky.social --query "rust programming"
```

Limit results:
```bash
autoreply search --account alice.bsky.social --query rust --limit 10
```

Search by DID:
```bash
autoreply search --account did:plc:abc123 --query "machine learning"
```

**Output:**

Returns markdown-formatted search results with highlighted matches:
```markdown
# Search Results for alice.bsky.social

Query: "rust programming"
Found: 3 posts

---

## Post 1

**Posted:** 2024-01-15T14:30:00Z

Just finished a **<mark>Rust</mark>** **<mark>programming</mark>** tutorial. Amazing language! 🦀

**URI:** at://alice.bsky.social/app.bsky.feed.post/abc123

---

## Post 2

...
```

**Search Features:**
- Case-insensitive matching
- Searches post text, quote text, and embedded content
- Highlights matching terms with `<mark>` tags
- Returns most recent matches first

**Exit Codes:**
- Same as profile command

---

## Examples

### Complete Workflow

1. **Login to BlueSky:**
```bash
autoreply login --handle alice.bsky.social --password app-password-xyz
```

2. **View your stored accounts:**
```bash
autoreply accounts list
```

3. **Query a profile:**
```bash
autoreply profile --account bob.bsky.social
```

4. **Search posts:**
```bash
autoreply search --account bob.bsky.social --query "rust" --limit 20
```

5. **Add a second account:**
```bash
autoreply login --handle charlie.bsky.social --password another-password
```

6. **Switch default account:**
```bash
autoreply accounts default charlie.bsky.social
```

7. **Logout when done:**
```bash
autoreply logout --handle alice.bsky.social
```

### Scripting Examples

**Check if an account exists:**
```bash
if autoreply profile --account alice.bsky.social > /dev/null 2>&1; then
    echo "Account exists"
else
    echo "Account not found"
fi
```

**Save search results to file:**
```bash
autoreply search --account alice.bsky.social --query "rust" > results.md
```

**Search with error handling:**
```bash
autoreply search --account alice.bsky.social --query "rust" || {
    exit_code=$?
    case $exit_code in
        3) echo "Account not found" ;;
        4) echo "Request timed out" ;;
        *) echo "Error occurred: $exit_code" ;;
    esac
    exit $exit_code
}
```

### Advanced Usage

**Using custom PDS:**
```bash
# Login to custom PDS
autoreply login --handle myhandle.custom.social --service https://pds.custom.social

# Query profiles on that PDS
autoreply profile --account myhandle.custom.social
```

**Quiet mode for scripts:**
```bash
# Only outputs the result, no logging
autoreply --quiet search --account alice.bsky.social --query rust
```

**Debug mode:**
```bash
# Verbose logging for troubleshooting
autoreply --verbose profile --account alice.bsky.social
```

## Troubleshooting

### Authentication Fails

```
Error: Authentication failed with status 401: ...
```

**Solutions:**
- Verify you're using an app password, not your main account password
- Check that the handle is correct (include the domain: alice.bsky.social)
- Ensure your app password hasn't been revoked in BlueSky settings

### Keyring Not Available

If you see file storage warnings:
```
Using: file storage
```

This means OS keyring is unavailable. Credentials are stored in:
- Linux/macOS: `~/.config/autoreply/credentials.json`
- Windows: `%APPDATA%\autoreply\credentials.json`

The file has restricted permissions (0600) for security.

### Network Errors

```
Error: Network error: connection timed out
```

**Solutions:**
- Check your internet connection
- Verify proxy settings if using a corporate network
- Try increasing timeout with verbose mode to see detailed errors

### Profile/Search Not Found

```
Error: Not found: DID resolution failed
```

**Solutions:**
- Verify the handle is spelled correctly
- Check that the account exists and is public
- Try using the DID directly instead of handle

## See Also

- [Authentication README](./src/auth/README.md) - Detailed authentication documentation
- [Main README](./README.md) - Project overview and MCP server mode
- [AT Protocol Documentation](https://atproto.com/) - Protocol specification
