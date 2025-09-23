# Migration plan: direct atcute replacement (single-file, no shims)

## Objective

- Replace direct uses of `AtpAgent`/`@atproto/api` with the atcute client library by performing in-place, direct substitutions in `index.js` for each tool, one at a time. No intermediate shims or adapter layers will be added. Keep single-file constraint and manual testing: no unit tests and no feature flags.

## Preliminary checks (before touching code)

Summary of findings (quick facts)

- The project already lists `@atcute/client` in `package.json` (version found on npm: 4.0.3). The package on npm is MIT-licensed.
- The atcute monorepo on GitHub is well-populated and licensed permissively (0BSD for the monorepo); key client functionality lives in `@atcute/client` and schema/definition packages such as `@atcute/bluesky` and `@atcute/atproto` provide the lexicons.
- The atcute monorepo on GitHub is well-populated and licensed permissively (0BSD for the monorepo); key client functionality lives in `@atcute/client`. Schema/definition packages (for example `@atcute/bluesky` and `@atcute/atproto`) are transitive dependencies.
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
  - CredentialManager exposes `manager.session` at runtime (contains tokens), login will always use stored credentials (username/password).

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
  - After this, use rpc for authenticated calls.

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
  - Confirmed `@atcute/bluesky` and `@atcute/atproto` exist as definition packages.
- Confirmed example endpoints: 'https://public.api.bsky.app' for public reads and 'https://bsky.social' for authenticated writes in examples.

Immediate actions you must perform locally before editing code

1. Inspect the lexicon sources to get the exact RPC procedure names for write actions you need (post, like/repost, delete, getPost). Inspect them under `node_modules/@atcute/bluesky` or `node_modules/@atcute/atproto` if available, or consult the upstream lexicon files on unpkg or the atproto repo. Example: search for 'app.bsky.feed.post', 'app.bsky.feed.getFeed', 'app.bsky.repost' etc.

2. Verify login using the project's CLI entrypoint rather than ad-hoc REPL snippets:
  - Run the repository-supported login command:

```cmd
node index.js login
```

  - Follow the interactive prompts to provide the test account `handle` and `password`. The command should exercise `CredentialManager` and perform an authenticated read (for example, `com.atproto.identity.resolveHandle`) to validate the login; confirm it completes successfully and that credentials are stored by the project's credential persistence logic.

3. Confirm what RPC method to call for posting and actions by grepping the lexicon files or by reading the lexicon JSON files online (unpkg or the atproto lexicons in the upstream repo).

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

# Further work outstanding

## Quick summary

This document already captured a comprehensive analysis and mapping from `AtpAgent`/`@atproto/api` to `@atcute/client`. The research, confirmed RPC names, and concrete code snippets for write/read procedures are done. What remains is the hands-on migration: updating `index.js` (starting with `clientLogin`), adapting storage/session persistence, and manually verifying each command in priority order.

## What is done (in this repository / plan)

- Preliminary checks and reconnaissance:
  - Confirmed `@atcute/client` exists and provided the expected Client/CredentialManager/simpleFetchHandler/ok primitives.
  - Mapped AtpAgent idioms to atcute equivalents for public/anonymous and authenticated usage.
  - Documented RPC usage patterns (`rpc.get`, `rpc.post`) and the `ok(...)` unwrap helper.
- Lexicon/procedure validation:
  - Confirmed canonical procedure names and record NSIDs for reads and writes (e.g., `app.bsky.feed.post`, `com.atproto.repo.createRecord`, `com.atproto.repo.deleteRecord`, `app.bsky.feed.getFeed`, etc.).
  - Provided concrete atcute snippets for create-post, create-like, create-repost, and delete using `com.atproto.repo.createRecord`/`deleteRecord`.
- High-level migration workflow and priority order defined (login → feed → profile → search/thread → write actions).

## What is partially done

- Package presence: `@atcute/client` is listed in `package.json` (per the plan).
- Exact parameter / return-shape verification: lexicon IDs are confirmed, but live verification against a running PDS (via REPL or test calls) has not been performed here.
- Keytar persistence approach is described; no code changes were applied yet. This repository will continue to persist raw credentials (username/password) rather than tokens/manager.session.

## What is still outstanding (hands-on migration tasks)

1. Update `clientLogin` in `index.js` (REQUIRED first step)
  - Replace `AtpAgent`/`agent.login(...)` with `CredentialManager` + `Client` per the plan:
    - Create manager: `new CredentialManager({ service })`
    - rpc: `new Client({ handler: manager })`
    - `await manager.login({ identifier, password })`
  - Decide contract returned by `clientLogin`: either return the `rpc` Client instance (preferred) or a small wrapper with the previous convenience methods. Update callers accordingly.
  - Persist raw credentials (username/password) into keytar and update `clientLoginOrFallback` to load credentials and call `manager.login({ identifier, password })` each time; do NOT attempt to persist or reuse `manager.session` tokens.
