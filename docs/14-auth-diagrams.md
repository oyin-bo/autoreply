# BlueSky MCP Authentication: Architecture Diagrams

Visual overview of the authentication system architecture for the autoreply MCP server.

---

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        MCP Client                               │
│                    (Gemini, VS Code, CLI)                       │
└────────────────────────┬────────────────────────────────────────┘
                         │ JSON-RPC over stdio/HTTP
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                     MCP Server (Go/Rust)                        │
│  ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓  │
│  ┃           Authentication Manager                        ┃  │
│  ┃  ┌──────────────────┐  ┌────────────────────────────┐  ┃  │
│  ┃  │  OAuth Client    │  │  Credential Storage        │  ┃  │
│  ┃  │                  │  │  ┌──────────────────────┐  │  ┃  │
│  ┃  │  • PKCE Flow     │  │  │  OS Keyring          │  │  ┃  │
│  ┃  │  • Device Flow   │◄─┼─►│  (Primary)           │  │  ┃  │
│  ┃  │  • DPoP Signing  │  │  │  • macOS Keychain    │  │  ┃  │
│  ┃  │  • Token Refresh │  │  │  • Windows Cred Mgr  │  │  ┃  │
│  ┃  │  • App Password  │  │  │  • Linux Secret Svc  │  │  ┃  │
│  ┃  └────────┬─────────┘  │  └──────────────────────┘  │  ┃  │
│  ┃           │            │  ┌──────────────────────┐  │  ┃  │
│  ┃           │            │  │  Encrypted File      │  │  ┃  │
│  ┃           │            │  │  (Fallback)          │  │  ┃  │
│  ┃           │            │  │  • AES-256-GCM       │  │  ┃  │
│  ┃           │            │  │  • User-only perms   │  │  ┃  │
│  ┃           │            │  └──────────────────────┘  │  ┃  │
│  ┃           │            └────────────────────────────┘  ┃  │
│  ┗━━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛  │
│              │                                                 │
│  ┌───────────▼──────────────────────────────────────────────┐  │
│  │                    MCP Tools                             │  │
│  │  • login          • post (with auth)                     │  │
│  │  • auth_status    • search (with auth)                   │  │
│  │  • logout         • profile (with auth)                  │  │
│  │  • set_default    • ...                                  │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────────────┘
                         │ HTTPS with DPoP
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                   BlueSky AT Protocol                           │
│  ┌──────────────────┐     ┌────────────────────────────────┐   │
│  │ OAuth Server     │     │ Personal Data Servers (PDS)    │   │
│  │ • Authorization  │     │ • User repositories            │   │
│  │ • Token Exchange │     │ • Posts, profiles, feeds       │   │
│  │ • Token Refresh  │     │ • Multi-PDS support            │   │
│  └──────────────────┘     └────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## OAuth PKCE Flow Sequence

```
User                CLI/MCP           OAuth Server       PDS
 |                    |                    |             |
 |  1. autoreply     |                    |             |
 |     login         |                    |             |
 |-----------------→ |                    |             |
 |                   |                    |             |
 |                   | 2. Generate PKCE   |             |
 |                   |    challenge       |             |
 |                   |                    |             |
 |                   | 3. Authorization   |             |
 |                   |    Request         |             |
 |                   |-------------------→|             |
 |                   |                    |             |
 |  4. Open browser  |                    |             |
 | ←-----------------|                    |             |
 |                   |                    |             |
 |  5. Login &       |                    |             |
 |     Consent       |                    |             |
 |-------------------------------------- →|             |
 |                   |                    |             |
 |                   | 6. Auth code       |             |
 |                   |   (callback)       |             |
 |                   | ←------------------|             |
 |                   |                    |             |
 |                   | 7. Exchange code   |             |
 |                   |    + PKCE verifier |             |
 |                   |    + DPoP proof    |             |
 |                   |-------------------→|             |
 |                   |                    |             |
 |                   | 8. Access token    |             |
 |                   |    Refresh token   |             |
 |                   | ←------------------|             |
 |                   |                    |             |
 |                   | 9. Store tokens    |             |
 |                   |    in keyring      |             |
 |                   |                    |             |
 |  10. Success!     |                    |             |
 | ←-----------------|                    |             |
 |                   |                    |             |
 |  11. API call     |                    |             |
 |     (post, etc)   |                    |             |
 |-----------------→ |                    |             |
 |                   |                    |             |
 |                   | 12. Authenticated  |             |
 |                   |     request        |             |
 |                   |     + DPoP proof   |             |
 |                   |------------------------------→  |
 |                   |                    |             |
 |  13. Response     |                    |             |
 | ←-----------------|------------------------------←  |
```

