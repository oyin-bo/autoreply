# Structured content versus text

## Current state

Both Go and Rust implementations define a `ToolResult` structure that contains a list of `ContentItem` objects. A `ContentItem` consists of a `type` (e.g., "text"), the `text` content, and optional `metadata`.

### Go Implementation

- The `tools/call` handler expects each tool's `Call` method to return a `(*ToolResult, error)`.
- The returned `ToolResult` object is directly serialized as the `result` field in the final JSON-RPC response.
- The `search` tool constructs a single `ContentItem` of type `text`, where the content is a large, pre-formatted Markdown string.

### Rust Implementation

- The `tools/call` dispatcher (`handle_tool_call`) routes requests to specific tool handlers (e.g., `handle_search`).
- Each tool handler is responsible for creating the entire `McpResponse` object, either for success or failure.
- A successful tool execution builds a `ToolResult` struct. This struct is then serialized into a `serde_json::Value` and placed inside the `result` field of the `McpResponse`.
- The `search` tool, similar to the Go version, returns a `ToolResult` containing a single `ContentItem` with its content being a formatted Markdown string.

In both cases, the tool result is effectively a single block of text, formatted as Markdown, and delivered within a `ContentItem`.

### Per-tool summary (Go server)

- `accounts`
	- Input schema: object with optional `action` (string: "list" | "set-default") and optional `handle` (string).
	- Output: `mcp.ToolResult` with one `ContentItem` of `type: "text"` containing Markdown. `list` returns a Markdown list of accounts; `set-default` returns a Markdown confirmation. No structured JSON output is provided (metadata is unused).

- `login`
	- Input schema: object with optional `handle` (string), optional `password` (string), optional `prompt_id` (string), optional `port` (integer).
	- Output: returns `mcp.ToolResult` with `ContentItem`s. Behavior:
		- On success or informative failures, returns a `text` content item with Markdown. No dedicated structured success object is returned.

- `logout`
	- Input schema: object with optional `handle` (string).
	- Output: `mcp.ToolResult` with a single `text` `ContentItem` containing Markdown confirmation. No structured result.

- `profile`
	- Input schema: object with required `account` (string).
	- Output: `mcp.ToolResult` with a single `text` `ContentItem` containing a rich Markdown profile (link, DID, display name, description, avatar, stats and a raw JSON details section). No separate structured/profile JSON is returned; raw data is embedded inside Markdown.

- `search`
	- Input schema: object with required `account` (string), required `query` (string), optional `limit` (integer).
	- Output: `mcp.ToolResult` with a single `text` `ContentItem` containing Markdown-formatted search results (header, per-post sections, embeds summary, summary footer). No structured JSON list of posts or per-post metadata is returned (all is in Markdown).

### Per-tool summary (Rust server)

- `login` (handler `tools::login::handle_login`)
	- Input schema: generated from `cli::LoginCommand` (exposed via `tools/list`). Typically contains `handle`, `password`, etc.
	- Output: returns a `ToolResult` (serialized into the RPC `result`) with one or more `ContentItem`s:
		- On success, returns a `text` `ContentItem` with Markdown. No distinct structured success object is returned.

- `profile` (handler `tools::profile::handle_profile`)
	- Input schema: generated from `cli::ProfileArgs` (exposed via `tools/list`), requires `account`.
	- Output: returns `ToolResult::text(markdown)` which is serialized into the JSON-RPC `result`. The Markdown contains profile fields and a raw JSON section; there is no separate structured JSON profile in the `result`.

# Immediate action: Replace input_text pattern with standard MCP elicitation/create requests

Summary: The project was migrated to the standard MCP elicitation flow. The codebase now uses server-initiated `elicitation/create` requests (when clients advertise the capability), removes custom `prompt_id` usage from input schemas, and implements a fallback that returns a `ToolResult` with `isError: true` and clear, user-friendly guidance when clients do not support elicitation. Password-related messaging follows the required security guidance (warn against main account passwords, point to app-password creation, and prefer OAuth). Tests verify elicitation and fallback behaviors.


# Immediate action: Align Go tools list, parameters and behaviour to Rust tools

Summary: The Go MCP tools were consolidated to match the Rust tool surface. `accounts` was merged into `login` as subcommands (list/default/delete) while preserving Go's explicit `InputSchema` objects and credential storage semantics. `tools/list` now exposes `login`, `profile`, and `search` with aligned schemas. Error signaling and elicitation behavior were normalized and covered by unit and integration tests.
	- Once tests pass and `tools/list` matches Rust, remove the `accounts` tool registration and any duplicate CLI wiring.

Notes (non-functional)
 - Preserve the readable Markdown outputs; the plan only changes tool names/parameters and error signaling, not the user-facing text formatting.
 - If backward compatibility is required for external MCP clients that currently call `accounts`, provide a short-term compatibility shim: register `accounts` as an alias that forwards to `login` until clients migrate.

Acceptance criteria (final)
 - `tools/list` exposes `login`, `profile`, `search` with matching schemas.
 - Login subcommands `list|default|delete` replicate existing `accounts` behavior.

# Immediate action: Small adjustments

- Rust schema cleanup
	- Remove `prompt_id` from the `tools/list` schema (keep it CLI-only if needed). Exclude it from `LoginCommand`’s MCP-facing schema so clients see only: `command`, `handle`, `password`, `service`, `port`.
	- Ensure Rust and Go expose equivalent input schemas for `login`, `profile`, `search`.

