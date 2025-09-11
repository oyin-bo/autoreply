# Migration plan: direct atcute replacement (single-file, no shims)

## Objective

- Replace direct uses of `AtpAgent`/`@atproto/api` with the atcute client library by performing in-place, direct substitutions in `index.js` for each tool, one at a time. No intermediate shims or adapter layers will be added. Keep single-file constraint and manual testing: no unit tests and no feature flags.

## Preliminary checks (before touching code)

Summary of findings (quick facts)

- The project already lists `@atcute/client` in `package.json` (version found on npm: 4.0.3). Prefer pinning to the exact published version you intend to use. The package on npm is MIT-licensed.
- The atcute monorepo on GitHub is well-populated and licensed permissively (0BSD for the monorepo); key client functionality lives in `@atcute/client` and schema/definition packages such as `@atcute/bluesky` and `@atcute/atproto` provide the lexicons.
- `@atcute/client` main primitives discovered from README and package dist:
  - Client class: used to perform RPC-style calls (rpc.get / rpc.post semantics).  
  - simpleFetchHandler({ service }) helper to create a fetch-based transport configured with the target service URL.  
  - CredentialManager({ service }) for authenticated flows (acts as a handler for Client and exposes manager.login and manager.session).  
  - ok(...) helper to unwrap optimistic results and throw on non-ok.

Concrete examples and mapping patterns

- Public/anonymous client (equivalent of incognito AtpAgent):
  - Atcute pattern: const handler = simpleFetchHandler({ service: 'https://public.api.bsky.app' }); const rpc = new Client({ handler });
  - Use rpc.get('app.bsky.actor.getProfile', { params: { actor: '...' } }) for read endpoints.

- Authenticated client (equivalent of AtpAgent + login):
  - Atcute pattern: const manager = new CredentialManager({ service: 'https://bsky.social' }); const rpc = new Client({ handler: manager }); await manager.login({ identifier, password }); manager.session contains tokens; rpc calls are authenticated.
  - manager.session example from README: { refreshJwt: '...', ... }.

- RPC patterns:
  - rpc.get('app.bsky.actor.getProfile', { params: { actor } }) returns { ok, data } or use await ok(rpc.get(...)) to get data directly.
  - Use rpc.get for read procedures. For write/procedure calls the README shows similar usage via rpc.post or rpc.call patterns (the Client exposes methods to invoke Lexicon procedures; exact call names are `app.bsky.*` and `com.atproto.*` lexicon procedure ids).

Mapping of AtpAgent usages in index.js → atcute equivalents (what to change directly)

- Agent creation:
  - Old: new AtpAgent({ service: 'https://public.api.bsky.app' })
  - New: const handler = simpleFetchHandler({ service: 'https://public.api.bsky.app' }); const rpc = new Client({ handler });

- Authenticated login:
  - Old: rpc.login({ identifier, password })
  - New: const manager = new CredentialManager({ service: 'https://bsky.social' }); const rpc = new Client({ handler: manager }); await manager.login({ identifier, password });
  - After this, use rpc for authenticated calls; persist credential tokens with existing keytar logic if desired (or continue to store raw password as before).

- getProfile / getFollowers / getFollows:
  - Old convenience methods: agent.getProfile({ actor }), agent.getFollowers({ actor, cursor }), agent.getFollows({ actor, cursor })
  - New RPC calls: rpc.get('app.bsky.actor.getProfile', { params: { actor } }); rpc.get('app.bsky.graph.getFollowers', { params: { actor, cursor } }) or the corresponding `app.bsky.*` procedure names found in `@atcute/bluesky` definitions. Use the `ok(...)` helper to unwrap `data` when you want to throw on non-ok results.

- feed retrieval / timeline / search:
  - Old: agent.app.bsky.feed.getFeed({ feed, cursor, limit }) or agent.getTimeline()
  - New: rpc.get('app.bsky.feed.getFeed', { params: { feed, cursor, limit } }); search: rpc.get('app.bsky.feed.searchPosts', { params: { q, cursor, limit } })

- getPost / getPostThread:
  - Old: agent.getPost({ repo, rkey }), agent.app.bsky.feed.getPostThread({ uri })
  - New: rpc.get('app.bsky.feed.getPostThread', { params: { uri } }) and rpc.get('app.bsky.feed.getPost', { params: { uri } }) or `com.atproto.repo.getRecord` variants — confirm exact procedure name in `@atcute/bluesky` or `@atcute/atproto` definitions.

