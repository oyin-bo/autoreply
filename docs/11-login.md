This document captures "what we want FROM the spec" for BlueSky MCP login support in the Go and Rust servers.

Goals
 - Produce a clear, implementable spec describing login flows usable by: CLI clients, interactive hosts (local GUI/terminal), HTTP-based MCP servers, and automated agents (e.g., Copilot coding agent).
 - Make implementations for `go-server/` and `rust-server/` straightforward and consistent.

High-level requirements
 - Support multiple authentication flows: OAuth (with PKCE and device-code where available), app username/password, and manual/elicited credentials for environments that cannot do OAuth.
 - Provide a CLI-first flow for headless/CI/agent scenarios (login via device code or pasted short-lived token).
 - Provide an interactive flow (browser + local redirect or embedded browser) for desktop/interactive use.
 - Expose an API for programmatic use by the MCP server to initiate login and retrieve token state.
 - Store credentials securely using OS-provided keyring/keychain where possible, with sensible fallbacks (encrypted file in user config dir, or plaintext only after explicit confirmation).
 - Refresh tokens automatically when supported by upstream; surface re-auth required events when refresh fails.

Actors and deployment models
 - CLI client (user runs `autoreply login` or similar). May open browser, show device code, or accept username/password.
 - HTTP MCP server (long-running daemon). May need server-side tokens for its account or to act on behalf of users.
 - Interactive host (desktop app or terminal UI). May run an embedded browser or open system browser and receive redirect.
 - Automated agents (e.g., Copilot coding agent). Headless: must use device-code or CLI-driven token transfer.

Authentication flows (priority order)
 1) OAuth 2.0 Authorization Code with PKCE
		- Preferred for interactive/desktop flows and servers that can open a browser and listen locally for a redirect.
		- Support loopback (http://127.0.0.1:PORT) redirect and custom URI-scheme where available.
		- Provide clear lifecycle: start auth, open browser, receive code, exchange for token(s), store tokens, schedule refresh.

 2) OAuth 2.0 Device Authorization Grant (device code)
		- Preferred for headless or limited-input environments (CI, remote agents, terminals without browser access).
		- Show user a short URL and code; poll token endpoint; store tokens on success.

 3) OAuth 2.0 Authorization Code via out-of-band manual copy
		- If automatic redirect isn't possible, allow user to paste the authorization code or final redirected URL back into the CLI.

 4) Username/password (resource owner credentials) for legacy or API-only providers
		- Only when the provider supports ROPC (rare) or a direct app username/password API.
		- Treat as least-preferred and require explicit admin consent and clear UX warnings.

 5) Manual token entry / elicitation
		- Provide a fallback where the user pastes an access token (and optionally refresh token) into a secure prompt. Useful when provider issues tokens via other tooling.

Credential storage
 - Primary: OS keychain / keyring
	 - Go: use existing `zalando/go-keyring` or `99designs/keyring` (or `github.com/zalando/go-keyring`) interfaces; support Linux Secret Service, macOS Keychain, Windows Credential Vault.
	 - Rust: use `keyring` crate or `secret-service` bindings; support same OS stores.
 - Secondary fallback: user-scoped encrypted file in platform config dir
	 - Format: JSON containing token metadata, encrypted with a key derived from OS secrets where possible (or a user-provided passphrase). File path: $XDG_CONFIG_HOME/autoreply/credentials.json (Linux), %APPDATA%\autoreply\credentials.json (Windows), ~/Library/Application Support/autoreply/credentials.json (macOS).
 - Tertiary fallback: plaintext file only when user explicitly forces it; warn strongly.
 - Storage shape (minimal):
	 - account_id: string (provider account handle)
	 - provider: string (e.g., bluesky)
	 - access_token: string
	 - refresh_token: string | null
	 - expires_at: RFC3339 timestamp | null
	 - scopes: [string]
	 - created_at: RFC3339
	 - meta: { client_id?: string, client_info?: {} }

Security and privacy constraints
 - Minimise token exposure in logs and process environment.
 - Use secure HTTP (TLS) and validate certificates for all token/authorize endpoints.
 - When storing tokens, ensure appropriate filesystem permissions (600/owner-only) for fallback files.
 - If using an embedded browser, isolate the auth session (use ephemeral browser profile if possible).

CLI UX / UX requirements
 - `autoreply login <some parameters>`
	 - Default auto method attempts browser PKCE then device-code then manual.
 - Clear, short interactive prompts for device-code URLs and copy/paste codes.
 - Helpful output on success: account identifier, token expiry, storage location used.
 - Clear error messages and remediation steps for common failures (network, client_id not authorized, code expired).

Server / API contract for MCP
 - Expose simple APIs (HTTP or local IPC) for initiating login and retrieving active credentials.
 - Minimal operations:
	 - POST /login/initiate { provider, method, client_info? } -> { session_id, url? , user_code? , poll_interval? }
	 - POST /login/complete { session_id, pasted_code? } -> { success, account_id }
	 - GET /login/status?session_id=... -> { state: pending|complete|error, expiry?, account? }
	 - GET /credentials?account_id=... -> { token shape above }
 - Tokens retrieved by the MCP server should be returned only to authenticated local callers or via configured policy (avoid leaking to remote callers by default).

Token lifecycle and refresh
 - When provider issues refresh tokens, the client should store them and use them to refresh access tokens transparently.
 - On refresh failure (invalid_grant, revoked), mark credentials as stale and require re-login; notify operator if running a server.
 - Implement token rotation handling, if provider supports.

Error modes and edge cases
 - Network failures during exchange or polling: retry with exponential backoff and surface user-visible hints.
 - Browser blocked or popup prevented: fall back to device-code or manual copy.
 - Provider does not support refresh tokens: store expiry and prompt re-login proactively before expiry (configurable threshold, e.g., 5 minutes).
 - Multiple accounts: allow multiple stored credentials keyed by account_id and provider; provide `autoreply account list` and `autoreply logout --account`.

Acceptance criteria (for this spec stage)
 - A clear document describing supported flows, storage options, APIs, and UX expectations (this file).
 - Language-agnostic token shape and REST-like API contract for initiating and completing login.
 - List of recommended libraries for Go and Rust for keyring and OAuth flows.

Next steps
 - Research details of BlueSky OAuth
 - Research concrete libraries and example implementations for Rust and Go: OAuth + PKCE + device-code + keychain/keyring crates/packages.
 - Produce server/client API details and example sequences for each flow (CLI, interactive, headless agent, and HTTP MCP server).
 - Implement a PoC in `go-server` and `rust-server` following the spec.