---

## Device Authorization Flow Sequence

```
User                CLI/MCP           OAuth Server
 |                    |                    |
 |  1. autoreply     |                    |
 |     login         |                    |
 |     --method      |                    |
 |     device        |                    |
 |-----------------→ |                    |
 |                   |                    |
 |                   | 2. Device auth     |
 |                   |    request         |
 |                   |-------------------→|
 |                   |                    |
 |                   | 3. Device code     |
 |                   |    User code       |
 |                   |    Verify URL      |
 |                   | ←------------------|
 |                   |                    |
 |  4. Display:      |                    |
 |     "Visit URL    |                    |
 |      Enter code"  |                    |
 | ←-----------------|                    |
 |                   |                    |
 |  5. Open browser  |                    |
 |     on phone/     |                    |
 |     other device  |                    |
 |                   |                    |
 |  6. Enter user    |                    |
 |     code          |                    |
 |-------------------------------------- →|
 |                   |                    |
 |  7. Login &       |                    |
 |     Consent       |                    |
 |-------------------------------------- →|
 |                   |                    |
 |                   | 8. Poll for token  |
 |                   |    (every 5s)      |
 |                   |-------------------→|
 |                   | ←------------------|
 |                   |    (pending)       |
 |                   |                    |
 |                   | 9. Poll again      |
 |                   |-------------------→|
 |                   |                    |
 |                   | 10. Access token   |
 |                   |     Refresh token  |
 |                   | ←------------------|
 |                   |                    |
 |                   | 11. Store tokens   |
 |                   |     in keyring     |
 |                   |                    |
 |  12. Success!     |                    |
 | ←-----------------|                    |
```

---

## Token Lifecycle State Machine

```
┌──────────────┐
│   No Token   │
│  (Logged Out)│
└──────┬───────┘
       │
       │ login (OAuth/password)
       ▼
┌──────────────┐
│ Valid Token  │◄────────┐
│  (Active)    │         │
└──────┬───────┘         │
       │                 │
       │ API call        │ refresh
       │ (check expiry)  │ success
       ▼                 │
┌──────────────┐         │
│ Token Valid? │─ Yes ──→│ Use token
└──────┬───────┘         │ for API call
       │                 │
       │ No (expired)    │
       ▼                 │
┌──────────────┐         │
│ Try Refresh  │─────────┘
└──────┬───────┘
       │
       │ refresh failed
       ▼
┌──────────────┐
│ Auth Error   │
│ (Re-auth     │
│  Required)   │
└──────────────┘
       │
       │ logout or
       │ re-login
       ▼
┌──────────────┐
│   No Token   │
└──────────────┘
```

---

## Multi-Account Storage Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    OS Keyring                               │
│  ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓   │
│  ┃ Service: autoreply-mcp                             ┃   │
│  ┃                                                     ┃   │
│  ┃  alice.bsky.social/access_token  = "eyJ..."        ┃   │
│  ┃  alice.bsky.social/refresh_token = "eyJ..."        ┃   │
│  ┃  alice.bsky.social/dpop_key      = "-----BEGIN"    ┃   │
│  ┃                                                     ┃   │
│  ┃  bob.bsky.social/access_token    = "eyJ..."        ┃   │
│  ┃  bob.bsky.social/refresh_token   = "eyJ..."        ┃   │
│  ┃  bob.bsky.social/dpop_key        = "-----BEGIN"    ┃   │
│  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ Metadata stored in
                              ▼
