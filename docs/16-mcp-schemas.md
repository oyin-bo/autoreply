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


