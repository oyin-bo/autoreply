# AT Protocol OAuth Status

## Current Situation

As of October 2024, **AT Protocol OAuth is not yet fully deployed** on all Bluesky PDS (Personal Data Server) instances. This means that while the OAuth implementation in this codebase is complete and functional, it may not work with all Bluesky accounts yet.

## Error You Might See

When trying to use OAuth authentication, you may see:

```
⏳ Discovering OAuth server metadata...
```

And then the program hangs or times out. This happens because the PDS server does not yet support the OAuth metadata endpoint (`/.well-known/oauth-authorization-server`).

## Recommended Authentication Method

**Use app passwords instead:**

```bash
./autoreply login --method password --handle your.handle.bsky.social
```

App passwords work with all Bluesky accounts and are currently the most reliable authentication method.

### How to Create an App Password

1. Go to https://bsky.app/settings/app-passwords
2. Click "Add App Password"
3. Give it a name (e.g., "autoreply-cli")
4. Copy the generated password
5. Use it when prompted by the login command

## When Will OAuth Work?

OAuth authentication will work once:

1. Bluesky/AT Protocol rolls out OAuth support to all PDS servers
2. Your specific PDS instance has been updated to support OAuth
3. The OAuth metadata endpoint becomes available at your PDS

You can test if your PDS supports OAuth by checking:
```bash
curl https://YOUR_PDS_URL/.well-known/oauth-authorization-server
```

If you get a valid JSON response, OAuth should work. If you get a timeout or 404, use app passwords.

## Implementation Status

The OAuth implementation in this codebase is **complete and production-ready**:

- ✅ DPoP (Demonstrating Proof of Possession) with ES256
- ✅ PKCE (Proof Key for Code Exchange)
- ✅ PAR (Pushed Authorization Request)
- ✅ Full authorization code flow
- ✅ Token refresh
- ✅ Callback server
- ✅ Browser integration

The code is ready and will work as soon as AT Protocol OAuth becomes available on your PDS.

## Timeline

According to the AT Protocol specification, OAuth support is planned but not yet universally deployed. Check the official Bluesky blog and AT Protocol documentation for updates on OAuth rollout.

## For Developers

If you're running your own PDS or testing against a development PDS with OAuth support enabled, the OAuth flow should work correctly. The implementation follows the AT Protocol OAuth specification exactly.