┌─────────────────────────────────────────────────────────────┐
│         ~/.config/autoreply-mcp/config.json                 │
│  ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓   │
│  ┃ {                                                   ┃   │
│  ┃   "version": "2.0",                                 ┃   │
│  ┃   "accounts": [                                     ┃   │
│  ┃     {                                               ┃   │
│  ┃       "handle": "alice.bsky.social",                ┃   │
│  ┃       "did": "did:plc:abc123xyz789",                ┃   │
│  ┃       "pds": "https://pds.example.com",             ┃   │
│  ┃       "storage_ref": "keyring",                     ┃   │
│  ┃       "created_at": "2025-01-01T09:00:00Z",         ┃   │
│  ┃       "last_used": "2025-01-14T15:45:00Z"           ┃   │
│  ┃     },                                              ┃   │
│  ┃     {                                               ┃   │
│  ┃       "handle": "bob.bsky.social",                  ┃   │
│  ┃       "did": "did:plc:xyz789abc123",                ┃   │
│  ┃       "pds": "https://bsky.social",                 ┃   │
│  ┃       "storage_ref": "keyring",                     ┃   │
│  ┃       "created_at": "2025-01-10T14:30:00Z",         ┃   │
│  ┃       "last_used": "2025-01-14T16:00:00Z"           ┃   │
│  ┃     }                                               ┃   │
│  ┃   ],                                                ┃   │
│  ┃   "default_account": "alice.bsky.social"            ┃   │
│  ┃ }                                                   ┃   │
│  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛   │
└─────────────────────────────────────────────────────────────┘
```

---

## Credential Storage Fallback Strategy

```
┌────────────────────────────────────────────────────┐
│          Credential Storage Decision Tree          │
└────────────────────────────────────────────────────┘

                    ┌──────────┐
                    │  Start   │
                    └────┬─────┘
                         │
                         ▼
              ┌──────────────────────┐
              │ OS Keyring Available?│
              └──────┬───────────────┘
                     │
         ┌───────────┴────────────┐
         │                        │
        Yes                      No
         │                        │
         ▼                        ▼
┌─────────────────┐    ┌──────────────────────┐
│  Use OS Keyring │    │ Encrypted File Stor  │
│  (Primary)      │    │ Available?           │
│                 │    └──────┬───────────────┘
│ • macOS:        │           │
│   Keychain      │   ┌───────┴────────┐
│ • Windows:      │   │                │
│   Credential    │  Yes              No
│   Manager       │   │                │
│ • Linux:        │   ▼                ▼
│   Secret        │ ┌────────┐  ┌─────────────┐
│   Service       │ │ Use    │  │ Prompt user │
└─────────────────┘ │ Encrypt│  │ for consent │
                    │ File   │  └──────┬──────┘
                    │        │         │
                    │ AES-   │   ┌─────┴──────┐
                    │ 256-   │   │            │
                    │ GCM    │  Yes          No
                    └────────┘   │            │
                                 ▼            ▼
                          ┌──────────┐  ┌─────────┐
                          │ Plaintext│  │ Abort   │
                          │ File     │  │ (Fail)  │
                          │ (Warning)│  └─────────┘
                          └──────────┘
```

---

## MCP Tool Call Flow with Authentication

```
Client                  MCP Server              Auth Manager           BlueSky
  |                          |                        |                   |
  |  tools/call: post       |                        |                   |
  |  { account: "alice",    |                        |                   |
  |    text: "Hello!" }     |                        |                   |
  |------------------------→|                        |                   |
  |                         |                        |                   |
  |                         | Get credentials        |                   |
  |                         | for "alice"            |                   |
  |                         |----------------------→ |                   |
  |                         |                        |                   |
  |                         |                        | Check token       |
  |                         |                        | expiry            |
  |                         |                        |                   |
  |                         |                        | Token valid?      |
  |                         |                        | ─────────┐        |
  |                         |                        |          │        |
  |                         |                        | ←────────┘        |
  |                         |                        | Yes               |
  |                         |                        |                   |
  |                         | Credentials            |                   |
  |                         | (access_token, dpop)   |                   |
  |                         | ←----------------------|                   |
  |                         |                        |                   |
  |                         | Create post            |                   |
  |                         | + DPoP proof           |                   |
  |                         |-------------------------------------→       |
  |                         |                        |                   |
  |                         |                        |                   |
  |                         | Post created           |                   |
  |                         | (URI, CID)             |                   |
  |                         | ←-------------------------------------|   |
  |                         |                        |                   |
  |  Result: success        |                        |                   |
  |  { uri: "at://..." }    |                        |                   |
  | ←-----------------------|                        |                   |
