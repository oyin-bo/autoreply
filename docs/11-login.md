# BlueSky MCP Login: Spec Wishlist

This document captures "what we want FROM the spec" for BlueSky MCP login support in the Go and Rust servers. It is a high-level wishlist to guide the research and specification phase.

## Goals

-   Produce a clear, implementable spec describing login flows usable by various clients.
-   Ensure implementations for `go-server/` and `rust-server/` are straightforward and consistent.

## High-Level Requirements

-   **Multiple Authentication Flows**: Support modern authentication methods like OAuth 2.0 (including PKCE and device code flows), but also allow for fallbacks like manual token entry for environments that have limitations.
-   **Multiple Concurrent Logins**: The MCP server must be able to manage authenticated sessions for multiple users and/or providers simultaneously.
-   **Varied Client Support**: Provide flows suitable for:
    -   Headless CLI (for CI/CD, agents).
    -   Interactive desktop use (with a browser).
-   **Secure Credential Storage**: Use OS-provided secure storage (like macOS Keychain, Windows Credential Manager, or Linux Secret Service) as the primary mechanism. Provide sensible and secure fallbacks.
-   **Programmatic API**: Expose a simple API for the MCP server to initiate login flows and retrieve credentials.
-   **Automatic Token Refresh**: Where possible, automatically refresh expired tokens without requiring user interaction.

## Actors and Deployment Models

-   **CLI Client**: A user running a command like `autoreply login`.
-   **HTTP MCP Server**: A long-running daemon that may need to act on behalf of one or more users.
-   **Interactive Host**: A desktop app or terminal UI that can guide a user through an interactive login.
-   **Automated Agents**: Headless tools (like a GitHub Copilot agent) that need to authenticate without direct user interaction.

## Desired Authentication Flows

The spec should consider a prioritized list of authentication flows to accommodate different client capabilities.

1.  **OAuth 2.0 with PKCE**: The preferred, most secure method for interactive clients that can open a browser.
2.  **OAuth 2.0 Device Authorization Grant**: The best option for headless or limited-input clients (e.g., CLIs on a remote server).
3.  **Manual Out-of-Band Flows**: As a fallback, allow users to manually paste an authorization code or token into the client.
4.  **Username/Password**: To be considered only as a last resort if a provider offers no other method, with strong warnings to the user.

## Credential Storage Strategy

-   **Primary**: Integrate with OS-native keychains. The research phase should identify the best cross-platform libraries in Go and Rust for this.
-   **Fallback**: If an OS keychain is unavailable, a secure, user-scoped, (encrypted?) file should be used.
-   **Plaintext**: Should be wary and enabled if a other options are impossible, and user consents.

The stored data should contain the essential information: an account identifier, the provider, the necessary tokens, and expiry information.

## Security and Privacy

-   Minimize token exposure in logs and the process environment.
-   Use secure transport (TLS) for all authentication-related network calls.
-   Ensure any fallback storage files have strict, user-only permissions.

## User Experience (UX)

-   The CLI should be intuitive, e.g., `autoreply login`.
-   Prompts during interactive flows should be clear and concise.
-   Success and error messages should be helpful and guide the user on what to do next.

## Server API Contract for MCP

The MCP server needs a simple API to manage authentication. This API should allow a client to:

-   Initiate a login flow for a specific provider.
-   Check the status of a pending login.
-   Retrieve credentials for an authenticated account.

Should login be a separate tool in MCP server? Research need to consider and offer a recommendation grounded in pragmatic aspects.

## Token Lifecycle and Error Handling

-   The system should handle token refresh automatically.
-   It must gracefully handle refresh failures by notifying the user or operator that re-authentication is needed.
-   It should provide clear guidance for common errors like network failures or misconfigurations.
-   The system must support managing multiple accounts, allowing users to list, use, and log out from specific accounts.

## Next Steps

-   ✅ Research the specifics of BlueSky's OAuth or other authentication mechanisms.
-   ✅ Identify and evaluate concrete libraries for both Go and Rust to handle OAuth flows and OS credential storage.
-   ✅ Draft a detailed technical spec based on this wishlist and the research findings.
-   Create a Proof of Concept (PoC) in `go-server` and `rust-server`.

## Implementation Plan

**See:** [12-auth-implementation-plan.md](./12-auth-implementation-plan.md) for the comprehensive implementation plan covering:
- AT Protocol OAuth mechanisms (PKCE, DPoP, Device Flow)
- Library recommendations for Rust (keyring-rs, atproto-oauth) and Go (go-keyring, indigo)
- Multi-account architecture with concurrent login support
- MCP tool specifications and CLI user experience
- Security considerations and testing strategy
- 9-week implementation roadmap

**Quick Reference:** [12-auth-quick-ref.md](./12-auth-quick-ref.md) for at-a-glance decisions and development guide.