- Posting / replies:
  - Old: agent.post({ text, reply }) returning posted.uri and posted.cid
  - New: likely rpc.post('app.bsky.feed.post', { data: { text, reply } }) or the appropriate write procedure. Response should include `uri` and `cid` but confirm the exact shape and property names from the definition package.

- Like / repost / delete:
  - Old: agent.like(uri, cid), agent.repost(uri, cid), agent.deletePost(uri)
  - New: these might map to write procedures such as rpc.post('app.bsky.reaction.create' or 'app.bsky.repost.create') or `com.atproto.repo.deleteRecord` — check `@atcute/bluesky` and `@atcute/atproto` definitions for exact names.

Practical verification steps performed by me (results)

- Confirmed `@atcute/client` exists on npm (v4.0.3) and README contains example usage showing: import { Client, CredentialManager, ok, simpleFetchHandler } from '@atcute/client'; rpc.get(...) pattern; CredentialManager for authenticated handler.
- Confirmed `@atcute/bluesky` and `@atcute/atproto` exist as definition packages and must be installed to obtain typed lexicon names; unpkg shows these packages available.
- Confirmed example endpoints: 'https://public.api.bsky.app' for public reads and 'https://bsky.social' for authenticated writes in examples.

Immediate actions you must perform locally before editing code

1. Install and pin the atcute packages used in the project (already present in package.json, but ensure installed):
   - npm install @atcute/client @atcute/bluesky @atcute/atproto
   - Verify versions in package-lock or pnpm lock match the intended release.

2. Inspect the installed `@atcute/bluesky` and `@atcute/atproto` packages (in node_modules or via unpkg) to get the exact RPC procedure names for write actions you need (post, like/repost, delete, getPost). Example: search for 'app.bsky.feed.post', 'app.bsky.feed.getFeed', 'app.bsky.repost' etc.

3. Try a small REPL experiment to confirm login and a simple read call:
   - Node REPL or small script:
     - const { Client, CredentialManager, simpleFetchHandler, ok } = require('@atcute/client');
     - const handler = simpleFetchHandler({ service: 'https://public.api.bsky.app' }); const rpc = new Client({ handler });
     - const { ok, data } = await rpc.get('app.bsky.actor.getProfile', { params: { actor: 'bsky.app' } }); console.log(ok, data);
   - For authenticated: const manager = new CredentialManager({ service: 'https://bsky.social' }); const rpcAuth = new Client({ handler: manager }); await manager.login({ identifier: handle, password }); then call rpcAuth.get('com.atproto.identity.resolveHandle', { params: { handle } }) and confirm `data.did`.

4. Confirm what RPC method to call for posting and actions by grepping the installed definition files or reading the package `@atcute/bluesky` content.

Potential gotchas you should be aware of

- Procedure names and parameter shapes can differ from what AtpAgent convenience methods provided. Expect to adapt parameter naming at each call site.  
- The atcute client returns results via { ok, data } tuples for rpc.get; using `ok(rpc.get(...))` unwraps and throws on non-ok — choose which pattern to use consistently.  
- CredentialManager stores session objects with refresh tokens; if you rely on storing raw passwords in keytar currently, decide whether to persist raw password (current approach) or manager.session (safer) — migrating storage format is optional but should be decided before wide migration.
- Some write actions may require calling `com.atproto.repo.createRecord` or other lower-level procedures instead of a convenient `agent.like` wrapper; verify exact write procedure names in `@atcute` definitions.

Conclusion of preliminary checks

- atcute provides all the low-level primitives required to implement everything currently used from AtpAgent: anonymous/public reads via simpleFetchHandler + Client, authenticated flows via CredentialManager, and RPC-style get/post methods for lexicon procedures.
- The immediate next step (as your plan says) is to validate authentication in-place by editing `clientLogin` in `index.js` to call `@atcute/client`'s CredentialManager and Client per the README, then manually test login and an authenticated read. After that, migrate the next command (feed) and validate.

## High-level workflow (per-command, direct replacement)

Overall approach for each tool/command (repeat for login, feed, profile, search, thread, post, like, repost, delete):

1. Locate all places in `index.js` where the old client is constructed or its method is invoked for the target command. Typical patterns to change:
   - new AtpAgent({ service: ... })
   - agent.login({ identifier, password })
   - agent.app.bsky.feed.getFeed(...)
   - agent.getProfile/getFollowers/getFollows
   - agent.getPost / agent.app.bsky.feed.getPostThread
   - agent.post / agent.like / agent.repost / agent.deletePost
   - agent.resolveHandle

2. Consult atcute API to determine the direct replacement call and its expected parameters and return shape.