```

### With Token Refresh

```
Client                  MCP Server              Auth Manager           BlueSky
  |                          |                        |                   |
  |  tools/call: post       |                        |                   |
  |------------------------→|                        |                   |
  |                         |                        |                   |
  |                         | Get credentials        |                   |
  |                         |----------------------→ |                   |
  |                         |                        |                   |
  |                         |                        | Check token       |
  |                         |                        | expiry            |
  |                         |                        | (EXPIRED!)        |
  |                         |                        |                   |
  |                         |                        | Refresh token     |
  |                         |                        | + DPoP proof      |
  |                         |                        |------------------→|
  |                         |                        |                   |
  |                         |                        | New access token  |
  |                         |                        | ←-----------------|
  |                         |                        |                   |
  |                         |                        | Store new token   |
  |                         |                        | in keyring        |
  |                         |                        |                   |
  |                         | Credentials            |                   |
  |                         | (NEW access_token)     |                   |
  |                         | ←----------------------|                   |
  |                         |                        |                   |
  |                         | Create post            |                   |
  |                         | + DPoP proof           |                   |
  |                         |-------------------------------------→       |
  |                         |                        |                   |
  |  Result: success        |                        |                   |
  | ←-----------------------| ←-------------------------------------|   |
```

---

## CLI User Journey

```
┌─────────────────────────────────────────────────────────────┐
│                    First-Time Setup                         │
└─────────────────────────────────────────────────────────────┘

1. User installs autoreply
   $ go install github.com/oyin-bo/autoreply/go-server/cmd/autoreply@latest

2. User attempts to post without auth
   $ autoreply post --text "Hello!"
   ✗ Error: No authenticated accounts found.
   → Run 'autoreply login' to authenticate.

3. User runs login command
   $ autoreply login
   
   Choose authentication method:
     1) OAuth (browser-based) [Recommended]
     2) Device code (for remote/headless)
     3) App password (legacy)
   
   Selection: 1

4. System opens browser
   Opening browser for authentication...
   
   [Browser opens to BlueSky OAuth page]

5. User logs in and grants permission
   [In browser: Login → Consent → Authorize]

6. CLI receives callback
   ✓ Successfully authenticated as @alice.bsky.social
     DID: did:plc:abc123xyz789
     PDS: https://pds.example.com
   
   Credentials stored securely in system keyring.

7. User can now post
   $ autoreply post --text "Hello from MCP!"
   ✓ Posted as @alice.bsky.social
     URI: at://alice.bsky.social/app.bsky.feed.post/abc123


┌─────────────────────────────────────────────────────────────┐
│                   Multiple Accounts                         │
└─────────────────────────────────────────────────────────────┘

1. User adds second account
   $ autoreply login
   
   Enter your BlueSky handle: bob.bsky.social
   [OAuth flow...]
   ✓ Successfully authenticated as @bob.bsky.social

2. User lists accounts
   $ autoreply accounts
   
   Authenticated Accounts:
     ✓ alice.bsky.social (default)
       DID: did:plc:abc123xyz789
       Expires: 2025-01-15 10:30:00
   
     ✓ bob.bsky.social
       DID: did:plc:xyz789abc123
       Expires: 2025-01-20 14:45:00

3. User posts as specific account
   $ autoreply post --account bob.bsky.social --text "Hello from Bob!"
   ✓ Posted as @bob.bsky.social

