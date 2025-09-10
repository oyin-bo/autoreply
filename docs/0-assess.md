# Project assessment: autoreply

## Summary

- **Purpose**: a small Node.js MCP (Model Context Protocol) server and CLI that integrates with BlueSky (AT Protocol / bsky) to provide tools for fetching feeds, searching, posting, and performing actions (like/ repost/ delete). It exposes those capabilities both as a command-line utility and as a stdio-based MCP server intended for use with Gemini/other MCP clients and VS Code.
- **Key files**: `index.js` (main implementation), `package.json`, and a tiny example MCP in `helloworld/tinymcp.js`.

## Goals and intended usage

- Provide a bridge between a client (Gemini CLI, VS Code MCP integration, or ad-hoc CLI) and BlueSky APIs so that clients can call well-structured tools (feed, profile, search, thread, post, like, repost, delete).  
- Support both authenticated and anonymous (incognito) access.  
- Offer an easy local-install path that registers the MCP server with Gemini CLI and VS Code (via `localInstall`).

## High-level architecture and implementation aspects

- **Single-file implementation**: most functionality is in `index.js` as a self-contained script that can run interactively (TTY) or in stdio MCP mode. This favors portability but makes the file large and mixes concerns (network, protocol, CLI, installation, parsing).

- **MCP server**: `McpServer` implements the MCP surface expected by clients:
  - `initialize` returns static server metadata and capabilities.
  - `notifications/initialized` and `shutdown` are present for protocol completeness.
  - `tools/list` enumerates tools by introspecting the `Tools` instance.
  - `tools/call` invokes a named tool and returns a response with both textual and structured content. It wraps results and marshals errors into MCP-style responses.

- **Tools**: `Tools` class contains the BlueSky-related operations exposed as callable tools. Each tool has a companion `'<name>:tool'` metadata object that provides name, description and JSON schemas for inputs/outputs. Major tool functions:
  - `login` — stores BlueSky handle/password (uses `clientLogin`).
  - `feed` — fetches a feed (popular feed by default or specific feed) with pagination. Formats posts using `formatPost`.
  - `profile` — fetches profile and follower/following pages.
  - `search` — search posts with optional `from` filter and pagination.
  - `thread` — fetches a full thread for a post URI (recursively flattens replies and attempts to restore missing root context if needed).
  - `post` — creates a post or reply (handles reply resolution to strongRef root/parent including CID retrieval).
  - `like`, `repost`, `delete` — perform authenticated actions on posts.

- **BlueSky client handling (AtpAgent)**:
  - `clientIncognito()` creates a single AtpAgent pointed at `https://public.api.bsky.app` for unauthenticated reads.
  - `clientLogin` logs in and caches AtpAgent instances per handle in `_clientLoggedInByHandle` and persists credentials using `keytar` if available.
  - `clientLoginOrFallback` chooses which client to use based on provided credentials or persisted default handle.

- **Credentials**: optional dependency on `keytar` is used when available; otherwise a fallback file `.bluesky_creds.json` in the package directory is used. The code attempts to detect and adapt to absence of system keyring.

- **Output formatting and embed extraction**: `formatPost` normalizes posts into a textual view and a `structured` object according to a declared `PostSchema`. Embedded images, video, external links, and record embeds are handled by `extractEmbeds` + helper functions to produce CDN-friendly URLs.