3. Edit `index.js` to replace the call(s) for this command only. Where the atcute method returns a different shape, update the calling code at the call site to adapt to the new return shape — but only for this command’s code paths (do not refactor unrelated code). Keep changes minimal and focused.

4. Manually run and validate the command in TTY (interactive) mode and MCP stdio mode as applicable. Fix errors until the command behaves as expected.

6. Proceed to the next command.

## Priority order

1. **login / clientLogin** — REQUIRED first step
   - Rationale: authentication is the dependency for several other tools (post, like, repost, delete). Validate that atcute login works and that credentials are stored via the existing keytar fallback logic.
   - Edits: replace AtpAgent constructor and rpc.login call inside `clientLogin`. Ensure `clientLogin` continues to return an object that the rest of the code treats as an agent. Update `clientLoginOrFallback` to work with atcute sessions if necessary.
   - Manual tests: use `node index.js login` or the interactive flow (localLogin) to store credentials. Then call an authenticated read (e.g., `feed` with login) to verify authenticated behavior.

2. **feed (read-only)**
   - Rationale: exercises incognito/public endpoints and basic feed shaping. Lower risk than write actions.
   - Edits: replace any `app.bsky.feed.getFeed` or `agent.getTimeline` calls with atcute equivalents. Adjust call-sites to new return shapes.
   - Manual tests: run `node index.js feed` and verify posts list and structuredContent format.

3. **profile**
   - Rationale: tests getProfile/getFollowers/getFollows calls and pagination handling.
   - Edits and tests: replace calls and validate follower/following cursors and returned handle/metadata.

4. **search and thread**
   - Rationale: search posts and thread retrieval use different endpoint shapes and nested responses.
   - Edits: swap calls and validate output of `search` and `thread` tools.

5. **post, like, repost, delete (write actions)**
   - Rationale: higher risk and require confirmed authentication.
   - Edits: replace post creation and action methods, ensure returned `uri` and `cid` are preserved or calling code is adjusted accordingly.
   - Manual tests: use a disposable/test account. Post a test message, then like/repost/delete, verifying expected responses and that objects returned have expected properties.

## Exact RPC procedure names — confirmed vs. replaced TO-VERIFY

I completed the lexicon research against the official atproto lexicons (bluesky-social/atproto) and confirmed authoritative procedure IDs and record NSIDs. The previous "TO-VERIFY" placeholder is replaced below with the verified mapping and concrete atcute call snippets you can use directly in `index.js`.

Confirmed authoritative facts

- Record NSIDs (record types):
  - `app.bsky.feed.post` — post record schema (see `lexicons/app/bsky/feed/post.json`).
  - `app.bsky.feed.like` — like record schema (see `lexicons/app/bsky/feed/like.json`).
  - `app.bsky.feed.repost` — repost record schema (see `lexicons/app/bsky/feed/repost.json`).

- Write / delete procedures (authoritative, server-implemented):
  - `com.atproto.repo.createRecord` — create a new record in a repo. Input requires: { repo, collection, record, ... } and returns { uri, cid }.
  - `com.atproto.repo.deleteRecord` — delete a record. Input requires: { repo, collection, rkey } (or similar) and returns status.

- Read / utility procedures (confirmed):
  - `app.bsky.feed.getFeed`, `app.bsky.feed.getPostThread`, `app.bsky.feed.getPosts`, `app.bsky.feed.getLikes`, `app.bsky.feed.getRepostedBy`, `app.bsky.feed.searchPosts`, `app.bsky.actor.getProfile`, `app.bsky.graph.getFollowers`, `app.bsky.graph.getFollows`, `com.atproto.identity.resolveHandle`, `com.atproto.repo.getRecord`.

What this means for direct replacement

- For creating posts, likes, and reposts, call the server write procedure `com.atproto.repo.createRecord` with `collection` set to the appropriate record NSID (`app.bsky.feed.post`, `app.bsky.feed.like`, `app.bsky.feed.repost`). The lexicon record definitions describe the exact fields to include in the `record` payload (e.g., `$type`, `text`, `subject`, `createdAt`, etc.).

- For deleting, call `com.atproto.repo.deleteRecord` with the repo DID, collection NSID, and the record key (`rkey`) or equivalent.

Concrete atcute call snippets (drop into `index.js` when migrating each command)

- Create a post:

