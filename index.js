#!/usr/bin/env node
// @ts-check

const fs = require('fs');
const path = require('path');
const os = require('os');
const readline = require('readline');
const readlineSync = require('readline-sync');

const { name, version } = require('./package.json');

(async () => {

  const { Client, CredentialManager, simpleFetchHandler, ok } = await import('@atcute/client');

  /**
   * @typedef {{
   *  setPassword(service: string, account: string, password: string): Promise<void>,
   *  getPassword(service: string, account: string): Promise<string | null>
   * }} KeytarLike
   */
  /** @type {KeytarLike | Promise<KeytarLike>} */
  let keytarOrPromise = requireOrMockKeytar();

  function requireOrMockKeytar() {

    const CRED_FILE = path.join(__dirname, '.bluesky_creds.json');
    const fallbackKeytar = {
      async setPassword(service, account, password) {
        let creds = {};
        if (fs.existsSync(CRED_FILE)) {
          try { creds = JSON.parse(fs.readFileSync(CRED_FILE, 'utf8')); } catch { }
        }
        creds[account] = password;
        fs.writeFileSync(CRED_FILE, JSON.stringify(creds, null, 2));
      },
      async getPassword(service, account) {
        let creds = {};
        if (fs.existsSync(CRED_FILE)) {
          try { creds = JSON.parse(fs.readFileSync(CRED_FILE, 'utf8')); } catch { }
        }
        return creds[account] || null;
      }
    };

    try {
      const keytarMod = require('keytar');
      const tryPromise = keytarMod.getPassword(name, 'default_handle');
      return (
        tryPromise
          .then(() => keytarMod)
          .catch(() => {
            return fallbackKeytar;
          })
          .then((successKeytar) => {
            keytarOrPromise = successKeytar;
            return keytarOrPromise;
          })
      );
    } catch (e) {
      return fallbackKeytar;
    }
  }

  const PostSchema = {
    type: 'object',
    properties: {
      indexedAt: { type: 'string', description: 'ISO timestamp when the post was indexed.' },
      author: { type: 'string', description: 'BlueSky handle of the author.' },
      authorName: { type: 'string', description: 'Name of the author, if available.' },
      postURI: { type: 'string', description: 'URI of the post.' },
      replyToURI: { type: 'string', description: 'URI of the post being replied to, if any.' },
      text: { type: 'string', description: 'Text content of the post.' },
      likeCount: { type: 'number', description: 'Number of likes.', nullable: true },
      replyCount: { type: 'number', description: 'Number of replies.', nullable: true },
      repostCount: { type: 'number', description: 'Number of reposts.', nullable: true },
      quoteCount: { type: 'number', description: 'Number of quotes.', nullable: true },
      links: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            url: { type: 'string', format: 'uri', description: 'URL of the link.' },
            title: { type: 'string', description: 'Title of the link, if available.' }
          },
          required: ['url']
        },
        description: 'List of links included in the post, which could be images, URL links, videos or other posts.'
      }
    },
    required: ['indexedAt', 'author', 'postURI', 'text']
  };

  class Tools {

    /**
   * @param {{ login?: string, password?: string }} args
   */
    async login({ login, password }) {
      if (!login || !password)
        throw new Error('Login handle and password are required.');

      await this.clientLogin({ login, password });

      return 'Credentials stored and default handle set to ' + login + '.';
    }

    'login:tool' = {
      name: 'login',
      description: 'Login and cache BlueSky handle and password.',
      inputSchema: {
        type: 'object',
        properties: {
          login: { type: 'string', description: 'Your BlueSky handle, who are you on BlueSky?' },
          password: { type: 'string', description: 'Your BlueSky app password (better not share it).' }
        },
        required: ['login', 'password']
      }
    };

    /**
   * @param {{
   *  cursor?: string,
   *  feed?: string,
   *  login?: string,
   *  password?: string,
   *  limit?: number
   * }} _
   */
    async feed({ cursor, feed, login: loginHandle, password, limit }) {
      const agent = await this.clientLoginOrFallback({ login: loginHandle, password });
      let feedData;

      if (feed || !loginHandle) {
        if (feed) {
          const fullFeedUri = breakFeedURI(feed);
          if (!fullFeedUri) {
            const likelyFeeds = await ok(agent.get('app.bsky.unspecced.getPopularFeedGenerators', {
              params: { query: feed }
            }));
            if (likelyFeeds.feeds.length) {
              feed = likelyFeeds.feeds[0].uri;
            }
          }
        }

        feedData = await ok(agent.get('app.bsky.feed.getFeed', {
          params: {
            feed: feed || 'at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot',
            cursor,
            limit: Math.min(limit || 20, 100)
          }
        }));
      } else {
        feedData = await ok(agent.get('app.bsky.feed.getTimeline', {
          params: {}
        }));
      }

      const formatted = /** @type {ReturnType<typeof formatPost>[]} */(feedData.feed.map(post =>
        !post.post ? undefined : formatPost(post.post)
      ).filter(Boolean));

      return {
        cursor: feedData.cursor,
        posts: formatted.map(post => post.structured)
      };
    }

    'feed:tool' = {
      name: 'feed',
      description:
        'Get the latest feed from BlueSky. ' +
        'Returns a list of messages or tweets or posts or skeets however you call them. ' +
        'If you want to see the latest posts from a specific user, just provide their handle. ' +
        'These feeds are paginated, you get the top chunk and a cursor, you can call the same tool again with the cursor to get more posts.',
      inputSchema: {
        type: 'object',
        properties: {
          feed: {
            type: 'string',
            description:
              '(Optional) The feed to retrieve, can be a BlueSky feed URI, or a name for a feed to search for. ' +
              'If unspecified, it will return the default popular feed What is Hot.'
          },
          login: {
            type: 'string', description:
              '(Optional) BlueSky handle for which the feed is requested. ' +
              'If unspecified, or specified as anonymous, the feed will be retrieved in the incognito mode.'
          },
          password: { type: 'string', description: '(Optional) BlueSky password to use.' },
          cursor: { type: 'string', description: '(Optional) Cursor for pagination.' },
          limit: { type: 'number', description: '(Optional) Limit the number of posts returned, defaults to 20, max 100.' }
        },
        required: []
      },
      outputSchema: {
        type: 'object',
        properties: {
          cursor: { type: 'string', description: 'Cursor for pagination, if more data is available.' },
          posts: {
            type: 'array',
            items: PostSchema
          }
        }
      }
    };

    /**
   * @param {{
   *  user: string,
   *  cursor?: string
   * }} _
   */
    async profile({ user, cursor }) {
      const agent = this.clientIncognito();
      const [followersCursor, followsCursor] = cursor ? JSON.parse(cursor) : [undefined, undefined];

      if (likelyDID(user)) {
        user = unwrapShortDID(user);
      } else {
        user = unwrapShortHandle(user);

        if (/\s/.test(user) || !/\./.test(user)) {
          // need to search for the user
          const actors = await ok(agent.get('app.bsky.actor.searchActors', {
            params: { q: user }
          }));
          if (actors.actors.length) {
            user = actors.actors[0].did;
          }
        }

      }

      const [profile, followers, following] = await Promise.all([
        ok(agent.get('app.bsky.actor.getProfile', { params: { actor: user } })),
        ok(agent.get('app.bsky.graph.getFollowers', { params: { actor: user, cursor: followersCursor } })),
        ok(agent.get('app.bsky.graph.getFollows', { params: { actor: user, cursor: followsCursor } }))
      ]);

      const structuredContent = {
        handle: profile.handle,
        displayName: profile.displayName,
        description: profile.description,
        createdAt: profile.createdAt,
        avatar: profile.avatar,
        banner: profile.banner,
        followersCount: profile.followersCount,
        followingCount: profile.followsCount,
        postsCount: profile.postsCount,
        followers: followers.followers.map((follower) => '@' + follower.handle),
        following: following.follows.map((follow) => '@' + follow.handle),
        cursor: JSON.stringify([followers.cursor, following.cursor])
      };

      return structuredContent;
    }

    'profile:tool' = {
      name: 'profile',
      description: 'Search for profile details, or retrieve exact by handle. Also report followers, and following, avatar, description and more.',
      inputSchema: {
        type: 'object',
        properties: {
          user: { type: 'string', description: 'The user\'s handle, name or just search term.' },
          cursor: { type: 'string', description: '(Optional) Cursor for pagination of followers/following.' },
        },
        required: ['user']
      },
      outputSchema: {
        type: 'object',
        properties: {
          createdAt: { type: 'string', format: 'date-time', description: 'The date and time when the profile was created.' },
          handle: { type: 'string' },
          displayName: { type: 'string', description: 'The display name of the account, tends to be short one line name, but longer than handle.' },
          description: { type: 'string', description: 'The description or bio of the account, tends to have some general info about the account, bragging rights and other info.' },
          avatar: { type: 'string', format: 'uri', description: 'URL to the profile icon (avatar).' },
          banner: { type: 'string', format: 'uri', description: 'URL to the profile banner image, usually a broad rectangle.' },
          followersCount: { type: 'number' },
          followingCount: { type: 'number' },
          postsCount: { type: 'number' },
          followers: { type: 'array', items: { type: 'string' } },
          following: { type: 'array', items: { type: 'string' } },
          cursor: { type: 'string', description: 'Cursor for pagination of followers/following.' }
        }
      }
    };

    async search({ from, query, login, password, cursor, limit }) {
      // Always run an incognito search. Optionally try an authenticated search in parallel
      // and prefer authenticated results if they succeed. Authenticated search failures
      // are tolerated (403, auth errors, etc.) — incognito must be used as the fallback.
      if (!query && !from) query = '*';

      // Normalize `from` to a handle when possible; we can do this against the incognito agent
      // so it can't fail due to missing auth.
      if (from) {
        if (likelyDID(from)) {
          try {
            const resolved = await ok(this.clientIncognito().get('app.bsky.actor.getProfile', { params: { actor: unwrapShortDID(from) } }));
            from = resolved.handle;
          } catch (e) {
            // If resolution fails in incognito, fall back to cheap unwrap/normalization
            from = unwrapShortHandle(from);
          }
        } else {
          from = unwrapShortHandle(from);
        }
      }

      const params = {
        q: (query || '') + (from ? ' from:' + from : ''),
        cursor,
        limit: Math.min(limit || 20, 100)
      };

      // Start incognito search
      // Helper to try incognito search, with a fallback to bsky.social if the public API returns 403.
      const tryIncognitoSearch = async () => {
        try {
          const agent = this.clientIncognito();
          return await ok(agent.get('app.bsky.feed.searchPosts', { params }));
        } catch (e) {
          // If public endpoint rejects (403), try the bsky.social host fallback
          const status = e?.status || e?.response?.status || e?.statusCode;
          if (status === 403) {
            try {
              const handler = simpleFetchHandler({ service: 'https://bsky.social' });
              const client = new Client({ handler });
              return await ok(client.get('app.bsky.feed.searchPosts', { params }));
            } catch (e2) {
              // fallback failed too, rethrow original error for upstream handling
              throw e2;
            }
          }
          throw e;
        }
      };

      const incognitoPromise = tryIncognitoSearch();

      // Start authenticated search in parallel if a login looks available.
      // We deliberately do not use clientLoginOrFallback because that would return incognito
      // and duplicate the same request; instead attempt clientLogin and allow it to fail.
      let authPromise = null;
      try {
        // Only attempt to create an authenticated client if a login is provided or a default exists
        const keytar = await keytarOrPromise;
        const effectiveLogin = login || (await keytar.getPassword(name, 'default_handle')) || undefined;
        if (effectiveLogin && effectiveLogin !== 'anonymous') {
          // Try to create/authenticate the client. This may reject; we'll catch below.
          authPromise = (async () => {
            const agent = await this.clientLogin({ login: effectiveLogin, password });
            return await ok(agent.get('app.bsky.feed.searchPosts', { params }));
          })();
        }
      } catch (e) {
        // Any error here should not prevent incognito from running — just log and continue.
        console.error('Auth search setup failed:', e?.message || e);
        authPromise = null;
      }

      // Wait for both (or just incognito) to settle. Use allSettled so auth failure doesn't throw.
      const settled = await Promise.allSettled([incognitoPromise, authPromise].filter(Boolean));

      // settled[0] is incognito (always present). If auth was started, it's the next item.
      const incResult = settled[0];
      const authResult = settled[1];

      let feed;

      if (incResult.status === 'fulfilled') {
        // Prefer incognito results when they succeed (public fallback must be used when available).
        feed = incResult.value;

        if (authResult && authResult.status === 'fulfilled') {
          // If authenticated search also succeeded, prefer authenticated (more personalized).
          feed = authResult.value;
        } else if (authResult && authResult.status === 'rejected') {
          console.error('Authenticated search failed (ignored):', authResult.reason?.message || authResult.reason);
        }
      } else {
        // Incognito failed. Try to use authenticated result if it succeeded.
        if (authResult && authResult.status === 'fulfilled') {
          feed = authResult.value;
        } else {
          console.error('Incognito search failed and no authenticated fallback available:', incResult.reason || incResult.status, authResult?.reason || authResult?.status);
          return { cursor: null, posts: [] };
        }
      }

      const formatted = feed.posts.map(post => formatPost(post));

      return {
        cursor: feed.cursor,
        posts: formatted.map(post => post.structured)
      };
    }

    'search:tool' = {
      name: 'search',
      description:
        'Search messages (also known as posts, tweets, or skeets) on BlueSky by text query. ' +
        'You can search for posts by text, or filter by author handle. ' +
        'If you want to see messages from a specific user, just provide their handle. ' +
        'That handle can also be a special value "me" to indicate the authenticated user\'s posts. ' +
        'These searches are paginated, you get the top chunk and a cursor, you can call the same tool again with the cursor to get more posts.',
      inputSchema: {
        type: 'object',
        properties: {
          from: { type: 'string', description: '(Optional) Messages from who, a handle or say \'me\' for the user that\'s logged in.' },
          query: { type: 'string', description: '(Optional) Text to search for in messages. Here\'s an old blog post about search tricks, https://bsky.social/about/blog/05-31-2024-search but you can probably find more Googling, because these things change and improve often.' },
          cursor: { type: 'string', description: '(Optional) Cursor for pagination.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to use for authenticated search, anonymous to force unanuthenticated.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use.' },
          limit: { type: 'number', description: '(Optional) Limit the number of posts returned, defaults to 20, max 100.' }
        },
        required: []
      },
      outputSchema: {
        type: 'object',
        properties: {
          cursor: { type: 'string', description: 'Cursor for pagination, if more data is available.' },
          posts: {
            type: 'array',
            items: PostSchema
          }
        }
      }
    };

    /**
   * @param {{
   * postURI: string,
   * login?: string,
   * password?: string
   * }} _
   */
    async thread({ postURI, login, password }) {
      if (!postURI) throw new Error('postURI is required.');
      const agent = await this.clientLoginOrFallback({ login, password });

      const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
      if (postRef) {
        if (!likelyDID(postRef.shortDID)) {
          const resolved = await ok(agent.get('com.atproto.identity.resolveHandle', { params: { handle: postRef.shortDID.replace('@', '') } }));
          postRef.shortDID = resolved.did;
        }

        postURI = makeFeedUri(postRef.shortDID, postRef.postID);
      }

      // Fetch thread
      const thread = await ok(agent.get('app.bsky.feed.getPostThread', { params: { uri: postURI } }));
      // Use a generic any type for record shapes so we don't depend on @atproto/api types at runtime
      const anchorRecord = /** @type {any} */(/** @type {*} */(/** @type {any} */(thread.thread).post?.record));

      /**
       * @typedef {any} PostOrPlaceholder
       */

      /**
       * Flatten thread into array
       * @param {PostOrPlaceholder} [node]
       */
      function flattenThread(node) {
        /**
         * @type {ReturnType<typeof formatPost>[]}
         */
        const arr = [];
        if (!node) return arr;
        if (node.post) {
          const postData = formatPost(node.post);
          arr.push(postData);
        }
        if (node.replies?.length) {
          for (const reply of node.replies) {
            arr.push(...flattenThread(reply));
          }
        }
        return arr;
      }
      const posts = flattenThread(thread.thread);

      // restore the context
      if (!posts.find(p => p.postURI === anchorRecord?.reply?.root?.uri)) {
        if (anchorRecord?.reply?.root?.uri) {
          const agentFallback = this.clientIncognito();
          const rootPost = await ok(agentFallback.get('app.bsky.feed.getPostThread', { params: { uri: anchorRecord?.reply?.root?.uri } }));
          const updated = flattenThread(rootPost.thread);
          posts.unshift(...updated);
        }
      }

      return {
        posts: posts.map(post => post.structured)
      };
    }

    'thread:tool' = {
      name: 'thread',
      description:
        'Fetch a thread by post URI, it returns all the replies and replies to replies, the whole bunch. ' +
        'If you\'re already logged in, this will fetch the thread as viewed by the logged in user (or you can provide handle/password directly). ' +
        'If the handle is a special placeholder value \'anonymous\', it will fetch the thread in incognito mode, ' +
        'that sometimes yields more if your logged in account is blocked by other posters. ' +
        'Note that messages in the thread are sometimes called skeets, tweets, or posts, but they are all the same thing.',
      inputSchema: {
        type: 'object',
        properties: {
          postURI: { type: 'string', description: 'The BlueSky URL of the post, or also can be at:// URI of the post to fetch the thread for.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to use for authenticated fetch.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use.' }
        },
        required: ['postURI']
      },
      outputSchema: {
        type: 'object',
        properties: {
          posts: {
            type: 'array',
            items: PostSchema
          }
        }
      }
    };

    async post({ text, handle, password, replyToURI }) {
      if (!handle || !password) {
        [{ handle, password }] = [await this.getCredentials(handle)];
      }

      const agent = await this.clientLogin({ login: handle, password });
      let reply;
      let replyTracking;
      const postRef = breakPostURL(replyToURI) || breakFeedURI(replyToURI);
      if (postRef) {
        if (!likelyDID(postRef.shortDID)) {
          const resolved = await ok(agent.get('com.atproto.identity.resolveHandle', { params: { handle: postRef.shortDID.replace('@', '') } }));
          postRef.shortDID = resolved.did;
        }

        const replyToPost = await ok(agent.get('com.atproto.repo.getRecord', {
          params: {
            repo: unwrapShortDID(postRef.shortDID),
            collection: 'app.bsky.feed.post',
            rkey: postRef.postID
          }
        }));
        reply = /** @type {const} */({
          root: replyToPost.value.reply?.root || {
            $type: 'com.atproto.repo.strongRef',
            uri: replyToPost.uri,
            cid: replyToPost.cid
          },
          parent: {
            $type: 'com.atproto.repo.strongRef',
            uri: replyToPost.uri,
            cid: replyToPost.cid
          }
        });
        replyTracking = replyToPost.value.text;
      }

      const myDid = agent.manager?.session?.did;
      if (!myDid) throw new Error('No authenticated session found');

      const posted = await ok(/** @type {any} */(agent).post('com.atproto.repo.createRecord', {
        data: {
          repo: myDid,
          collection: 'app.bsky.feed.post',
          record: {
            $type: 'app.bsky.feed.post',
            text,
            reply,
            createdAt: new Date().toISOString()
          }
        }
      }));

      return (
        replyTracking ? 'Replied to ' + replyTracking + ' with ' + /** @type {any} */(posted).uri + ':\n' + text :
          replyToURI ? 'Could not split ' + JSON.stringify(replyToURI) + '/' + JSON.stringify(postRef) + ', posted alone ' + /** @type {any} */(posted).uri + ':\n' + text :
            'Posted ' + /** @type {any} */(posted).uri + ':\n' + text
      );
    }

    'post:tool' = {
      name: 'post',
      description: 'Post a message to BlueSky. Some people call these messages tweets or skeets or posts, same difference.',
      inputSchema: {
        type: 'object',
        properties: {
          replyToURI: { type: 'string', description: 'The post URI (or BlueSky URL of the post) to which the reply is made (if any).' },
          text: { type: 'string', description: 'The text to post.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to post the message as.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use.' }
        },
        required: ['text']
      }
    };

    async like({ postURI, login, password }) {
      if (!postURI) throw new Error('postURI is required.');

      const agent = await this.clientLogin({ login, password });

      const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
      if (!postRef) throw new Error('Invalid post URI or feed URI.');
      if (!likelyDID(postRef.shortDID)) {
        const resolved = await ok(agent.get('com.atproto.identity.resolveHandle', { params: { handle: postRef.shortDID.replace('@', '') } }));
        postRef.shortDID = resolved.did;
      }

      const likePost = await ok(agent.get('com.atproto.repo.getRecord', {
        params: {
          repo: unwrapShortDID(postRef.shortDID),
          collection: 'app.bsky.feed.post',
          rkey: postRef.postID
        }
      }));

      const myDid = agent.manager?.session?.did;
      if (!myDid) throw new Error('No authenticated session found');

      await ok(/** @type {any} */(agent).post('com.atproto.repo.createRecord', {
        data: {
          repo: myDid,
          collection: 'app.bsky.feed.like',
          record: {
            $type: 'app.bsky.feed.like',
            subject: { uri: makeFeedUri(postRef.shortDID, postRef.postID) },
            createdAt: new Date().toISOString()
          }
        }
      }));
    
      return (
        `Post liked: ${postRef.shortDID}/${postRef.postID} (${likePost.uri}): ${likePost.value.text}`
      );
    }

    'like:tool' = {
      name: 'like',
      description: 'Like a post by URI or BlueSky URL.',
      inputSchema: {
        type: 'object',
        properties: {
          postURI: { type: 'string', description: 'The BlueSky URL or at:// URI of the post to like.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to authenticate as. Leave empty for already logged in user.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use. Leave empty for already logged in user.' }
        },
        required: ['postURI']
      }
    };

    async repost({ postURI, login, password }) {
      if (!postURI) throw new Error('postURI is required.');

      const agent = await this.clientLogin({ login, password });

      const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
      if (!postRef) throw new Error('Invalid post URI or feed URI.');
      if (!likelyDID(postRef.shortDID)) {
        const resolved = await ok(agent.get('com.atproto.identity.resolveHandle', { params: { handle: postRef.shortDID.replace('@', '') } }));
        postRef.shortDID = resolved.did;
      }

      const repostPost = await ok(agent.get('com.atproto.repo.getRecord', {
        params: {
          repo: unwrapShortDID(postRef.shortDID),
          collection: 'app.bsky.feed.post',
          rkey: postRef.postID
        }
      }));

      const myDid = agent.manager?.session?.did;
      if (!myDid) throw new Error('No authenticated session found');

      await ok(/** @type {any} */(agent).post('com.atproto.repo.createRecord', {
        data: {
          repo: myDid,
          collection: 'app.bsky.feed.repost',
          record: {
            $type: 'app.bsky.feed.repost',
            subject: { uri: makeFeedUri(postRef.shortDID, postRef.postID) },
            createdAt: new Date().toISOString()
          }
        }
      }));
    
      return (
        `Post reposted: ${postRef.shortDID}/${postRef.postID} (${repostPost.uri}): ${repostPost.value.text}`
      );
    }

    'repost:tool' = {
      name: 'repost',
      description: 'Repost a post by URI or BlueSky URL.',
      inputSchema: {
        type: 'object',
        properties: {
          postURI: { type: 'string', description: 'The BlueSky URL or at:// URI of the post to repost.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to authenticate as. Leave empty for already logged in user.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use. Leave empty for already logged in user.' }
        },
        required: ['postURI']
      }
    };

    async delete({ postURI, handle, password }) {
      if (!postURI) throw new Error('postURI is required.');

      const agent = await this.clientLogin({ login: handle, password });
    
      // Parse the URI to get repo and collection details
      const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
      if (postRef) {
        const myDid = agent.manager?.session?.did;
        if (!myDid) throw new Error('No authenticated session found');
      
        await ok(/** @type {any} */(agent).post('com.atproto.repo.deleteRecord', {
          data: {
            repo: myDid,
            collection: 'app.bsky.feed.post',
            rkey: postRef.postID
          }
        }));
      } else {
        // If it's already a complete URI, try to extract repo and rkey
        const uriParts = postURI.match(/^at:\/\/([^\/]+)\/([^\/]+)\/(.+)$/);
        if (!uriParts) throw new Error('Invalid post URI format');
      
        const [, repo, collection, rkey] = uriParts;
        await ok(/** @type {any} */(agent).post('com.atproto.repo.deleteRecord', {
          data: {
            repo,
            collection,
            rkey
          }
        }));
      }
    
      return 'Post deleted';
    }

    'delete:tool' = {
      name: 'delete',
      description: 'Delete a post by URI (authenticated only).',
      inputSchema: {
        type: 'object',
        properties: {
          postURI: { type: 'string', description: 'The URI of the post to delete.' },
          login: { type: 'string', description: '(Optional) BlueSky handle to authenticate as, if not logged in already.' },
          password: { type: 'string', description: '(Optional) BlueSky password to use.' }
        },
        required: ['postURI']
      },
      outputSchema: {
        type: 'object',
        properties: {
          success: { type: 'boolean' },
          message: { type: 'string' }
        },
        required: ['success', 'message']
      }
    };

    /**
     * @param {{ login?: string, password?: string }} _
     * @returns {Promise<InstanceType<Client> & { authenticated?: boolean, manager?: InstanceType<CredentialManager> }>}
     */
    async clientLoginOrFallback({ login, password }) {
      const keytar = await keytarOrPromise;
      if (!login) login = (await keytar.getPassword(name, 'default_handle')) || undefined;
      // Treat explicit 'anonymous' as no-login
      if (login === 'anonymous') login = undefined;

      // Validate the stored default handle to avoid trying to derive a service from invalid values
      if (login && !likelyDID(login)) {
        try {
          // normalizeAndEnsureValidHandle will throw if handle is invalid (missing domain, bad chars)
          normalizeAndEnsureValidHandle(login);
        } catch (err) {
          // Bad stored default handle - ignore and fallback to incognito
          console.error('Ignoring invalid stored default handle:', login);
          login = undefined;
        }
      }

      // Only attempt to read the saved password if we have a (valid) login
      if (login) {
        password = password || /** @type {string} */(await keytar.getPassword(name, login));
        try {
          return await this.clientLogin({ login, password: /** @type {string} */(password) });
        } catch (e) {
          // If login fails for any reason, fall back to incognito rather than crashing the feed
          console.error('Login failed for', login, password.slice(0, 2) + Array(password.length - 2).fill('*').join(''), '- falling back to incognito:', e?.message || e);
          return this.clientIncognito();
        }
      }

      return this.clientIncognito();
    }

    /**
     * @type {Record<string, InstanceType<Client> & { authenticated?: boolean, manager?: InstanceType<CredentialManager> }>}
     */
    _clientLoggedInByHandle = {};

    /**
     * @param {{ login: string, password: string }} param0
     */
    async clientLogin({ login, password }) {
      const existing = this._clientLoggedInByHandle[login];
      if (existing) return existing;
      // Derive service URL using strict atproto algorithm:
      // 1. If the identifier is a DID, use the default PDS.
      // 2. Otherwise, normalize and validate the handle using strict atproto rules.
      // 3. Extract the host by dropping the leftmost label (user part) from the normalized handle.
      // 4. Prepend 'https://' to form the service URL.
      let service = 'https://bsky.social';
      // TODO: resolve user's PDS from their handle/DID when possible

      const manager = new CredentialManager({ service });
      const rpc = /** @type {InstanceType<Client> & { authenticated?: boolean, manager?: InstanceType<CredentialManager> }} */(new Client({ handler: manager }));
    
      await manager.login({ identifier: login, password });

      // store credentials
      const keytar = await keytarOrPromise;
      await keytar.setPassword(name, login, password);
      await keytar.setPassword(name, 'default_handle', login);

      rpc.authenticated = true;
      rpc.manager = manager;

      this._clientLoggedInByHandle[login] = rpc;
      return rpc;
    }

    /**
     * @type {InstanceType<Client> | undefined}
     */
    _clientIncognito;

    clientIncognito() {
      if (this._clientIncognito) return this._clientIncognito;
      // Use the project's public read endpoint (not environment-overridable).
      const service = 'https://public.api.bsky.app';
      const handler = simpleFetchHandler({ service });
      this._clientIncognito = new Client({ handler });
      return this._clientIncognito;
    }

    /**
  * @param {string} [handleImpersonate]
  */
    async getCredentials(handleImpersonate) {
      const keytar = await keytarOrPromise;

      let password;
      let handle = handleImpersonate;
      if (!handle) handle = await keytar.getPassword(name, 'default_handle') || undefined;
      if (!handle) throw new Error('BlueSky login is required.');
      password = await keytar.getPassword(name, handle);
      if (!password) throw new Error('Password for ' + handle + ' is lost, please login again.');
      return { handle, password };
    }

  }

  // @ts-ignore Adding custom property to prototype
  SyntaxError.prototype.mcpCode = -32700;

  class McpError extends Error {
    /**
     * @param {string} message The error message.
     * @param {number} code The JSON-RPC error code.
     * @param {any} [extra] Extra error data (maps to mcpExtra).
     */
    constructor(message, code, extra = null) {
      super(message);
      this.name = 'McpError';
      /** @type {number} */
      this.mcpCode = code;
      /** @type {any} */
      this.mcpExtra = extra;
    }
  }

  class McpServer {

    tools = new Tools();

    /**
     * First call to MCP.
     * @param {{ protocolVersion?: string, capabilities?: any, clientInfo?: any }} [_]
     */
    initialize({ protocolVersion, capabilities, clientInfo } = {}) {
      return {
        protocolVersion: '2025-06-18',
        capabilities: {
          tools: {
            listChanged: false, // We don't support dynamic tool list changes for this skeleton
          },
          // resources: {}, // No resource capabilities for this skeleton
          // prompts: {},   // No prompt capabilities for this skeleton
        },
        serverInfo: {
          name: 'random-number-mcp-server',
          version: '1.0.0',
        },
      };
    }

    /**
     * Implementation is required, but does not need to do anything.
     */
    'notifications/initialized'() {
    }

    /** Does not get called at least in Gemini CLI */
    shutdown() {
      process.nextTick(() => process.exit(0));
    }

    'tools/list'() {
      return {
        tools: getInfo(this.tools).map(([name, info]) => info).filter(Boolean)
      };
    }

    async 'tools/call'({ name, arguments: args }) {
      if (!this.tools[name])
        throw new McpError(`Tool '${name}' not found`, -32601, `The tool '${name}' is not recognized by this server.`);

      const structuredContent = await this.tools[name](args);

      console.error('Tool ' + name + ': ', args, structuredContent);

      return {
        content: [
          {
            type: 'text',
            text: typeof structuredContent === 'string' ? structuredContent : JSON.stringify(structuredContent, null, 2),
          }
        ],
        structuredContent
      };
    }
  }

  function runMcp() {

    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout, // Not strictly needed for output, but good practice for readline
      terminal: false // Set to false when reading from non-interactive streams like pipes
    });

    const mcp = new McpServer();

    // Listen for each line of input
    rl.on('line', async (line) => {
      let request = undefined;
      if (!line) return;
      try {
        request = JSON.parse(line);

        // Log to stderr for debugging, as stdout is for protocol messages
        // console.error('Request ', request);

        if (!mcp[request.method])
          throw new McpError(`Method '${request.method}' not found`, -32601, `The method '${request.method}' is not recognized by this server.`);

        const result = await mcp[request.method](request.params);

        // If requestID is undefined/null (notification), no response is sent.
        if (typeof request.id !== 'undefined') { // Check if it's a request (has an ID)
          // console.error('Response ', result);
          process.stdout.write(
            JSON.stringify({
              jsonrpc: '2.0',
              id: request.id,
              result
            }) + '\n');
        }

      } catch (e) {
        // console.error(`Error processing line (request ID ${request?.id || 'N/A'}):`, e);

        process.stdout.write(
          JSON.stringify({
            jsonrpc: '2.0',
            id: request?.id,
            error: {
              code: e.mcpCode || -32000,
              message: e.message,
              data: e.mcpExtra || e.stack
            }
          }) + '\n');
      }
    });

    rl.on('close', () => {
      process.exit(0);
    });

    rl.on('error', (err) => {
      // console.error('Readline error:', err);
      process.stdout.write(
        JSON.stringify({
          jsonrpc: '2.0',
          id: null,
          error: {
            code: -32000, // Internal error
            message: err.message || 'Internal error',
            data: 'Readline stream error occurred.',
          },
        }) + '\n');
      process.exit(1);
    });

  }

  function getInfo(obj) {
    return Object.getOwnPropertyNames(Object.getPrototypeOf(obj))
      .filter(name => typeof obj[name] === 'function' && name !== 'constructor')
      .map(name => [name, obj[name + ':tool']]);
  }


  /**
   * @param {any} post
   */
  function formatPost(post) {
    /** @type {any} */
    const postRecord = post.record
    let replyToURI = postRecord.reply?.parent?.uri;
    if (replyToURI === post.uri) replyToURI = undefined;

    const header =
      post.indexedAt + ' @' + post.author.handle +
      (
        post.author.displayName ?
          ' ' + JSON.stringify(post.author.displayName) + ' ' :
          ''
      ) +
      ' postURI: ' + post.uri +
      (replyToURI ? ' reply to: ' + replyToURI : '');
  
    const text = /** @type {string} */(
      post.record.text || ''
    ).split('\n').map(line => '> ' + line).join('\n');

    const stats =
      (post.likeCount || post.replyCount || post.repostCount || post.quoteCount ?
        '(' +
        [
          post.likeCount ? post.likeCount + ' likes' : '',
          post.replyCount ? post.replyCount + ' replies' : '',
          post.repostCount ? post.repostCount + ' reposts' : '',
          post.quoteCount ? post.quoteCount + ' quotes' : ''
        ].filter(Boolean).join(', ') +
        ')'
        : ''
      );
  
    const textual = header + '\n' + text + stats;

    let links = extractEmbeds(post.author.handle, postRecord.embed);

    return {
      textual,
      structured: {
        indexedAt: post.indexedAt,
        author: post.author.handle,
        authorName: post.author.displayName,
        postURI: post.uri,
        replyToURI,
        text: /** @type {string} */(post.record.text),
        likeCount: post.likeCount,
        replyCount: post.replyCount,
        repostCount: post.repostCount,
        quoteCount: post.quoteCount,
        links
      }
    };
  }

  /**
   * @param {string} shortDID
   * @param {any} embed
   */
  function extractEmbeds(shortDID, embed) {
    if (!embed) return;

    /** @type {{ url: string, title?: string }[] | undefined} */
    let embeds = undefined;

    embeds = addEmbedImages(shortDID, /** @type {any} */(embed).images, embeds);
    embeds = addEmbedVideo(shortDID, /** @type {any} */(embed), embeds);
    embeds = addEmbedExternal(shortDID, /** @type {any} */(embed).external, embeds);
    embeds = addEmbedRecord(/** @type {any} */(embed).record, embeds);
    embeds = addEmbedRecordMedia(shortDID, /** @type {any} */(embed), embeds);

    return embeds;
  }

  /**
   * @param {string} shortDID
   * @param {any} embedImages 
   * @param {{ url: string, title?: string }[] | undefined} embeds 
   */
  function addEmbedImages(shortDID, embedImages, embeds) {
    if (!embedImages?.length) return embeds;
    for (const img of embedImages) {
      if (!img) continue;
      const url = getFeedBlobUrl(shortDID, img.image?.ref?.toString());
      if (url) {
        embeds = addToArray(embeds, {
          url,
          title: img.alt || undefined
        });
      }
    }
    return embeds;
  }

  /**
   * @param {string} shortDID
   * @param {any} embedVideo 
   * @param {{ url: string, title?: string }[] | undefined} embeds 
   */
  function addEmbedVideo(shortDID, embedVideo, embeds) {
    const url = getFeedVideoBlobUrl(shortDID, embedVideo?.video?.ref?.toString());
    if (url) {
      embeds = addToArray(embeds, {
        url,
        title: embedVideo?.alt || undefined
      });
    }
    return embeds;
  }

  /**
   * @param {string} shortDID
   * @param {any} embedExternal
   * @param {{ url: string, title?: string }[] | undefined} embeds 
   */
  function addEmbedExternal(shortDID, embedExternal, embeds) {
    if (!embedExternal?.uri) return embeds;
    const url = embedExternal.uri || undefined;
    if (!url) return embeds;
    return addToArray(embeds, {
      url,
      title: embedExternal.title || embedExternal.description || undefined,
      // imgSrc: getFeedBlobUrl(shortDID, embedExternal.thumb?.ref?.toString())
    });
  }

  /**
   * @param {any} embedRecord
   * @param {{ url: string, title?: string }[] | undefined} embeds 
   */
  function addEmbedRecord(embedRecord, embeds) {
    if (!embedRecord?.uri) return embeds;
    return addToArray(embeds, {
      url: embedRecord.uri
    });
  }

  /**
   * @param {string} shortDID
   * @param {any} embedRecordMedia
   * @param {{ url: string, title?: string }[] | undefined} embeds 
   */
  function addEmbedRecordMedia(shortDID, embedRecordMedia, embeds) {
    embeds = addEmbedImages(
      shortDID,
    /** @type {any} */(embedRecordMedia?.media)?.images,
      embeds);

    embeds = addEmbedVideo(
      shortDID,
    /** @type {any} */(embedRecordMedia?.media),
      embeds);

    embeds = addEmbedExternal(
      shortDID,
    /** @type {any} */(embedRecordMedia?.media)?.external,
      embeds);

    embeds = addEmbedRecord(
    /** @type {any} */(embedRecordMedia?.record)?.record,
      embeds);

    return embeds;
  }

  /**
   * @template T
   * @param {T[] | undefined} array
   * @param {T | undefined} element
   * @returns T[] | undefined
   */
  function addToArray(array, element) {
    if (!element) return array;
    if (!array) return [element];
    array.push(element);
    return array;
  }

  function getFeedBlobUrl(did, cid) {
    if (!did || !cid) return undefined;
    return `https://cdn.bsky.app/img/feed_thumbnail/plain/${unwrapShortDID(did)}/${cid}@jpeg`;
  }

  function getFeedVideoBlobUrl(did, cid) {
    if (!did || !cid) return undefined;
    return `https://video.bsky.app/watch/${unwrapShortDID(did)}/${cid}/thumbnail.jpg`;
  }

  /**
  * @param {string | null | undefined} url
  */
  function breakPostURL(url) {
    if (!url) return;
    const matchBsky = _breakBskyPostURL_Regex.exec(url);
    if (matchBsky) return { shortDID: shortenDID(matchBsky[1]), postID: matchBsky[2]?.toString().toLowerCase() };
    const matchGisting = _breakGistingPostURL_Regex.exec(url);
    if (matchGisting) return { shortDID: shortenDID(matchGisting[2]), postID: matchGisting[3]?.toString().toLowerCase() };
    const matchBskyStyle = _breakBskyStylePostURL_Regex.exec(url);
    if (matchBskyStyle) return { shortDID: shortenDID(matchBskyStyle[2]), postID: matchBskyStyle[3]?.toString().toLowerCase() };
  }
  const _breakBskyPostURL_Regex = /^http[s]?\:\/\/bsky\.app\/profile\/([a-z0-9\.\:\-]+)\/post\/([a-z0-9]+)(\/|$)/i;
  const _breakBskyStylePostURL_Regex = /^http[s]?\:\/\/(bsky\.app|6sky\.app|gist\.ing|gisti\.ng|gist\.ink)\/profile\/([a-z0-9\.\:\-]+)\/post\/([a-z0-9]+)(\/|$)/i;
  const _breakGistingPostURL_Regex = /^http[s]?\:\/\/(6sky\.app|gist\.ing|gisti\.ng|gist\.ink)\/([a-z0-9\.\:\-]+)\/([a-z0-9]+)(\/|$)/i;

  /** @param {string | null | undefined} text */
  function likelyDID(text) {
    return !!text && (
      !text.trim().indexOf('did:') ||
      text.trim().length === 24 && !/[^\sa-z0-9]/i.test(text)
    );
  }

  /**
   * @param {T} did
   * @returns {T}
   * @template {string | undefined | null} T
   */
  function shortenDID(did) {
    return did && /** @type {T} */(did.replace(_shortenDID_Regex, '').toLowerCase() || undefined);
  }

  const _shortenDID_Regex = /^did\:plc\:/;

  /**
   * @param {T} shortDID
   * @returns {T}
   * @template {string | undefined | null} T
   */
  function unwrapShortDID(shortDID) {
    return /** @type {T} */(
      !shortDID ? undefined : shortDID.indexOf(':') < 0 ? 'did:plc:' + shortDID.toLowerCase() : shortDID.toLowerCase()
    );
  }

  /**
   * @param {T} shortHandle
   * @returns {T}
   * @template {string | undefined | null} T
   */
  function unwrapShortHandle(shortHandle) {
    if (likelyDID(shortHandle)) return unwrapShortDID(shortHandle);
    shortHandle = cheapNormalizeHandle(shortHandle);
    return /** @type {T} */(
      !shortHandle ? undefined : shortHandle.indexOf('.') < 0 ? shortHandle.toLowerCase() + '.bsky.social' : shortHandle.toLowerCase()
    );
  }

  function cheapNormalizeHandle(handle) {
    handle = handle && handle.trim().toLowerCase();

    if (handle && handle.charCodeAt(0) === 64)
      handle = handle.slice(1);

    const urlprefix = 'https://bsky.app/';
    if (handle && handle.lastIndexOf(urlprefix, 0) === 0) {
      const postURL = breakPostURL(handle);
      if (postURL && postURL.shortDID)
        return postURL.shortDID;
    }

    if (handle && handle.lastIndexOf('at:', 0) === 0) {
      const feedUri = breakFeedURI(handle);
      if (feedUri && feedUri.shortDID)
        return feedUri.shortDID;

      if (handle && handle.lastIndexOf('at://', 0) === 0) handle = handle.slice(5);
      else handle = handle.slice(3);
    }

    return handle || undefined;
  }

  /**
   * Normalize and ensure handle follows atproto syntax rules.
   * This is a strict validator taken from @atproto/syntax semantics:
   * - lower-cases input
   * - requires domain-like handle (at least two labels)
   * - enforces label and overall length constraints and allowed chars
   * Throws on invalid handles.
   * @param {string} handle
   * @returns {string} normalized handle
   */
  function normalizeAndEnsureValidHandle(handle) {
    if (!handle) throw new Error('Handle is required');
    const normalized = String(handle).trim().toLowerCase();

    // overall length
    if (normalized.length > 253) throw new Error('Handle too long (253 chars max)');

    // must contain at least one dot (two labels)
    const labels = normalized.split('.');
    if (labels.length < 2) throw new Error('Handle must include a domain (e.g. alice.example)');

    // allowed characters: ASCII letters, digits, dashes, periods
    if (!/^[a-z0-9.-]*$/.test(normalized)) throw new Error('Invalid characters in handle');

    // per-label constraints
    for (let i = 0; i < labels.length; i++) {
      const l = labels[i];
      if (l.length < 1) throw new Error('Handle parts can not be empty');
      if (l.length > 63) throw new Error('Handle part too long (max 63 chars)');
      if (l.startsWith('-') || l.endsWith('-')) throw new Error('Handle parts can not start or end with hyphens');
      // final label (TLD) must start with ASCII letter
      if (i + 1 === labels.length && !/^[a-z]/.test(l)) throw new Error('Handle final component (TLD) must start with ASCII letter');
    }

    return normalized;
  }

  /**
  * @param {string | null | undefined} uri
  */
  function breakFeedURI(uri) {
    if (!uri) return;
    const match = _breakFeedUri_Regex.exec(uri);
    if (!match || !match[4]) return;
    if (match[3] === 'app.bsky.feed.post') return { shortDID: shortenDID(match[2]), postID: match[4] };
    return { shortDID: match[2], postID: match[4], feedType: match[3] };
  }
  const _breakFeedUri_Regex = /^at\:\/\/(did:plc:)?([a-z0-9]+)\/([a-z\.]+)\/?(.*)?$/;

  function makeFeedUri(shortDID, postID) {
    return 'at://' + unwrapShortDID(shortDID) + '/app.bsky.feed.post/' + postID;
  }


  async function localLogin() {
    try {
      const mcp = new McpServer();

      process.stdout.write('BlueSky login for ' + name + ' v' + version + ' MCP');
      const keytar = await keytarOrPromise;
      console.log();
      const login = readlineSync.question('   account: ');
      const password = readlineSync.question('  password: ', { hideEchoBack: true, mask: '*' });
      process.stdout.write('    access..');
      const feed = await mcp.tools.feed({ login, password });
      process.stdout.write('.');
      const profile = await mcp.tools.profile({ user: login });
      process.stdout.write('\n\nLogged in as @' + profile.handle + ' ' + profile.displayName);
      await keytar.setPassword(name, login, password);
      await keytar.setPassword(name, 'default_handle', login);
      console.log();
      if (feed.posts.length) {
        for (let i = 0; i < feed.posts.length && i < 4; i++) {
          const post = feed.posts[i];
          console.log('  ' + post.indexedAt + ' @' + post.author + ' ' + post.text.trim().split('\n')[0]);
        }
      }
      console.log('\nCredentials stored.');
    } catch (e) {
      console.error('Login failed:', e.message);
    }
  }

  async function printFeedPreview(params) {
    console.log();

    const mcp = new McpServer();
    const feed = await mcp.tools.feed({ limit: 100, ...params });
    const posts = feed.posts;
    console.log('Current feed:');
    const now = new Date();
    let output = [];
    posts.sort((a, b) => new Date(b.indexedAt).getTime() - new Date(a.indexedAt).getTime());
    for (const post of posts) {
      const dtPost = new Date(post.indexedAt);
      const dtStr =
        dtPost.toISOString().split('T')[0] === now.toISOString().split('T')[0] ?
          dtPost.toLocaleTimeString() :
          dtPost.toLocaleDateString();

      const text = post.text.trim().split('\n').filter(ln => ln.trim())[0];
      if (!text) continue;

      output.push(
        '  ' + dtStr.padStart(10) + ' ' + ('@' + post.author).padStart(31, output.length % 2 ? ' ' : '\u00B7 ') + '  ' + (text.length > 60 ? text.slice(0, 65) + '...' : text)
      );

      if (output.length > 20) break;
    }

    console.log(output.length ? output.join('\n') : 'No posts found in the feed.');
  }

  async function localInstall() {
    const settingsPath = path.join(os.homedir(), '.gemini', 'settings.json');
    console.log('>Installing ' + name + ' v' + version + ' MCP server');
    process.stdout.write('  for Gemini CLI ' + settingsPath);
    fs.mkdirSync(path.dirname(settingsPath), { recursive: true });
    process.stdout.write('.');
    let settingsJson = {};
    if (fs.existsSync(settingsPath)) {
      try { settingsJson = JSON.parse(fs.readFileSync(settingsPath, 'utf8')); } catch { }
    }
    process.stdout.write('.');

    settingsJson = {
      ...settingsJson,
      allowMCPServers: [
        'autoreply',
        ...(settingsJson.allowMCPServers || []).filter(server => server !== 'autoreply'),
      ],
      mcpServers: {
        ...settingsJson.mcpServers,
        autoreply: {
          ...settingsJson.mcpServers?.autoreply,
          type: 'stdio',
          command: 'node',
          args: [
            path.resolve(__filename)
          ],
        }
      }
    };

    process.stdout.write('.');
    fs.writeFileSync(settingsPath, JSON.stringify(settingsJson, null, 2));
    console.log(' OK');

    const mcpJsonPath =
      process.platform === 'win32' ?
        path.join(process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming'), 'Code', 'User', 'mcp.json') :
        process.platform === 'darwin' ?
          path.join(os.homedir(), 'Library', 'Application Support', 'Code', 'User', 'mcp.json') :
          !!process.env.CODESPACES || !!process.env.CODESPACE_NAME ?
            path.join(os.homedir(), '.vscode-remote', 'data', 'User', 'mcp.json') :
            path.join(os.homedir(), '.config', 'Code', 'User', 'mcp.json');
    process.stdout.write('  for VS Code    ' + mcpJsonPath + '..');
    fs.mkdirSync(path.dirname(mcpJsonPath), { recursive: true });
    process.stdout.write('.');

    let mcpJson = {};
    if (fs.existsSync(mcpJsonPath)) {
      try { mcpJson = JSON.parse(fs.readFileSync(mcpJsonPath, 'utf8')); } catch { }
    }

    mcpJson = {
      ...mcpJson,
      servers: {
        ...mcpJson.servers,
        autoreply: {
          ...mcpJson.servers?.autoreply,
          name: 'autoreply',
          type: 'stdio',
          command: 'node',
          args: [
            path.resolve(__filename)
          ]
        }
      }
    };

    fs.writeFileSync(mcpJsonPath, JSON.stringify(mcpJson, null, 2));
    console.log(' OK.');

    console.log('Successfully installed for ' + path.resolve(__filename));
  }

  async function runInteractive() {
    process.stdout.write(name + ' v' + version);
    const [_node, _script, cmd] = process.argv;
    if (cmd === 'install') {
      console.log();
      return localInstall();
    }

    const mcp = new McpServer();
    if (mcp[cmd]) {
      process.stdout.write('\n  MCP ' + JSON.stringify(cmd) + '...');
      const result = await mcp[cmd](params(cmd) || {});
      console.log(' ', result);
    } else if (mcp.tools[cmd]) {
      process.stdout.write('\n  MCP command ' + JSON.stringify(cmd) + '...');
      const result = await mcp.tools[cmd](params(cmd) || {});
      console.log(' ', result);
    } else {
      console.log(
        '\n' +
        (cmd ? 'Unknown command ' + cmd + '.\n' : '') +
        '\nAvailable commands:\n' +
        '  install - Installs the MCP server locally.\n' +
        getInfo(mcp).map(([key]) => '  ' + key + ' - MCP method').join('\n') + '\n' +
        getInfo(mcp.tools).map(([key, info]) => '  ' + key + (info ? ' - ' + info.description : ' - extra')).join('\n')
      );
      printFeedPreview(params(cmd));
    }

    function params(cmd) {
      if (process.argv.length < 4) return undefined;

      const raw = process.argv.slice(3).join(' ');

      // Try JSON first (e.g. '{"text":"hi"}' or '"string"')
      try {
        return JSON.parse(raw);
      } catch (e) {
        // ignore and try eval next
      }

      // Try eval for JS literals like ({a:1}) or [1,2]
      try {
        // eslint-disable-next-line no-eval
        return eval('(' + raw + ')');
      } catch (e) {
        // If eval fails (e.g., bare word like Microsoft), fall through to heuristics
      }

      // Heuristic fallback: if the tool has an input schema, map the bare string to a likely key
      try {
        const toolInfo = cmd ? mcp.tools[cmd + ':tool'] : undefined;
        const props = toolInfo?.inputSchema?.properties || {};
        if ('query' in props) return { query: raw };
        if ('text' in props) return { text: raw };
        if ('user' in props) return { user: raw };
        if ('postURI' in props) return { postURI: raw };
        if ('login' in props && 'password' in props) return { login: raw };
      } catch (e) {
        // ignore heuristics errors
      }

      // Default fallback: return an object with `query` so common commands like `search` work
      return { query: raw };
    }
  }


  if (require.main === module) {
    if (process.stdin.isTTY && process.stdout.isTTY) {
      runInteractive();
    } else {
      runMcp();
    }
  }

})();