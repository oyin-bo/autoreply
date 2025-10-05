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

- `search` (handler `tools::search::handle_search`)
	- Input schema: generated from `cli::SearchArgs` (exposed via `tools/list`) and includes `account`, `query`, optional `limit`.
	- Output: returns `ToolResult::text(markdown)` serialized into the RPC `result`. The Markdown contains per-post sections and summary; there is no structured JSON array of posts or per-post metadata in the `result`.

### Common observations (both servers)

- Input schemas are advertised via `tools/list`/`initialize` (Go uses explicit `InputSchema` structs; Rust uses schemars-generated schemas from CLI argument structs).
- Tools prefer returning human-readable Markdown inside a single `ContentItem` (type `text`).
- Elicitation/interactive prompts use `ContentItem` with `type: "input_text"` and include machine-readable `metadata` containing `prompt_id` and `field` (both servers use this pattern).
- No tool currently returns fully structured JSON as the primary `result` (they embed raw JSON only inside Markdown or use `metadata` for elicitation). This limits programmatic consumption of per-item data by MCP clients.

# Immediate action: Replace input_text pattern with standard MCP elicitation/create requests

## Research findings: MCP Standard Elicitation (Protocol Version 2025-06-18)

The official MCP specification defines elicitation as a **client feature** that allows **servers to request user input** via the `elicitation/create` method. This is fundamentally different from the current implementation.

### Official MCP Elicitation Flow

**Current (WRONG) implementation:**
1. Client calls `tools/call` → Server returns `input_text` ContentItem in tool result
2. Client somehow correlates this with original request using custom `prompt_id`
3. Client calls tool again with the elicited value

**Correct MCP Standard implementation:**
1. Client calls `tools/call` (JSON-RPC id=1)
2. Server **makes a NEW REQUEST** to client: `elicitation/create` (JSON-RPC id=2)
3. Client responds to elicitation (correlates via standard JSON-RPC id=2)
4. Server completes tool execution and returns result (correlates with original id=1)

### JSON-RPC Shape (Official MCP Specification)

**Server Request (Server → Client):**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "elicitation/create",
  "params": {
    "message": "Please provide your BlueSky handle",
    "requestedSchema": {
      "type": "object",
      "properties": {
        "handle": {
          "type": "string",
          "description": "Your BlueSky handle (e.g., user.bsky.social)"
        }
      },
      "required": ["handle"]
    }
  }
}
```

**Client Response (Client → Server):**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "action": "accept",
    "content": {
      "handle": "user.bsky.social"
    }
  }
}
```

### Key Differences from Current Implementation

| Aspect | Current (Wrong) | MCP Standard (Correct) |
|--------|----------------|------------------------|
| **Direction** | Tool result content | Server-initiated RPC request |
| **Method** | `input_text` ContentItem | `elicitation/create` method |
| **Correlation** | Custom `prompt_id` token | Standard JSON-RPC `id` field |
| **Response** | Tool re-invocation | Direct RPC response |
| **Actions** | N/A | `accept`, `decline`, `cancel` |

### Critical Implications

1. **`input_text` is NOT part of MCP standard** - it's a custom content type we invented
2. **`prompt_id` is unnecessary** - JSON-RPC `id` provides correlation
3. **Nested RPC pattern** - server becomes client temporarily to request input
4. **Client capability required** - clients must declare `"elicitation": {}` capability during initialization
5. **Security constraint** - spec says "Servers MUST NOT use elicitation to request sensitive information" (though passwords are commonly needed)

### Fallback Policy for Clients Without Elicitation

**MCP hosts and clients that do not support `elicitation/create` are fully supported.**

When a tool requires user input (e.g., missing credentials) but the client/host lacks elicitation capability:

1. **Return a tool-level error:** The server must return a `ToolResult` with `isError: true` (not a JSON-RPC error)
2. **Include client/host identification:** The error message must name the specific client or host (as declared during initialization)
3. **Provide clear alternatives:** The message must guide users to either:
   - Use OAuth authentication (preferred), or
   - Supply credentials up-front as tool parameters
4. **Password security requirements:** When the missing input is a password, the message must:
   - Explicitly warn against using main BlueSky account passwords
   - Direct users to create app passwords at the BlueSky app-passwords settings page
   - Offer OAuth as the strongly preferred alternative
   - Use clear, non-technical language suitable for end users