- Rust ToolResult error signaling
	- Extend Rust `ToolResult` to include `isError: boolean` and set it for elicitation fallbacks and guidance-only failures, matching Go.
	- Until then, document interim behavior or switch these cases to JSON-RPC errors for consistency; preferred path is adding `isError` for parity.

- Tools list descriptions
	- Align Rust `login` tool description to explicitly mention subcommands “list, default, delete” (Go already does). Keep descriptions consistent across both servers.

- Tests to cover elicitation + fallbacks
	- Go: Add unit tests for `login` covering:
		- Elicitation flow when client supports it (accept / decline / cancel)
		- Fallback text with `isError: true` when client lacks elicitation support
		- Use `internal/testutil/MockServer` to simulate client behavior
	- Rust: Add tests for `login` covering:
		- Successful `elicitation/create` round trips (handle, then password)
		- Transport errors and fallback messages
		- Initialize handling of client capabilities (elicitation present vs absent)

- Documentation and compatibility
	- Update Go docs to stop recommending the legacy `accounts` tool. Replace examples with `login` subcommands (`list`, `default`, `delete`).
	- If external clients rely on `accounts`, provide a short-lived compatibility alias that forwards to `login` subcommands and document a deprecation window.

- Message copy consistency (non-functional)
	- Keep security guidance uniform across Go and Rust: don’t use main account password; link to app-password page; prefer OAuth by default.

- Minor output alignment (non-blockquote)
	- Standardize placement of the “Created” timestamp and summary footer between Go and Rust search outputs (choose one ordering and apply in both).
	- Keep the existing readable Markdown; do not introduce blockquote conventions here (tracked separately under the Markdown plan).

## Progress Log

2025-10-29
- Rust
	- ToolResult now includes optional isError (serialized as isError) and helper with_error_flag; applied to login fallback messages.
	- Excluded prompt_id from MCP-facing login schema via schemars skip on LoginCommand; tools/list schema reflects only command, handle, password, service, port.
	- Exposed build_tools_array for testing; made login fallback helpers pub(crate).
	- Added tests: schema excludes prompt_id; login fallback sets isError. All cargo tests passing.
- Go
	- Added unit test covering login fallback when client lacks elicitation support; asserts IsError and guidance copy.
	- Updated MCP docs to replace legacy accounts/logout examples with login subcommands (list/delete).
	- Confirmed tools/list descriptions mention subcommands; existing tests remain green.
- Notes
	- Elicitation round-trip tests (accept/decline/cancel) are planned; current Server signature (concrete type) complicates mocking. Will consider introducing an interface or adapter to enable mocking in unit tests.

# Simplify tool schema: The Plan

## Vision: Markdown-Structured Output for LLM Consumption

The architecture doc says it plainly: "BlueSky data is too rich and verbose for LLM." MCP tools should return **slim, natural, scannable text** — not JSON.

### Why Markdown?
- **Token efficient**: 45% fewer tokens than JSON (measured on real data)
- **LLM-native**: Models are trained on natural language, not schemas
- **Scannable**: `👍 14` beats `{"likes": {"count": 14}}`

### The Critical Problem: Content Ambiguity

**User content can contain Markdown syntax.** If a BlueSky post says:

```
Check out my project!
## Features
- Fast
- Simple
```

And our tool outputs:

```markdown
@alice/3kq6b3f1
## Features
- Fast
- Simple
👍 12
```

How does the LLM know `## Features` is user content, not a tool section header?

### The Solution: Blockquote Prefix

Prefix user content lines with `>` — the Markdown blockquote syntax. This clearly delimits content from structure:

```markdown
@alice/3kq6b3f1
> ## Features
> - Fast
> - Simple
👍 12  💬 4  2024-10-06T10:05:33Z
```

**Why this works:**
- Unambiguous: blockquotes are visually distinct
- Familiar: `>` is standard Markdown, widely understood
- Simple: prepend `> ` to each content line
- LLMs trained on this convention (email replies, forum quotes)
- Renders nicely in Markdown viewers (indented block)
- User's Markdown stays intact inside the quote

## Design Conventions

### Posts in threads, search, feed: The Standard Format

```markdown
# Thread · 8 posts

@alice/3kq8a3f1
> Hot take: Markdown > JSON for LLM tools
👍 234  ♻️ 89  💬 45  2024-10-06T10:15:33Z

└─@a/…a3f1 → @bob/3kq8b2e4
> Agree! But what about content escaping?
👍 12  2024-10-06T10:18:56Z

  └@b/…8b2e4 → @bob/3kq8b10F
> Indent user content. Simple.
👍 8  2024-10-06T10:25:33Z

└─@a/…a3f1 → @carol/3kq8d9f3
> Disagree. JSON has structure...
👍 45  💬 7  2024-10-06T12:03:41Z

  └─@c/…d9f3 → @alice/3kq8e5a2
> Because LLMs parse language, not schemas
👍 23  2024-10-06T12:30:15Z

   └─@c/…d9f3 → @alice/3kq8e5a2
> What about nested threads?
👍 5  2024-10-06T13:10:52Z
```

The thread indicators are only there on the first line of the post. That keeps the subsequent Markdown of the post content/stats untainted and valid blockquote-style.

The indentation is reflecting from which level the reply is going.

The first extra-compacted link is for disambiguation to which post this one is replying. It uses only first letter of the handle, and only last four digits of the ref key. But if that replied-to post is not in the current thread, a full @handle/refkey is used without compaction.

The content of the post is then block-quoted.

Images are converted to Markdown notation below the text, (still inside block quote or no?) with ALT text used in the square brackets as intended.

The stats and the timestamp go last.