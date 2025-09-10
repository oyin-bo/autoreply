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

## Exact RPC procedure names — confirmed vs. to-verify

Below I list RPC procedure names and lexicon IDs relevant to the migration. Some items were confirmed by reading the atproto lexicons and atcute README; others are very likely but should be verified locally in your installed `@atcute/*` packages before editing code.

Confirmed (seen in atproto lexicons or atcute docs):

- `app.bsky.feed.getFeed` — feed retrieval (params: feed, cursor, limit).
- `app.bsky.feed.getPostThread` — thread retrieval (params: uri, depth, parentHeight, etc.).
- `app.bsky.feed.searchPosts` — search posts (params: q, cursor, limit).
- `app.bsky.actor.getProfile` — profile lookup (params: actor).
- `app.bsky.graph.getFollowers` — followers list (params: actor, cursor, limit).
- `app.bsky.graph.getFollows` — following list (params: actor, cursor, limit).
- `com.atproto.identity.resolveHandle` — resolve a handle to a DID (used for short-handle -> DID resolution).
- `com.atproto.repo.createRecord` — lower-level repo create that can be used to write records when higher-level convenience calls are not available.

High-confidence (examples appear in docs or agent-samples; please verify exact procedure name/parameter shape in installed definitions):

- `app.bsky.feed.post.create` — create a post (write). The official atproto docs show usage like `agent.app.bsky.feed.post.create({ repo: did }, { text, createdAt, ... })` so expect a procedure with `.create` for the post collection.
- `app.bsky.feed.getPost` — single post read by URI or by repo/rkey (verify exact name: it may be available as a convenience in some clients or via `com.atproto.repo.getRecord`).
- `com.atproto.repo.deleteRecord` — delete a record (used by delete operations).

To-verify (look these up in your installed `@atcute/*` packages before coding):

- Repost: procedure id may be `app.bsky.repost.create` or `app.bsky.feed.repost.create` or similar. The atproto ecosystem exposes repost/write procedures but exact namespace/name should be confirmed.
- Like/reaction: procedure id may be `app.bsky.reaction.create`, `app.bsky.like.create`, or another `app.bsky.*` procedure. Confirm by grepping the installed definitions.
- Any convenience wrappers (e.g., `getTimeline`, `getPost`, `post`, `like`, `repost`) provided by `AtpAgent` are not one-to-one with RPC IDs — where a convenience method existed you may need to call the corresponding RPC directly (e.g., `rpc.get('app.bsky.feed.getFeed', { params: {...} })` or call `rpc.post('app.bsky.feed.post.create', { data: {...} })` depending on Client API.

How to verify locally (quick checklist)

1. Inspect installed packages in node_modules (or browse the same package version on unpkg): look under `node_modules/@atcute/bluesky` and `node_modules/@atcute/atproto` for `lib/types` or `dist` folders that list lexicon IDs and procedure names.

2. Grep/search for likely procedure ids (case-sensitive) such as `feed.post`, `repost.create`, `reaction.create`, `repo.createRecord`, `repo.deleteRecord`, `feed.getPost`, `feed.getPostThread`.

3. Confirm both the procedure id (string you will pass to `client.get`/`client.post`) and the parameter shape (`params` vs `data` for write operations). The `@atcute/client` README example shows `rpc.get('app.bsky.actor.getProfile', { params: { actor } })` and write calls typically use `rpc.post('app.bsky.feed.post.create', { data: { ... } })` or `rpc.call`-style APIs depending on client version.

Example places to look in the installed package (paths are relative to your project root):

- node_modules/@atcute/bluesky/lib/types/app/bsky
- node_modules/@atcute/atproto/lib/types/com/atproto
- node_modules/@atcute/bluesky/dist (or lib) — look for JSON or TypeScript files that contain `id` fields with the lexicon RPC ids

If you prefer a quick programmatic check, open Node REPL and require the package files or simply read the JSON lexicon files to list `id` fields — this will show the exact RPC ids and their param/output schemas.

Recommendation

- Before editing `index.js`, confirm the exact write RPC ids for post/repost/reaction/delete in your installed `@atcute` definition packages; update the migration plan mapping accordingly; then perform the direct replacement for `clientLogin` (use `CredentialManager` and `Client`) and validate authentication.

Once you confirm the exact procedure names locally, I'll update the plan lines in this document with the precise RPC ids to use for each tool (and example call snippets to place in `index.js`) if you want.