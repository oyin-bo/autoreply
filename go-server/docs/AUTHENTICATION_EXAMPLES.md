# Authentication Examples

This document provides practical examples of using the authentication features in the Go server.

## Prerequisites

1. Build the server:
```bash
cd go-server
go build -o autoreply ./cmd/autoreply
```

2. Generate an app password in BlueSky:
   - Go to https://bsky.app/settings/app-passwords
   - Click "Add App Password"
   - Give it a name (e.g., "autoreply-cli")
   - Copy the generated password

## Basic Authentication Flow

### 1. Login

```bash
./autoreply login --handle your.handle.bsky.social --password xxxx-xxxx-xxxx-xxxx
```

Expected output:
```
# Login Successful

Successfully authenticated as **@your.handle.bsky.social**

**DID:** `did:plc:...`

Credentials have been securely stored and will be used for authenticated operations.
```

### 2. List Accounts

```bash
./autoreply accounts
```

Expected output:
```
# Authenticated Accounts

Found 1 authenticated account(s):

- **@your.handle.bsky.social** *(default)*

Default account: **@your.handle.bsky.social**
```

### 3. Logout

```bash
./autoreply logout
```

Expected output:
```
# Logout Successful

Credentials for **@your.handle.bsky.social** have been removed.
```

## Multi-Account Management

### Login with Multiple Accounts

```bash
# Login first account
./autoreply login --handle alice.bsky.social --password xxxx-xxxx-xxxx-xxxx

# Login second account
./autoreply login --handle bob.bsky.social --password yyyy-yyyy-yyyy-yyyy
```

### List All Accounts

```bash
./autoreply accounts
```

Expected output:
```
# Authenticated Accounts

Found 2 authenticated account(s):

- @alice.bsky.social
- **@bob.bsky.social** *(default)*

Default account: **@bob.bsky.social**
```

### Set Default Account

```bash
./autoreply accounts --action set-default --handle alice.bsky.social
```

Expected output:
```
# Default Account Updated

Default account set to **@alice.bsky.social**
```

### Logout Specific Account

```bash
./autoreply logout --handle bob.bsky.social
```

## MCP Server Mode

Start the MCP server (authentication tools available via JSON-RPC):

```bash
./autoreply
```

### Example MCP Request - Login

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "login",
    "arguments": {
      "handle": "your.handle.bsky.social",
      "password": "xxxx-xxxx-xxxx-xxxx"
    }
  }
}
```

### Example MCP Request - List Accounts

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "login",
    "arguments": { "command": "list" }
  }
}
```

### Example MCP Request - Logout (remove credentials)

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "login",
    "arguments": { "command": "delete", "handle": "your.handle.bsky.social" }
  }
}
```

## Error Handling

### Invalid Credentials

```bash
./autoreply login --handle test.bsky.social --password wrong-password
```

Expected error:
```
Error: Failed to create session: authentication failed with status 401: ...
```

### No Default Handle

```bash
./autoreply logout
```

If no default handle is set:
```
Error: No handle provided and no default handle set: no default handle set
```

### Account Not Found

```bash
./autoreply logout --handle nonexistent.bsky.social
```

Expected error:
```
Error: Failed to delete credentials: no credentials found for handle: nonexistent.bsky.social
```

## Security Notes

1. **App Passwords**: Never share your app passwords. They provide full access to your account.

2. **Credential Storage**: Credentials are stored securely:
   - **macOS**: In Keychain
   - **Windows**: In Credential Manager
   - **Linux**: In Secret Service (via D-Bus)
   - **Fallback**: Encrypted file in `~/.autoreply/`

3. **Token Lifetime**:
   - Access tokens expire after 2 hours
   - Refresh tokens expire after 90 days
   - Future updates will add automatic token refresh

4. **Revocation**: To revoke access:
   - Use `autoreply logout` to remove local credentials
   - Revoke the app password in BlueSky settings

## Troubleshooting

### Keyring Issues on Linux

If you encounter keyring errors on Linux:

```bash
# Install required packages
# Ubuntu/Debian:
sudo apt-get install libsecret-1-0 libsecret-1-dev

# Fedora:
sudo dnf install libsecret libsecret-devel

# Arch:
sudo pacman -S libsecret
```

### File Backend Permission Issues

If using file backend, ensure proper permissions:

```bash
chmod 700 ~/.autoreply
chmod 600 ~/.autoreply/*
```

### Testing Without Real Credentials

For testing purposes, you can use the accounts list command without any credentials:

```bash
./autoreply accounts
```

This will safely report that no accounts are configured.

## Next Steps

After authenticating, you can use authenticated operations with other tools that support them (to be implemented in future updates).