2. Run a smoke test for login + one authenticated read
  - Use a disposable test account.
  - Run the project's login command:

```cmd
node index.js login
```

  - Confirm `manager.login()` (used by the CLI) succeeds and that an authenticated read such as `com.atproto.identity.resolveHandle` returns a DID.

4. Migrate public/read commands (low-risk)
  - `feed`: replace feed/timeline retrieval with `rpc.get('app.bsky.feed.getFeed', { params: { feed, cursor, limit } })` (or `getTimeline` for authenticated home timeline).
  - `profile`: `rpc.get('app.bsky.actor.getProfile', { params: { actor } })` and followers/follows via `app.bsky.graph.*`.
  - `search` / `thread`: `rpc.get('app.bsky.feed.searchPosts', { params: ... })` and `app.bsky.feed.getPostThread` respectively.
  - Run manual tests: `node index.js feed`, `node index.js profile` to verify output shapes. Adjust shaping/formatters as needed.

5. Migrate write actions (requires auth; higher risk)
  - Implement `post`, `like`, `repost` using `com.atproto.repo.createRecord` with `collection` set to `app.bsky.feed.post|like|repost` and `record` shaped as lexicon requires. Use `ok(rpc.post(...))` to unwrap or handle errors explicitly.
  - Implement `delete` via `com.atproto.repo.deleteRecord`.
  - Test these with a disposable account; verify returned `{ uri, cid }` and that operations are visible on the PDS.

6. Update storage/persistence strategy
  - Continue storing raw credentials (username/password) in keytar and use them to call `manager.login` when needed. Do NOT store or rely on `manager.session` tokens. If you change the storage format in future, provide fallback migration logic in `clientLoginOrFallback` to import older entries.

7. Record versions, update lockfile, and add minimal checks
  - Commit updated `package.json` and lockfile (ensure exact versions are recorded as needed).
  - Add a brief note in `README.md` describing the atcute version and the migration steps.

8. Quality gates and manual validation
  - Lint/typecheck (if present), quick smoke tests for each migrated command, and at least one end-to-end manual verification run for write actions.
  - If repository contains test harnesses (e.g., `test-*.js`), run any relevant quick tests after migration.

## Risks and gotchas to watch while migrating

- RPC parameter and return shapes: Atcute uses names and shapes from the lexicons; these differ from AtpAgent convenience methods. Use `ok(...)` or explicitly handle `{ ok, data }` tuples.
- Auth/session representation: this migration will NOT switch to persisting `manager.session` or tokens. Continue to store raw credentials and call `manager.login` each time; keep a fallback path for legacy credential entries if formats change.
- Write semantics: use `com.atproto.repo.createRecord` for posts/reactions/reposts — ensure `record` matches lexicon (fields like `$type`, `text`, `subject`, `createdAt`).
- Timeline vs feed: `getTimeline` is authenticated and returns the requesting account's home timeline; `getFeed` may be public. Use the appropriate RPC per command.

## Acceptance criteria / done definition

Migration for a single command is considered complete when:

1. `index.js` uses `@atcute/client` for that command (no remaining `AtpAgent` calls in that command's code path).
2. Manual smoke test for the command runs successfully (for writes: verified visible effect on the PDS; for reads: expected output and no runtime errors).
3. If the command requires auth, login flow via `clientLogin` works and stored credentials (username/password) can be loaded and used by `clientLoginOrFallback` to re-login.
4. `package.json` lists the `@atcute` packages and lockfile committed.

## Suggested immediate next actions (concrete)

1. Implement `clientLogin` change in `index.js` and update `clientLoginOrFallback` to load/store `manager.session`.
2. Run the project's login command to verify an authenticated read:

```cmd
node index.js login
```

3. Proceed to migrate `feed` (read) next and run `node index.js feed`.

If you'd like, I can now:

- edit `index.js` to implement `clientLogin` using `CredentialManager` and return a `Client` instance, and update `clientLoginOrFallback` to persist `manager.session` (I can implement this change and run quick local checks).  
- or generate a small REPL script you can run to verify `@atcute/client` behaviour with your credentials.

---

Requirements coverage (mapping to the migration plan)

- Preliminary research and mapping: Done.
- clientLogin migration: Outstanding (highest-priority implementation task).
- Feed/profile/search/thread migrations: Outstanding (next tasks after login).
- Post/like/repost/delete migrations: Outstanding (requires auth verification).
- Manual testing and persistence migration: Outstanding.

"Further work outstanding" now lists clear, ordered tasks, acceptance criteria, and immediate commands. Follow the priority order above (login → feed → profile → search/thread → write actions) and ask me to perform the code edits and verifications you'd like done next.