4. User changes default account
   $ autoreply use bob.bsky.social
   ✓ Default account set to @bob.bsky.social
   
   $ autoreply post --text "Now posting as Bob by default"
   ✓ Posted as @bob.bsky.social

5. User logs out of one account
   $ autoreply logout alice.bsky.social
   ✓ Logged out from @alice.bsky.social
     Credentials removed from system keyring.
```

---

## Implementation Phases Visual

```
┌─────────────────────────────────────────────────────────────────┐
│                    9-Week Implementation Plan                   │
└─────────────────────────────────────────────────────────────────┘

Week 1-2: Foundation
├── Credential Manager
│   ├── OS Keyring Integration
│   ├── Encrypted File Fallback
│   └── Config File Management
└── Unit Tests
    └── Storage Operations

Week 3-4: OAuth Client
├── PKCE Flow
│   ├── Challenge Generation
│   ├── Browser Callback
│   └── Token Exchange
├── Device Flow
│   ├── Device Code Request
│   ├── Polling Logic
│   └── Token Exchange
├── DPoP Support
│   └── JWT Generation
└── Token Refresh
    └── Auto-refresh Logic

Week 5: CLI Integration
├── Commands
│   ├── login
│   ├── accounts
│   ├── use
│   └── logout
├── Interactive Prompts
└── Help Text

Week 6: MCP Tools
├── Tool Implementations
│   ├── login
│   ├── auth_status
│   ├── logout
│   └── set_default_account
└── Existing Tool Updates
    └── Add 'account' parameter

Week 7: Token Lifecycle
├── Expiry Checking
├── Automatic Refresh
├── Retry Logic
└── Background Refresh

Week 8: Testing & Hardening
├── Integration Tests
├── Platform Tests
│   ├── macOS
│   ├── Windows
│   └── Linux
├── Security Audit
└── Performance Tests

Week 9: Documentation
├── User Guides
├── Troubleshooting
├── Architecture Docs
└── Demo Videos

Timeline:  [████████████████████████████████████] 100%
Progress:  [██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  ~5% (Planning complete)
```

---

## Platform Support Matrix

```
┌────────────────────────────────────────────────────────────┐
│           Credential Storage by Platform                   │
└────────────────────────────────────────────────────────────┘

┌──────────┬─────────────────┬──────────────┬─────────────┐
│ Platform │   Primary       │   Fallback   │  Last Resort│
├──────────┼─────────────────┼──────────────┼─────────────┤
│ macOS    │ Keychain        │ Encrypted    │ Plaintext   │
│          │ (built-in)      │ File         │ (consent)   │
│          │ ✅ Available    │ ✅ Works     │ ⚠️ Warning  │
├──────────┼─────────────────┼──────────────┼─────────────┤
│ Windows  │ Credential      │ Encrypted    │ Plaintext   │
│          │ Manager         │ File         │ (consent)   │
│          │ (built-in)      │ ✅ Works     │ ⚠️ Warning  │
│          │ ✅ Available    │              │             │
├──────────┼─────────────────┼──────────────┼─────────────┤
│ Linux    │ Secret Service  │ Encrypted    │ Plaintext   │
│ (GNOME)  │ (libsecret)     │ File         │ (consent)   │
│          │ ✅ Available    │ ✅ Works     │ ⚠️ Warning  │
├──────────┼─────────────────┼──────────────┼─────────────┤
│ Linux    │ N/A             │ Encrypted    │ Plaintext   │
│ (headless│                 │ File         │ (consent)   │
│  server) │ ⚠️ Not Available│ ✅ Works     │ ⚠️ Warning  │
└──────────┴─────────────────┴──────────────┴─────────────┘
```

---

**Document Version:** 1.0  
**Last Updated:** 2025-01-01  
**Status:** ✅ Planning Complete

For detailed implementation information, see:
- [12-auth-implementation-plan.md](./12-auth-implementation-plan.md)
- [README-AUTH.md](./README-AUTH.md)