- **URI parsing utilities**: a set of helper functions for parsing/normalizing different BlueSky URL and URI forms: `breakPostURL`, `breakFeedURI`, `makeFeedUri`, `unwrapShortDID`, `shortenDID`, `unwrapShortHandle`, `cheapNormalizeHandle`. These allow the tools to accept multiple input formats (bsky.app URLs, at:// URIs, DID short forms, or simple handles).

- **CLI & install mode**:
  - When run in an interactive TTY, the script exposes a small CLI allowing direct invocation of MCP methods or tools and a human-friendly preview mode.
  - `localInstall()` writes entries into Gemini CLI settings and the VS Code `mcp.json` to register this script as a local MCP server for convenience.

- **Error handling**: a custom `McpError` class attaches `mcpCode` and `mcpExtra` properties so that serialized MCP errors include structured diagnostic data.

## Notable/interesting solutions

- **Keytar fallback**: the implementation gracefully degrades from `keytar` to a local JSON credential file (`.bluesky_creds.json`) when the optional native dependency is not available, enabling cross-platform operation without native build dependencies.

- **Agent caching**: authenticated agents are cached per handle in `_clientLoggedInByHandle` to avoid repeated logins and re-use credentials, improving responsiveness and reducing login calls.

- **Flexible input parsing**: the code tolerates multiple BlueSky input formats (HTTP URLs, at:// URIs, short handles, and did:plc: identifiers) which improves UX for callers.

- **Thread flattening logic**: the recursive `flattenThread` function in `thread()` carefully flattens nested replies and includes logic to fetch the root context if the anchor record indicates a root outside the currently returned thread.

- **Structured responses**: every tool returns both a textual rendering and a `structuredContent` JSON fragment suitable for clients that can interpret structured output.

## Potential issues, code smells and improvement suggestions

- **Single large file**: `index.js` mixes protocol handling, CLI, installation, BlueSky client logic and utility functions. Consider splitting responsibilities into modules (mcp/protocol, tools/blueSky, utils/uri, storage/credentials).

- **Credential storage security**: fallback stores passwords in plain JSON inside the project directory. This is insecure and should be avoided for anything beyond experimentation. Prefer OS keyring (`keytar`) or a secure local store, and never check credentials into git.

- **Dependency mismatch**: `package.json` lists `@atcute/client` as a dependency, but code imports `@atproto/api` (AtpAgent). Ensure dependencies reflect actual imports; pin versions or update imports to a single supported client library.

- **Potential bug in `likelyDID`**: the expression `!text.trim().indexOf('did:')` uses the logical NOT operator with `indexOf` return value and is non-idiomatic and possibly incorrect for some inputs. Prefer `text.trim().startsWith('did:')`.

- **Use of `eval` as fallback when parsing CLI params is unsafe**. Replace `eval` with more robust parsing or require JSON only.

- **Minimal automated tests and type checking**: consider adding unit tests and/or migrating to TypeScript to catch mistakes early (the file contains `@ts-check` annotations but is plain JS).

- **Error surface**: some network or API errors may leak internal stacks into MCP `data`. Consider sanitizing error payloads before exposing them to clients.

- **Rate limiting & retries**: currently there is no retry logic or rate-limit handling for API calls. Adding a small retry/backoff and handling rate-limited responses would make the tool more robust.

- **Better schema validation**: tools include schema metadata, but inputs are not strictly validated. Adding runtime validation would improve reliability when the MCP client sends malformed input.

## Files of interest and functional blocks

- `index.js`:
  - MCP server and loop (runMcp, runInteractive) — protocol glue and CLI entrypoints.
  - `McpServer` — method implementations that map MCP calls to `Tools`.
  - `Tools` — BlueSky operations and client lifecycle management.
  - Credentials helper (`requireOrMockKeytar`) + fallback credential store implementation.
  - BlueSky data shaping (formatPost, extractEmbeds and helpers).
  - URI/identifier helpers (breakPostURL, breakFeedURI, unwrapShortDID, unwrapShortHandle, etc.).

- `helloworld/tinymcp.js`: a compact example MCP server showing the minimal structure (initialize, tools/list, tools/call) and a single `random` tool. Useful as a reference implementation.

- `package.json`: declares optional `keytar` and a client dependency mismatch (see note above).

## Final notes

- The project is a practical, usable skeleton for exposing BlueSky functionality via MCP. It has several pragmatic choices (fallback credential store, flexible input parsing, agent caching) that make it convenient for experimentation and local use.
- For production or wider distribution, split the code into modules, secure credentials, fix the small correctness issues, and add tests and CI.