This policy ensures graceful degradation while maintaining security and user guidance.

## Technical migration plan (code-only, tests, refactor)

**Goal:** Replace custom `input_text` ContentItem pattern with standard MCP `elicitation/create` requests. Remove all `prompt_id` usage. Implement proper nested RPC pattern where server requests user input from client.

### Architecture change: Server must send RPC requests to client

Current servers are **request handlers only**. To use standard elicitation, servers must be able to **send requests to the client** (nested RPC). This requires:

1. **Bidirectional communication channel** - both parties can send requests
2. **Client capability negotiation** - client must declare `"elicitation": {}` during initialization
3. **Asynchronous tool execution** - tool handlers must support async operations to wait for elicitation responses
4. **Fallback handling** - when client lacks elicitation, apply the fallback policy above (tool-level `isError` response)

### Key implementation requirements

**Remove custom patterns:**
- Remove `prompt_id` from all tool input schemas
- Replace `input_text` ContentItems with standard `elicitation/create` requests
- Use JSON-RPC `id` for correlation (standard mechanism)

**Add elicitation support:**
- Implement bidirectional RPC infrastructure (server can send requests to client)
- Add elicitation request/response handling in MCP layer
- Update tool handlers to use `elicitation/create` when client supports it
- Handle all response actions: `accept`, `decline`, `cancel`

**Fallback for clients without elicitation:**
- Check client capabilities during initialization
- When tool needs input but client lacks elicitation, return `ToolResult` with `isError: true`
- Error message must name the client/host and guide users to OAuth or upfront credentials

**Password security requirements (MANDATORY):**

When eliciting or requesting passwords (whether via `elicitation/create` or in error messages for non-elicitation clients):

1. **Explicit warnings:** Messages must clearly state not to use main BlueSky account passwords
2. **App password guidance:** Messages must direct users to the BlueSky app-passwords creation page
3. **OAuth preference:** Messages must present OAuth as the strongly recommended alternative
4. **User-friendly language:** Use clear, non-technical wording appropriate for end users
5. **Cancel handling:** When user cancels password elicitation, provide OAuth guidance

These requirements apply to:
- `elicitation/create` message text when requesting passwords
- Tool-level error messages when elicitation is unavailable
- CLI prompts and interactive flows
- Documentation and examples
### Acceptance criteria

**Core functionality:**
- ✅ All `prompt_id` usages removed from input schemas
- ✅ No `input_text` ContentItems in tool results
- ✅ Standard `elicitation/create` used when client supports it
- ✅ Clients without elicitation receive appropriate `isError` tool result with guidance
- ✅ Client capabilities checked during initialization

**Password security (MANDATORY):**
- ✅ Password elicitation messages explicitly warn against using main account passwords
- ✅ Password messages include direct link/reference to app password creation
- ✅ Password messages present OAuth as the preferred alternative
- ✅ User cancellation of password prompts provides OAuth guidance
- ✅ All password-related messaging uses clear, user-friendly language

**Testing:**
- ✅ Tests verify elicitation flow (accept/decline/cancel actions)
- ✅ Tests verify fallback behavior for clients without elicitation
- ✅ Tests verify all password security messaging requirements


# Immediate action: Align Go tools list, parameters and behaviour to Rust tools

Assessment — Go-only features worth preserving

- Explicit `InputSchema` implementations (Go): Go tools declare concrete JSON schema objects per-tool. This is useful for clarity and for MCP clients that consume the schema directly without relying on generated schemas. Rust uses schemars on CLI structs; keep Go schemas (or migrate them to equivalent schemars-like definitions) so clients keep the same contract.
- `ToolResult.IsError` flag (Go): Go occasionally returns a `ToolResult` with `IsError: true` instead of emitting a JSON-RPC error. This is a non-standard but intentional pattern for returning a user-visible message while avoiding an RPC-level error. Preserve the intent (surface non-fatal failures clearly) but convert to one consistent mechanism (recommend RPC error codes rather than mixing flags).
- `accounts` tool (Go) with multi-action `action` param: provides a small CLI-like management surface (list, set-default) separate from `login`. Rust folds these into `login` subcommands. The functionality is useful; migrate behaviour under `login` as subcommands to match Rust, but preserve the user-facing outputs and the schema semantics.
- Prompt correlation support (`prompt_id`) in Go `login`: Rust already supports elicitation via `prompt_id` too, but ensure Go's hex-based prompt id generation and the ability to accept a client-supplied `prompt_id` are preserved.
- Keyring + encrypted-file fallback semantics: both languages implement equivalent secure storage. Keep parity; preserve any platform-specific keyring configuration from Go only if it offers additional platform behavior not present in Rust (current inspection shows rough parity, so no further action required here).

