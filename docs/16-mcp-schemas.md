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
		- For elicitation, returns an `input_text` content item with `Metadata` containing a JSON object with `prompt_id` and `field`.
		- On success or informative failures, returns a `text` content item with Markdown. No dedicated structured success object is returned; metadata is only used for elicitation prompts.

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
		- If elicitation is required, returns an `input_text` `ContentItem` and may also include a preceding `text` `ContentItem` with an explanatory message. The `input_text` metadata contains `prompt_id` and `field` as JSON.
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
 - Elicitation uses `input_text` `ContentItem` with `metadata` containing `prompt_id` and `field`.
 - No loss of credential storage semantics (keyring + file fallback preserved).

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
## Post 1 · 2h ago

Check out my project!
## Features
- Fast
- Simple

👍 12
```

How does the LLM know `## Features` is user content, not a tool section header?

### The Solution: Blockquote Prefix

Prefix user content lines with `>` — the Markdown blockquote syntax. This clearly delimits content from structure:

```markdown
## Post 1 · 2h ago · @alice

> Check out my project!
> ## Features
> - Fast
> - Simple

👍 12  ♻️ 3
```

**Why this works:**
- Unambiguous: blockquotes are visually distinct
- Familiar: `>` is standard Markdown, widely understood
- Simple: prepend `> ` to each content line
- LLMs trained on this convention (email replies, forum quotes)
- Renders nicely in Markdown viewers (indented block)
- User's Markdown stays intact inside the quote

## Design Conventions

### Emoji Vocabulary
- `👍 14` — likes
- `♻️ 7` — reposts  
- `💬 3` — reply count
- `📷` / `🎥` — media
- `✓` — success
- `⚠️` — warnings

### Structure
- **H1**: Tool result title  
- **H2**: Individual items (posts, profiles)
- **H3/H4**: Subsections (replies in threads)
- **Relative time**: "2h ago" not ISO (unless debugging)
- **Compact metrics**: One line, emoji-prefixed
- **Progressive disclosure**: `<details>` for raw data/debugging

## Example Outputs

### Profile (Enhanced)
```markdown
# @alice.bsky.social

Software engineer 🐕 dog lover | Building cool things

📊 Joined May 2023 · 1.2K followers · 843 following

<details><summary>Technical</summary>
DID: did:plc:abc123...
</details>
```

### Search (Enhanced)
```markdown
# Search: "climate" in @scientist

Found 23 posts

---

## Post 1 · 2h ago · @scientist

  New IPCC report shows **climate** crisis acceleration.
  We need action now. 🌍

👍 142  ♻️ 67  💬 23

---

## Post 2 · 1d ago · @scientist

  Thread on **climate** solutions (1/5)...

👍 89  ♻️ 34  💬 12
```

### Feed (New Tool)
```markdown
# Following · 50 posts

## @bob.dev · 3m
  Just shipped! 🚀
👍 5  ♻️ 2

## @carol · 15m  
  Thread on writing... (1/7)
👍 23  ♻️ 8  💬 4

## @dave · 1h · ↻ @original
  Amazing artwork... 📷
👍 156  ♻️ 89

→ More (cursor: abc123)
```

**Ultra-compact variant** for mass analysis:
```markdown
# Following · 50 posts

@bob · 3m — Shipped! 🚀 · 👍5
@carol · 15m — Writing thread (1/7) · 👍23 💬4
@dave · 1h · ↻@original — Artwork 📷 · 👍156
```

### Thread (New Tool)
```markdown
# Thread · 8 posts

## Original · @alice · 4h ago

  Hot take: Markdown > JSON for LLM tools

👍 234  ♻️ 89  💬 45

---

### @bob · 3h ago

  Agree! But what about content escaping?

👍 12

#### @alice · 3h ago

  Indent user content. Simple.

👍 8

---

### @carol · 2h ago

  Disagree. JSON has structure...

👍 45  💬 7
```

### Action Confirmations
```markdown
✓ Logged in as @alice.bsky.social

✓ Posted at://did:plc:.../3k...

✓ Liked 3 posts

⚠️ Delete failed: Post not found
```

## Implementation Notes

**Keep input schemas** — they're fine. Clear, typed, documented.

**Eliminate output schemas** — just return `ToolResult { content: [text] }`. No `isError`, no metadata (except elicitation).

**Token efficiency** — measured on real data: 45% reduction per post. For 50-post feeds: ~1000 tokens saved.

**Testing** — validate with actual LLMs. Can they summarize feeds? Understand threads? Parse profiles? Success = comprehension, not JSON validity.

**Future tools** need same treatment:
- `feed` — critical (most-used tool)
- `thread` — critical (conversation context)
- `post_details` — useful for engagement analysis
- `post` / `delete` / `like` — simple confirmations

This positions **autoreply** as best-in-class for LLM-native tool design.
