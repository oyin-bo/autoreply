// @ts-check

import { Client, CredentialManager, simpleFetchHandler, ok } from '@atcute/client';

import {
  addToArray,
  getFeedBlobUrl,
  getFeedVideoBlobUrl,
  breakPostURL,
  likelyDID,
  shortenDID,
  unwrapShortDID,
  unwrapShortHandle,
  cheapNormalizeHandle,
  breakFeedURI,
  makeFeedUri
} from './core.js';
import createProxyAwareFetch from './fetch-proxied.js';
import keytarOrPromise from './keytar.js';
import PostSchema from './post-schema.js';

import package_json from '../package.json';


// Create proxy-aware fetch function
const proxyAwareFetch = createProxyAwareFetch();


class Tools {

  /**
 * @param {{ login?: string, password?: string }} args
 */
  async login({ login, password }) {
    if (!login || !password)
      throw new Error('Login handle and password are required.');

    await this.clientLogin({ login, password });

    return {
      success: true,
      message: 'Credentials stored and default handle set to ' + login + '.',
      handle: login,
      text: `Successfully logged in as @${login} and stored credentials.`
    };
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
    },
    outputSchema: {
      type: 'object',
      properties: {
        success: { type: 'boolean', description: 'Whether the login was successful.' },
        message: { type: 'string', description: 'Success message confirming credentials were stored.' },
        handle: { type: 'string', description: 'The handle that was logged in.' }
      },
      required: ['success', 'message', 'handle']
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
          feed: /** @type {*} */(feed) || 'at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot',
          cursor,
          limit
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

    const posts = formatted.map(post => post.structured);

    return {
      cursor: feedData.cursor,
      posts,
      text: 'Feed returned at ' + feedData.cursor
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
      user = /** @type {string} */(unwrapShortHandle(user));

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

    /**
     * @type {[
     *  import('@atcute/bluesky').AppBskyActorGetProfile.$output,
     *  import('@atcute/bluesky').AppBskyGraphGetFollowers.$output,
     *  import('@atcute/bluesky').AppBskyGraphGetFollows.$output
     * ]}
     */
    const [profile, followers, following] = await Promise.all([
      ok(agent.get('app.bsky.actor.getProfile', { params: { actor: /** @type {*} */(user) } })),
      ok(agent.get('app.bsky.graph.getFollowers', { params: { actor: /** @type {*} */(user), cursor: followersCursor } })),
      ok(agent.get('app.bsky.graph.getFollows', { params: { actor: /** @type {*} */(user), cursor: followsCursor } }))
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

    const profileText =
      '@' + structuredContent.handle +
      (structuredContent.displayName ? ' ' + structuredContent.displayName.replace(/\s+/g, ' ').trim() : '') +
      (structuredContent.description ? '/ ' + structuredContent.description.replace(/\s+/g, ' ').trim() + ' ' : '') +
      structuredContent.followersCount + ' followers, ' +
      structuredContent.followingCount + ' following, ' +
      structuredContent.postsCount + ' posts' +
      (structuredContent.createdAt ?
        ' created: ' + new Date(structuredContent.createdAt) :
        '');

    return {
      ...structuredContent,
      text: profileText
    };
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
        handle: { type: 'string', description: 'The user\'s BlueSky handle (e.g., user.bsky.social).' },
        displayName: { type: 'string', description: 'The display name of the account, tends to be short one line name, but longer than handle.' },
        description: { type: 'string', description: 'The description or bio of the account, tends to have some general info about the account, bragging rights and other info.' },
        avatar: { type: 'string', format: 'uri', description: 'URL to the profile icon (avatar).' },
        banner: { type: 'string', format: 'uri', description: 'URL to the profile banner image, usually a broad rectangle.' },
        followersCount: { type: 'number', description: 'Total number of users following this account.' },
        followingCount: { type: 'number', description: 'Total number of users this account is following.' },
        postsCount: { type: 'number', description: 'Total number of posts made by this account.' },
        followers: { type: 'array', items: { type: 'string' }, description: 'List of handles of users following this account (paginated).' },
        following: { type: 'array', items: { type: 'string' }, description: 'List of handles of users this account is following (paginated).' },
        cursor: { type: 'string', description: 'Cursor for pagination of followers/following.' }
      }
    }
  };

  /**
   * @param {{
   *   from?: string,
   *   query?: string,
   *   login?: string,
   *   password?: string,
   *   cursor?: string,
   *   limit?: number
   * }} _
   */
  async search({ from, query, login, password, cursor, limit }) {

    // Set default query if neither query nor from is provided
    if (!query && !from) query = '*';

    // fallback to thread if query is post link
    {
      const postRef = breakPostURL(query) || breakFeedURI(query);
      if (postRef) {
        const threadResult = await this.thread({ postURI: /** @type {*} */(query), login, password });
        if (threadResult.posts.length) {
          const author = threadResult.posts?.[0]?.author;
          if (shortenDID(author?.did) === shortenDID(from) || author?.handle === from)
            return threadResult;

          const filtered = threadResult.posts.filter(p => {
            const author = p.author;
            return shortenDID(author?.did) === shortenDID(from) || author?.handle === from;
          });
          return {
            posts: filtered
          };
        }
      }
    }

    // Get appropriate client (authenticated if possible, incognito as fallback)
    const agent = await this.clientLoginOrFallback({ login, password });

    const [searchCursor, feedCursor] = cursor ? cursor.split('<<SPLIT>>') : [undefined, undefined];

    const feedFetchPromise = !from || from === '*' ? undefined :
      this.feed({
        cursor: feedCursor,
        login,
        password,
        feed: from,
        limit
      });

    // Normalize `from` to a handle when possible
    if (from) {
      if (likelyDID(from)) {
        try {
          const resolved = await ok(agent.get('app.bsky.actor.getProfile', { params: { actor: /** @type {*} */(unwrapShortDID(from)) } }));
          from = resolved.handle;
        } catch (e) {
          // If resolution fails, fall back to cheap unwrap/normalization
          from = unwrapShortHandle(from);
        }
      } else {
        from = unwrapShortHandle(from);
      }
    }

    const params = {
      q: (query || '') + (from ? ' from:' + from : ''),
      cursor: searchCursor,
      limit
    };

    // Make the search request  
    const searchOutput = await ok(agent.get('app.bsky.feed.searchPosts', { params }));
    const formatted = /** @type {ReturnType<typeof formatPost>[]} */(/** @type {any} */(searchOutput).posts.map((/** @type {any} */ post) => formatPost(post)));

    const feedOutput = await feedFetchPromise?.catch(() => undefined);

    let combinedCursor = /** @type {any} */(searchOutput).cursor;
    if (feedOutput?.cursor) combinedCursor += '<<SPLIT>>' + feedOutput.cursor;
    let combinedPosts = formatted.map((/** @type {any} */ post) => post.structured);
    if (feedOutput?.posts?.length) {
      combinedPosts = combinedPosts.concat(feedOutput.posts);
    }

    return {
      cursor: combinedCursor,
      posts: combinedPosts,
      text: 'Search cursor ' + combinedCursor.split(/\s+/g).join('/')
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
        postRef.shortDID = await this._resolveHandle(postRef.shortDID);
      }

      postURI = makeFeedUri(postRef.shortDID, postRef.postID);
    }

    // Fetch thread
    const thread = await ok(agent.get('app.bsky.feed.getPostThread', { params: { uri: /** @type {*} */(postURI) } }));
    // Use a generic any type for record shapes so we don't depend on @atproto/api types at runtime
    const anchorRecord = /** @type {any} */(thread.thread).post?.record;

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
    if (!posts.find(p => p.structured.postURI === anchorRecord?.reply?.root?.uri)) {
      if (anchorRecord?.reply?.root?.uri) {
        const agentFallback = this.clientIncognito();
        const rootPost = await ok(agentFallback.get('app.bsky.feed.getPostThread', { params: { uri: anchorRecord?.reply?.root?.uri } }));
        const updated = flattenThread(rootPost.thread);
        posts.unshift(...updated);
      }
    }

    const structuredPosts = posts.map(post => post.structured);

    return {
      posts: structuredPosts,
      text: 'Thread with ' + structuredPosts.length + ' posts'
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

  /**
   * @type {{ [handle: string]: string | Promise<string> }}
   */
  _resolvedHandleCache = {};

  /**
   * @param {string} handle
   */
  _resolveHandle(handle) {
    if (likelyDID(handle)) return handle;

    const existing = this._resolvedHandleCache[handle];
    if (existing) return existing;

    return this._resolvedHandleCache[handle] = (async () => {
      const agent = this.clientIncognito();
      /** @type {*} */
      const resolved = await ok(agent.get(
        /** @type {*} */('com.atproto.identity.resolveHandle'),
        { params: { handle: /** @type {*} */(handle).replace('@', '') } }
      ));
      return this._resolvedHandleCache[handle] = resolved.did;
    })();

  }

  /**
   * @param {{
   *  text: string,
   *  login?: string,
   *  password?: string,
   *  replyToURI?: string
   * }} _
   */
  async post({ text, login, password, replyToURI }) {

    const agent = await this.clientLoginRequired({ login, password });
    let reply;
    let replyTracking;
    const postRef = breakPostURL(replyToURI) || breakFeedURI(replyToURI);
    if (postRef) {
      if (!likelyDID(postRef.shortDID))
        postRef.shortDID = await this._resolveHandle(postRef.shortDID);

      /** @type {*} */
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
      input: {
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

    const messageText = replyTracking ? 'Replied to ' + replyTracking + ' with ' + /** @type {any} */(posted).uri + ':\n' + text :
      replyToURI ? 'Could not split ' + JSON.stringify(replyToURI) + '/' + JSON.stringify(postRef) + ', posted alone ' + /** @type {any} */(posted).uri + ':\n' + text :
        'Posted ' + /** @type {any} */(posted).uri + ':\n' + text;

    const summaryText = replyToURI ?
      `Replied to post with: "${text}"` :
      `Posted: "${text}"`;

    return {
      success: true,
      postURI: /** @type {any} */(posted).uri,
      text: summaryText,
      message: messageText,
      isReply: !!replyToURI,
      replyToURI: replyToURI || null
    };
  }

  'post:tool' = {
    name: 'post',
    description: 'Post a message to BlueSky. Some people call these messages tweets or skeets or posts, same difference.',
    inputSchema: {
      type: 'object',
      properties: {
        text: { type: 'string', description: 'The text of the post to send.' },
        replyToURI: { type: 'string', description: 'The post URI (or BlueSky URL of the post) to which the reply is made (if any).' },
        login: { type: 'string', description: '(Optional) BlueSky handle to post the message as.' },
        password: { type: 'string', description: '(Optional) BlueSky password to use.' }
      },
      required: ['text']
    },
    outputSchema: {
      type: 'object',
      properties: {
        success: { type: 'boolean', description: 'Whether the post was successful.' },
        postURI: { type: 'string', description: 'URI of the created post.' },
        message: { type: 'string', description: 'Success message with details.' },
        isReply: { type: 'boolean', description: 'Whether this was a reply to another post.' },
        replyToURI: { type: 'string', nullable: true, description: 'URI of the post being replied to, if any.' }
      },
      required: ['success', 'postURI', 'text', 'message', 'isReply', 'replyToURI']
    }
  };

  /**
   * @param {{
   *  postURI: string,
   *  login?: string,
   *  password?: string
   * }} _
   */
  async like({ postURI, login, password }) {
    if (!postURI) throw new Error('postURI is required.');

    const agent = await this.clientLoginRequired({ login, password });

    const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
    if (!postRef) throw new Error('Invalid post URI or feed URI.');
    if (!likelyDID(postRef.shortDID)) {
      postRef.shortDID = await this._resolveHandle(postRef.shortDID);
    }

    /** @type {*} */
    const likePost = await ok(agent.get('com.atproto.repo.getRecord', {
      params: {
        repo: unwrapShortDID(postRef.shortDID),
        collection: 'app.bsky.feed.post',
        rkey: postRef.postID
      }
    }));

    const myDid = agent.manager?.session?.did;
    if (!myDid) throw new Error('No authenticated session found');

    await ok(agent.post('com.atproto.repo.createRecord', {
      input: {
        repo: myDid,
        collection: 'app.bsky.feed.like',
        record: {
          $type: 'app.bsky.feed.like',
          subject: {
            uri: makeFeedUri(postRef.shortDID, postRef.postID),
            cid: likePost.cid
          },
          createdAt: new Date().toISOString()
        }
      }
    }));

    return {
      success: true,
      postURI: makeFeedUri(postRef.shortDID, postRef.postID),
      postText: likePost.value.text,
      message: `Post liked: ${postRef.shortDID}/${postRef.postID} (${likePost.uri}): ${likePost.value.text}`,
      author: postRef.shortDID,
      text: `Liked post by @${postRef.shortDID}: "${likePost.value.text.substring(0, 50)}${likePost.value.text.length > 50 ? '...' : ''}"`
    };
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
    },
    outputSchema: {
      type: 'object',
      properties: {
        success: { type: 'boolean', description: 'Whether the like was successful.' },
        postURI: { type: 'string', description: 'URI of the post that was liked.' },
        postText: { type: 'string', description: 'Text content of the post that was liked.' },
        message: { type: 'string', description: 'Success message with details.' },
        author: { type: 'string', description: 'Author of the post that was liked.' }
      },
      required: ['success', 'postURI', 'postText', 'message', 'author']
    }
  };

  /**
   * @param {{
   *  postURI: string,
   *  login?: string,
   *  password?: string
   * }} _
   */
  async repost({ postURI, login, password }) {
    if (!postURI) throw new Error('postURI is required.');

    const agent = await this.clientLoginRequired({ login, password });

    const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
    if (!postRef) throw new Error('Invalid post URI or feed URI.');
    if (!likelyDID(postRef.shortDID)) {
      postRef.shortDID = await this._resolveHandle(postRef.shortDID);
    }

    /** @type {*} */
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
      input: {
        repo: myDid,
        collection: 'app.bsky.feed.repost',
        record: {
          $type: 'app.bsky.feed.repost',
          subject: { uri: makeFeedUri(postRef.shortDID, postRef.postID) },
          createdAt: new Date().toISOString()
        }
      }
    }));

    return {
      success: true,
      postURI: makeFeedUri(postRef.shortDID, postRef.postID),
      postText: repostPost.value.text,
      message: `Post reposted: ${postRef.shortDID}/${postRef.postID} (${repostPost.uri}): ${repostPost.value.text}`,
      author: postRef.shortDID,
      text: `Reposted post by @${postRef.shortDID}: "${repostPost.value.text.substring(0, 50)}${repostPost.value.text.length > 50 ? '...' : ''}"`
    };
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
    },
    outputSchema: {
      type: 'object',
      properties: {
        success: { type: 'boolean', description: 'Whether the repost was successful.' },
        postURI: { type: 'string', description: 'URI of the post that was reposted.' },
        postText: { type: 'string', description: 'Text content of the post that was reposted.' },
        message: { type: 'string', description: 'Success message with details.' },
        author: { type: 'string', description: 'Author of the post that was reposted.' }
      },
      required: ['success', 'postURI', 'postText', 'message', 'author']
    }
  };

  /**
   * @param {{
   *  postURI: string,
   *  login?: string,
   *  password?: string
   * }} _
   */
  async delete({ postURI, login, password }) {
    if (!postURI) throw new Error('postURI is required.');


    const agent = await this.clientLoginRequired({ login, password });

    // Parse the URI to get repo and collection details
    const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
    if (postRef) {
      const myDid = agent.manager?.session?.did;
      if (!myDid) throw new Error('No authenticated session found');

      await ok(/** @type {any} */(agent).post('com.atproto.repo.deleteRecord', {
        input: {
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
        input: {
          repo,
          collection,
          rkey
        }
      }));
    }

    return {
      success: true,
      postURI: postURI,
      message: 'Post deleted',
      text: `Successfully deleted post: ${postURI}`
    };
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
        success: { type: 'boolean', description: 'Whether the deletion was successful.' },
        postURI: { type: 'string', description: 'URI of the post that was deleted.' },
        message: { type: 'string', description: 'Success message confirming the deletion.' }
      },
      required: ['success', 'postURI', 'message']
    }
  };

  /**
   * @param {{ login?: string, password?: string }} _
   * @returns {Promise<Client & { authenticated?: boolean, manager?: CredentialManager }>}
   */
  async clientLoginOrFallback({ login, password }) {
    const keytar = await keytarOrPromise;
    if (!login) login = (await keytar.getPassword(package_json.name, 'default_handle')) || undefined;
    // Treat explicit 'anonymous' as no-login
    if (login === 'anonymous') login = undefined;

    // Validate the stored default handle to avoid trying to derive a service from invalid values
    if (login && !likelyDID(login))
      login = await this._resolveHandle(login);

    // Only attempt to read the saved password if we have a login
    if (login) {
      password = password || /** @type {string} */(await keytar.getPassword(package_json.name, login));
      try {
        return await this.clientLogin({ login, password: /** @type {string} */(password) });
      } catch (e) {
        // If login fails for any reason, fall back to incognito rather than crashing the feed
        console.error('Login failed for', login, (password || '').slice(0, 2) + '***', '- falling back to incognito:', e?.message || e);
        return this.clientIncognito();
      }
    }

    return this.clientIncognito();
  }

  /**
   * @param {{ login?: string, password?: string }} _
   * @returns {Promise<Client & { authenticated?: boolean, manager?: CredentialManager }>}
   */
  async clientLoginRequired({ login, password }) {
    if (!login || !password) {
      const creds = await this.getCredentials(login);
      login = creds.handle;
      password = creds.password;
    }

    try {
      return await this.clientLogin({ login, password });
    } catch (e) {
        /** @type {Error} */(e).message = 'Authentication failed for ' + login + '/' + (password || '').slice(0, 2) + '***' + '. ' + (/** @type {Error} */(e)?.message || e);
      console.error('Authentication failed for', login, '- this operation requires login.');
      throw e;
    }
  }

  /**
   * @type {Record<string, Client & { authenticated?: boolean, manager?: CredentialManager }>}
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

    const manager = new CredentialManager({ service, fetch: proxyAwareFetch });
    const rpc = /** @type {Client & { authenticated?: boolean, manager?: CredentialManager }} */(new Client({ handler: manager }));

    await manager.login({ identifier: login, password });

    // store credentials
    const keytar = await keytarOrPromise;
    await keytar.setPassword(package_json.name, login, password);
    await keytar.setPassword(package_json.name, 'default_handle', login);

    rpc.authenticated = true;
    rpc.manager = manager;

    this._clientLoggedInByHandle[login] = rpc;
    return rpc;
  }

  /**
   * @type {Client | undefined}
   */
  _clientIncognito;

  clientIncognito() {
    if (this._clientIncognito) return this._clientIncognito;

    // Use the project's public read endpoint (not environment-overridable).
    const service = 'https://public.api.bsky.app';
    const handler = simpleFetchHandler({ service, fetch: proxyAwareFetch });
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
    if (!handle) handle = await keytar.getPassword(package_json.name, 'default_handle') || undefined;
    if (!handle) throw new Error('BlueSky login is required.');
    password = await keytar.getPassword(package_json.name, handle);
    if (!password) throw new Error('Password for ' + handle + ' is lost, please login again.');
    return { handle, password };
  }

}

/**
 * @param {any} post
 */
export function formatPost(post) {
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

  let links = extractEmbeds(post.author.did, postRecord.embed);

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
    const url = getFeedBlobUrl(shortDID, img.image?.ref?.$link);
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
  const url = getFeedVideoBlobUrl(shortDID, embedVideo?.video?.ref?.$link);
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
export default Tools;