Migration plan — consolidate Go tools to match Rust tool set while retaining logic

Goal: Make Go MCP tools present the same tool names and parameter model as Rust (single `login` tool with list/default/delete subcommands, `profile`, `search`) while preserving Go implementation logic, explicit schemas, and elicitation behavior.

Steps (ordered, minimal disruption):

1) Create a compatibility checklist (quick tests)
	- Define acceptance criteria: tools/list must report the same tool names and input schemas as Rust; `login` must accept subcommands equivalent to Rust (list/default/delete/login); elicitation must use `input_text` with `metadata` containing `prompt_id` and `field`.

2) Replace `accounts` tool surface with `login` subcommands
	- Keep the `accounts` implementation code, but move its logic under `login` handling (Login subcommands: `list`, `default`, `delete`).
	- Implement a `LoginCommand`-style input schema for Go: either (A) keep Go's explicit `InputSchema` but add a `command` field with allowed enum values, or (B) model subcommands as separate properties — keep the schema explicit and document the mapping.
	- Update `go-server/internal/mcp` tool registry so `login` is the registered tool name; remove `accounts` registration.

3) Preserve elicitation and `prompt_id` behavior
	- Ensure `login` accepts an optional `prompt_id` parameter and returns `ContentItem` with `Type: "input_text"` and `Metadata` containing `{ "prompt_id": <id>, "field": <name> }` exactly as Rust does.
	- Preserve client-supplied `prompt_id` semantics (if provided, reuse; otherwise generate one). Keep Go's generator if desired, but document the format difference vs Rust (hex vs alphanumeric) or switch to Rust-style 16-char alphanumeric for uniformity.

4) Normalize error signaling (preserve intent)
	- Replace ad-hoc `ToolResult.IsError` use with consistent behavior: prefer returning a JSON-RPC error for true failures, and use a non-error `ToolResult` only for elicitation or user prompts. If compatibility is required for existing clients that check `isError`, preserve `isError` but add a comment and plan to deprecate it in a follow-up.

5) Keep explicit InputSchemas (but align shapes)
	- Where Rust exposes schemars-generated schemas, translate Go's explicit schemas to match Rust's `cli` shapes (field names and required fields). Keep Go schemas as the authoritative shape for the Go MCP server; if desired, add a script or tests that compare the generated Rust schema to the Go schema to detect drift.

6) Consolidate `logout` behavior under `login` subcommands if desired
	- Rust exposes account deletion as a login subcommand; decide to either keep `logout` (standalone) for compatibility or handle `delete` via `login` subcommand. Prefer the Rust style (single `login` tool with subcommands) for consistency.

7) Update `tools/list` and `initialize` outputs
	- Ensure `tools/list` returns the same tool names and `inputSchema` shapes as Rust: `login` (with subcommands modeled in schema), `profile`, `search`.

8) Tests and verification
	- Add unit tests that call `tools/list` and assert tool names and schema shapes.
	- Add integration tests that perform `tools/call` for each subcommand: `login:list`, `login:default`, `login:delete`, `login` (normal login), `profile`, `search` and verify the returned `ContentItem` types and `metadata` for elicitation.

9) Rollout strategy
	- Implement the changes on a feature branch; keep the old `accounts` tool code unremoved until integration tests pass.
	- Once tests pass and `tools/list` matches Rust, remove the `accounts` tool registration and any duplicate CLI wiring.

Notes (non-functional)
 - Preserve the readable Markdown outputs; the plan only changes tool names/parameters and error signaling, not the user-facing text formatting.
 - If backward compatibility is required for external MCP clients that currently call `accounts`, provide a short-term compatibility shim: register `accounts` as an alias that forwards to `login` until clients migrate.

Acceptance criteria (final)
 - `tools/list` exposes `login`, `profile`, `search` with matching schemas.
 - Login subcommands `list|default|delete` replicate existing `accounts` behavior.
 - Elicitation uses `input_text` `ContentItem` with `metadata` containing `prompt_id` and `field`.
 - No loss of credential storage semantics (keyring + file fallback preserved).