```js
// assumes `rpc` is a Client instance whose handler is a CredentialManager (authenticated)
const created = await ok(rpc.post('com.atproto.repo.createRecord', {
  data: {
    repo: myDid,
    collection: 'app.bsky.feed.post',
    record: {
      $type: 'app.bsky.feed.post',
      text: text,
      createdAt: new Date().toISOString(),
      // include reply/embed fields as needed per lexicon
    }
  }
}));
// created.uri and created.cid are returned
```

- Create a like (reaction):

```js
const like = await ok(rpc.post('com.atproto.repo.createRecord', {
  data: {
    repo: myDid,
    collection: 'app.bsky.feed.like',
    record: {
      $type: 'app.bsky.feed.like',
      subject: { uri: targetUri },
      createdAt: new Date().toISOString()
    }
  }
}));
```

- Create a repost:

```js
const repost = await ok(rpc.post('com.atproto.repo.createRecord', {
  data: {
    repo: myDid,
    collection: 'app.bsky.feed.repost',
    record: {
      $type: 'app.bsky.feed.repost',
      subject: { uri: targetUri },
      createdAt: new Date().toISOString()
    }
  }
}));
```

- Delete a record (post, like, or repost):

```js
await ok(rpc.post('com.atproto.repo.deleteRecord', {
  data: {
    repo: myDid,
    collection: 'app.bsky.feed.post', // or 'app.bsky.feed.like', etc.
    rkey: recordKey
  }
}));
```

Notes and rationale

- The atproto lexicons define record shapes (e.g., `app.bsky.feed.like`) and server procedures (e.g., `com.atproto.repo.createRecord`). While some client libraries expose convenience wrappers such as `agent.app.bsky.feed.post.create`, the canonical, cross-client approach is to call the repo write procedures with the proper `collection` NSID. This is the most robust direct-replacement path when removing `AtpAgent` convenience functions.

- I verified these procedure IDs and record NSIDs against the official lexicons in the Bluesky `atproto` repository (files under `lexicons/app/bsky/feed/*.json` and `lexicons/com/atproto/repo/*.json`).

Next step

- You can now update `index.js` per the migration plan: replace agent.write calls with the `rpc.post('com.atproto.repo.createRecord', ...)` pattern and replace deletes with `rpc.post('com.atproto.repo.deleteRecord', ...)`. Start with `clientLogin` as planned, then migrate `post` and `like/repost/delete` using the examples above.

## Auth requirements per tool (authoritative)

The lexicon files are the source of truth for whether a procedure requires an authenticated session. Below is a concise mapping you can paste into the plan and use while migrating `index.js`.

- login / clientLogin — requires credentials (use `CredentialManager.login`). Source: `@atcute/client` CredentialManager docs and login flow.

- post (create a post) — requires auth. Use `com.atproto.repo.createRecord` with `collection: 'app.bsky.feed.post'`. Source: `lexicons/com/atproto/repo/createRecord.json` + `lexicons/app/bsky/feed/post.json`.

- like (reaction) — requires auth. Implement via `com.atproto.repo.createRecord` with `collection: 'app.bsky.feed.like'`. Source: `lexicons/app/bsky/feed/like.json` + createRecord.

- repost — requires auth. Implement via `com.atproto.repo.createRecord` with `collection: 'app.bsky.feed.repost'`. Source: `lexicons/app/bsky/feed/repost.json` + createRecord.

- delete (remove record) — requires auth. Use `com.atproto.repo.deleteRecord` (input: { repo, collection, rkey }). Source: `lexicons/com/atproto/repo/deleteRecord.json`.

- feed (app.bsky.feed.getFeed), profile (app.bsky.actor.getProfile), search (app.bsky.feed.searchPosts), getPost/getPosts/getLikes/getRepostedBy, resolveHandle (com.atproto.identity.resolveHandle) — read-only; do not require auth. Source: respective lexicon files under `lexicons/app/bsky/feed/*.json` and `lexicons/com/atproto/identity/resolveHandle.json`.

- thread / getPostThread — public read; lexicon note: "Does not require auth, but additional metadata and filtering will be applied for authed requests." Source: `lexicons/app/bsky/feed/getPostThread.json`.

- getTimeline (app.bsky.feed.getTimeline) — authenticated: returns the requesting account's home timeline. Treat as auth-required. Source: `lexicons/app/bsky/feed/getTimeline.json`.

Guidance

- Canonical rule: server write procedures under `com.atproto.repo.*` are implemented by the PDS and are declared as requiring auth in the lexicon; prefer calling `com.atproto.repo.createRecord`/`deleteRecord` for writes when doing direct replacements. Query procedures (type: "query") are generally public unless the lexicon text explicitly references the "requesting account".