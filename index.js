#!/usr/bin/env node
// @ts-check

const { name, version } = require('./package.json');

const fs = require('fs');
const path = require('path');
const prompt = require('prompt-sync')({ sigint: true });
const os = require('os');

const { AtpAgent } = require('@atproto/api');

const {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ToolSchema,
  RootsListChangedNotificationSchema,
} = require("@modelcontextprotocol/sdk/types.js");

const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio.js');

const { Server } = require('@modelcontextprotocol/sdk/server/index.js');

/**
 * @typedef {{
 *  setPassword(service: string, account: string, password: string): Promise<void>,
 *  getPassword(service: string, account: string): Promise<string>
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
    const tryPromise = keytarMod.getPassword(name, "default_handle");
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

/**
 * @param {{ handle?: string, password?: string }} args
 */
async function handleLogin({ handle, password }) {
  if (!handle || !password)
    throw new Error('Handle and password are required.');
  const keytar = await keytarOrPromise;

  await keytar.setPassword(name, handle, password);
  await keytar.setPassword(name, "default_handle", handle);
  return {
    content: [{
      type: 'text',
      text: 'Credentials stored and default handle set to ' + handle + '.'
    }]
  };
}

/**
 * @param {string} [handleImpersonate]
 */
async function getCredentials(handleImpersonate) {
  const keytar = await keytarOrPromise;

  let password;
  let handle = handleImpersonate;
  if (!handle) handle = await keytar.getPassword(name, "default_handle") || undefined;
  if (!handle) throw new Error('BlueSky login is required.');
  password = await keytar.getPassword(name, handle);
  if (!password) throw new Error('Password for ' + handle + ' is lost, please login again.');
  return { handle, password };
}

async function handlePost({ text, handle, password, replyToURI }) {
  if (!handle || !password) {
    [{handle, password}] = [await getCredentials(handle)];
  }

  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });
  let reply;
  let replyTracking;
  const postRef = breakPostURL(replyToURI) || breakFeedURI(replyToURI);
  if (postRef) {
    if (!likelyDID(postRef.shortDID)) {
      const resolved = await agent.resolveHandle({ handle: postRef.shortDID.replace('@', '') });
      postRef.shortDID = resolved.data.did;
    }

    const replyToPost = await agent.getPost({
      repo: unwrapShortDID(postRef.shortDID),
      rkey: postRef.postID
    });
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

  const posted = await agent.post({
    text,
    reply
  });

  return {
    content: [
      {
        type: 'text',
        text:
          replyTracking ? 'Replied to ' + replyTracking + ' with '  + posted.uri + ':\n' + text :
            replyToURI ? 'Could not split ' + JSON.stringify(replyToURI) + '/' + JSON.stringify(postRef) + ', posted alone ' + posted.uri + ':\n' + text :
              'Posted ' + posted.uri + ':\n' + text
      }
    ]
  };
}

async function handleFeed({ cursor, handle, password }) {
  const keytar = await keytarOrPromise;
  if (!handle) handle = await keytar.getPassword(name, "default_handle");
  if (handle === 'anonymous') handle = undefined;

  if (handle && !password) [{ password }] = [await getCredentials(handle)];

  let feed;
  if (!handle) {
    const agent = new AtpAgent({ service: 'https://api.bsky.app' });
    feed = await agent.app.bsky.feed.getFeed({
      feed: 'at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot',
      cursor
    });
  } else {
    const agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: handle, password });
    feed = await agent.getTimeline();
  }
  return {
    content: [
      {
        type: 'text',
        text:
          'cursor: ' + feed.data.cursor + '\n' +
          'feed:\n\n' + feed.data.feed.map(post =>
            post.post.indexedAt + ' @' + post.post.author.handle + (post.post.author.displayName ? ' ' + JSON.stringify(post.post.author.displayName) + ' ' : '') +  ' postURI: ' + post.post.uri + '\n' +
            post.post.record.text +
            (post.post.likeCount || post.post.replyCount || post.post.repostCount || post.post.quoteCount ?
              '\n(' +
              [
                post.post.likeCount ? post.post.likeCount + ' likes' : '',
                post.post.replyCount ? post.post.replyCount + ' replies' : '',
                post.post.repostCount ? post.post.repostCount + ' reposts' : '',
                post.post.quoteCount ? post.post.quoteCount + ' quotes' : ''
              ].filter(Boolean).join(', ') +
              ')'
              : '')
          ).join('\n\n')
      }
    ],
    structuredContent: {
      cursor: feed.data.cursor,
      posts: feed.data.feed.map(post => ({
        indexedAt: post.post.indexedAt,
        author: post.post.author.handle,
        authorName: post.post.author.displayName,
        postURI: post.post.uri,
        text: /** @type {string} */(post.post.record.text),
        likeCount: post.post.likeCount,
        replyCount: post.post.replyCount,
        repostCount: post.post.repostCount,
        quoteCount: post.post.quoteCount
      }))
    }
  };
}

async function handleProfile({ user, cursor }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const [followersCursor, followsCursor] = cursor ? JSON.parse(cursor) : [undefined, undefined];
  const [profile, followers, following] = await Promise.all([
    agent.getProfile({ actor: user }),
    agent.getFollowers({ actor: user, cursor: followersCursor }),
    agent.getFollows({ actor: user, cursor: followsCursor })
  ]);

  const structuredContent = {
    handle: profile.data.handle,
    displayName: profile.data.displayName,
    description: profile.data.description,
    createdAt: profile.data.createdAt,
    followersCount: profile.data.followersCount,
    followingCount: profile.data.followsCount,
    postsCount: profile.data.postsCount,
    followers: followers.data.followers.map((follower) => '@' + follower.handle),
    following: following.data.follows.map((follow) => '@' + follow.handle),
    cursor: JSON.stringify([followers.data.cursor, following.data.cursor])
  };

  return {
    content: [
      {
        type: 'text',
        text: `Profile: @${structuredContent.handle} (${structuredContent.displayName})\n\n` +
          'created: ' + structuredContent.createdAt + '\n\n' +
          `${structuredContent.description}\n\n` +
          `Followers: ${structuredContent.followersCount}, Following: ${structuredContent.followingCount}, Posts: ${structuredContent.postsCount}\n` +
          `\nFollowers:\n${structuredContent.followers.join(', ')}\n` +
          `\nFollowing:\n${structuredContent.following.join(', ')}`
      }
    ],
    structuredContent
  };
}

async function handleSearch({ from, query, handle, password, cursor }) {
  const keytar = await keytarOrPromise;
  if (!handle) handle = await keytar.getPassword(name, "default_handle");
  if (handle === 'anonymous') handle = undefined;

  if (handle && !password) [{ password }] = [await getCredentials(handle)];

  if (!query && !from) query = '*';

  let feed;
  if (!handle) {
    // Unauthenticated search: use public feed and filter
    const agent = new AtpAgent({ service: 'https://api.bsky.app' });
    feed = await agent.app.bsky.feed.searchPosts({
      q: query + (from ? ' from:' + from : ''),
      cursor
    });
  } else {
    // Authenticated search: get timeline and filter
    if (!password) [{ password }] = [await getCredentials(handle)];
    const agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: handle, password });
    feed = await agent.app.bsky.feed.searchPosts({
      q: query + (from ? ' from:' + from : ''),
      cursor
    });
  }

  return {
    content: [
      {
        type: 'text',
        text:
          'cursor: ' + feed.data.cursor + '\n' +
          'search feed:\n\n' +
          feed.data.posts.map(post =>
            post.indexedAt + ' @' + post.author.handle + (post.author.displayName ? ' ' + JSON.stringify(post.author.displayName) + ' ' : '') + ' postURI: ' + post.uri + '\n' +
            post.record.text +
            (post.likeCount || post.replyCount || post.repostCount || post.quoteCount ?
              '\n(' +
              [
                post.likeCount ? post.likeCount + ' likes' : '',
                post.replyCount ? post.replyCount + ' replies' : '',
                post.repostCount ? post.repostCount + ' reposts' : '',
                post.quoteCount ? post.quoteCount + ' quotes' : ''
              ].filter(Boolean).join(', ') +
              ')'
              : '')
          ).join('\n\n')
      }
    ],
    structuredContent: {
      cursor: feed.data.cursor,
      posts: feed.data.posts.map(post => ({
        indexedAt: post.indexedAt,
        author: post.author.handle,
        authorName: post.author.displayName,
        postURI: post.uri,
        text: post.record.text,
        likeCount: post.likeCount ?? null,
        replyCount: post.replyCount ?? null,
        repostCount: post.repostCount ?? null,
        quoteCount: post.quoteCount ?? null
      }))
    }
  };
}


async function handleLike({ postURI, handle, password }) {
  if (!postURI) throw new Error('postURI is required.');

  if (!handle || !password) {
    [{ handle, password }] = [await getCredentials(handle)];
  }

  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });

  const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
  if (!postRef) throw new Error('Invalid post URI or feed URI.');
  if (!likelyDID(postRef.shortDID)) {
    const resolved = await agent.resolveHandle({ handle: postRef.shortDID.replace('@', '') });
    postRef.shortDID = resolved.data.did;
  }

  const likePost = await agent.getPost({
    repo: unwrapShortDID(postRef.shortDID),
    rkey: postRef.postID
  });

  await agent.like(makeFeedUri(postRef.shortDID, postRef.postID), likePost.cid);
  return {
    content: [
      {
        type: 'text',
        text: `Post liked: ${postRef.shortDID}/${postRef.postID} (${likePost.uri}): ${likePost.value.text}`
      }
    ]
  };
}

async function handleRepost({ postURI, handle, password }) {
  if (!postURI) throw new Error('postURI is required.');

  if (!handle || !password) {
    [{ handle, password }] = [await getCredentials(handle)];
  }

  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });

  const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
  if (!postRef) throw new Error('Invalid post URI or feed URI.');
  if (!likelyDID(postRef.shortDID)) {
    const resolved = await agent.resolveHandle({ handle: postRef.shortDID.replace('@', '') });
    postRef.shortDID = resolved.data.did;
  }

  const repostPost = await agent.getPost({
    repo: unwrapShortDID(postRef.shortDID),
    rkey: postRef.postID
  });

  await agent.repost(makeFeedUri(postRef.shortDID, postRef.postID), repostPost.cid);
  return {
    content: [
      {
        type: 'text',
        text: `Post reposted: ${postRef.shortDID}/${postRef.postID} (${repostPost.uri}): ${repostPost.value.text}`
      }
    ]
  };
}

async function handleThreads({ postURI, handle, password }) {
  if (!postURI) throw new Error('postURI is required.');
  const keytar = await keytarOrPromise;
  if (!handle) handle = await keytar.getPassword(name, "default_handle");
  if (handle === 'anonymous') handle = undefined;
  if (handle && !password) [{ password }] = [await getCredentials(handle)];

  let agent;
  if (!handle) {
    agent = new AtpAgent({ service: 'https://api.bsky.app' });
  } else {
    agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: handle, password });
  }

  const postRef = breakPostURL(postURI) || breakFeedURI(postURI);
  if (postRef) {
    if (!likelyDID(postRef.shortDID)) {
      const resolved = await agent.resolveHandle({ handle: postRef.shortDID.replace('@', '') });
      postRef.shortDID = resolved.data.did;
    }

    postURI = makeFeedUri(postRef.shortDID, postRef.postID);
  }

  // Fetch thread
  const thread = await agent.app.bsky.feed.getPostThread({ uri: postURI });
  const anchorRecord = /** @type {import('@atproto/api').AppBskyFeedPost.Record} */(/** @type {*} */(
  /** @type {import('@atproto/api/dist/client/types/app/bsky/feed/defs').ThreadViewPost} */(thread.data.thread).post?.record));

  /**
   * @typedef {Omit<Partial<import('@atproto/api/dist/client/types/app/bsky/feed/defs').ThreadViewPost &
   *  Partial<import('@atproto/api/dist/client/types/app/bsky/feed/defs').NotFoundPost> &
   *  Partial<import('@atproto/api/dist/client/types/app/bsky/feed/defs').BlockedPost>>, '$type'> & { $type: string }} PostOrPlaceholder
   */

  /**
   * Flatten thread into array
   * @param {PostOrPlaceholder} [node]
   */
  function flattenThread(node) {
    /**
     * @type {{
     *  indexedAt: string,
     *  author: string,
     *  authorName?: string,
     *  postURI: string,
     *  replyToURI: string,
     *  text: unknown,
     *  likeCount?: number,
     *  replyCount?: number,
     *  repostCount?: number,
     *  quoteCount?: number
     * }[]}
     */
    const arr = [];
    if (!node) return arr;
    if (node.post) {
      const postData ={
        indexedAt: node.post.indexedAt,
        author: node.post.author.handle,
        authorName: node.post.author.displayName,
        postURI: node.post.uri,
        replyToURI: node.post.uri,
        text: node.post.record.text,
        likeCount: node.post.likeCount,
        replyCount: node.post.replyCount,
        repostCount: node.post.repostCount,
        quoteCount: node.post.quoteCount
      };
      arr.push(postData);
    }
    if (node.replies?.length) {
      for (const reply of node.replies) {
        arr.push(...flattenThread(reply));
      }
    }
    return arr;
  }
  const posts = flattenThread(thread.data.thread);

  // restore the context
  if (!posts.find(p => p.postURI === anchorRecord?.reply?.root?.uri)) {
    if (anchorRecord?.reply?.root?.uri) {
      const rootPost = await agent.app.bsky.feed.getPostThread({ uri: anchorRecord?.reply?.root?.uri });
      const updated = flattenThread(rootPost.data.thread);
      posts.unshift(...updated);
    }
  }

  return {
    content: [
      {
        type: 'text',
        text:
          posts.map(post =>
            post.indexedAt + ' @' + post.author + (post.author.displayName ? ' ' + JSON.stringify(post.author.displayName) + ' ' : '') + ' postURI: ' + post.postURI + ' in reply to postURI: ' + post.replyToURI + '\n' +
            post.text +
            (post.likeCount || post.replyCount || post.repostCount || post.quoteCount ?
              '\n(' +
              [
                post.likeCount ? post.likeCount + ' likes' : '',
                post.replyCount ? post.replyCount + ' replies' : '',
                post.repostCount ? post.repostCount + ' reposts' : '',
                post.quoteCount ? post.quoteCount + ' quotes' : ''
              ].filter(Boolean).join(', ') +
              ')'
              : '')
          ).join('\n\n')
      }
    ],
    structuredContent: {
      posts
    }
  };
}

async function handleDelete({ postURI, handle, password }) {
  if (!postURI || !handle || !password) throw new Error('postURI, handle, and password are required.');
  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });
  await agent.deletePost(postURI);
  return { content: { type: 'text', success: true, text: 'Post deleted' } };
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

function runMCP() {

  const server = new Server(
    { name, version },
    {
      capabilities: {
        tools: {
          post: ToolSchema,
          feed: ToolSchema,
          profile: ToolSchema,
          search: ToolSchema,
          threads: ToolSchema,
          delete: ToolSchema,
          like: ToolSchema,
          repost: ToolSchema,
          login: ToolSchema
        }
      }
    }
  );

  // Register MCP tools
  server.setRequestHandler(ListToolsRequestSchema, async (request) => {
    return {
      tools: [
        {
          name: "login",
          description: "Login and cache BlueSky handle and password.",
          inputSchema: {
            type: "object",
            properties: {
              handle: { type: "string", description: "Your BlueSky handle, who are you on BlueSky?" },
              password: { type: "string", description: "Your BlueSky app password (better not share it)." }
            },
            required: ["handle", "password"]
          }
        },
        {
          name: "post",
          description: "Post a message to BlueSky. Some people call these messages tweets or skeets or posts, same difference.",
          inputSchema: {
            type: "object",
            properties: {
              replyToURI: { type: "string", description: "The post URI (or BlueSky URL of the post) to which the reply is made (if any)." },
              text: { type: "string", description: "The text to post." },
              handle: { type: "string", description: "(Optional) BlueSky handle to post the message as." },
              password: { type: "string", description: "(Optional) BlueSky password to use." }
            },
            required: ["text"]
          }
        },
        {
          name: "feed",
          description:
            "Get the latest feed from BlueSky. " +
            "Returns a list of messages or tweets or posts or skeets however you call them. " +
            "If you want to see the latest posts from a specific user, just provide their handle. " +
            "These feeds are paginated, you get the top chunk and a cursor, you can call the same tool again with the cursor to get more posts.",
          inputSchema: {
            type: "object",
            properties: {
              handle: {
                type: "string", description:
                  "(Optional) BlueSky handle for which the feed is requested. " +
                  "If unspecified, or specified as anonymous, the feed will be retrieved in the incognito mode."
              },
              password: { type: "string", description: "(Optional) BlueSky password to use." },
              cursor: { type: "string", description: "(Optional) Cursor for pagination." }
            },
            required: []
          },
          outputSchema: {
            type: "object",
            properties: {
              cursor: { type: "string", description: "Cursor for pagination, if more data is available." },
              posts: {
                type: "array",
                items: {
                  type: "object",
                  properties: {
                    indexedAt: { type: "string", description: "ISO timestamp when the post was indexed." },
                    author: { type: "string", description: "BlueSky handle of the author." },
                    authorName: { type: "string", description: "Name of the author, if available." },
                    postURI: { type: "string", description: "URI of the post." },
                    text: { type: "string", description: "Text content of the post." },
                    likeCount: { type: "number", description: "Number of likes.", nullable: true },
                    replyCount: { type: "number", description: "Number of replies.", nullable: true },
                    repostCount: { type: "number", description: "Number of reposts.", nullable: true },
                    quoteCount: { type: "number", description: "Number of quotes.", nullable: true }
                  },
                  required: ["indexedAt", "author", "postURI", "text"]
                }
              }
            }
          }
        },
        {
          name: "profile",
          description: "Get profile details, followers, and following for an account.",
          inputSchema: {
            type: "object",
            properties: {
              user: { type: "string", description: "The handle of the user to get the profile for." },
              cursor: { type: "string", description: "(Optional) Cursor for pagination of followers/following." },
            },
            required: ["user"]
          },
          outputSchema: {
            type: "object",
            properties: {
              handle: { type: "string" },
              displayName: { type: "string" },
              description: { type: "string" },
              followersCount: { type: "number" },
              followingCount: { type: "number" },
              postsCount: { type: "number" },
              followers: { type: "array", items: { type: "string" } },
              following: { type: "array", items: { type: "string" } },
              cursor: { type: "string", description: "Cursor for pagination of followers/following." }
            }
          }
        },
        {
          name: "search",
          description:
            "Search messages (also known as posts, tweets, or skeets) on BlueSky by text query. " +
            "You can search for posts by text, or filter by author handle. " +
            "If you want to see messages from a specific user, just provide their handle. " +
            "That handle can also be a special value 'me' to indicate the authenticated user's posts. " +
            "These searches are paginated, you get the top chunk and a cursor, you can call the same tool again with the cursor to get more posts.",
          inputSchema: {
            type: "object",
            properties: {
              from: { type: "string", description: "(Optional) Messages from who, a handle or say 'me' for the user that's logged in." },
              query: { type: "string", description: "(Optional) Text to search for in messages. Here's an old blog post about search tricks, https://bsky.social/about/blog/05-31-2024-search but you can probably find more Googling, because these things change and improve often." },
              cursor: { type: "string", description: "(Optional) Cursor for pagination." },
              handle: { type: "string", description: "(Optional) BlueSky handle to use for authenticated search, anonymous to force unanuthenticated." },
              password: { type: "string", description: "(Optional) BlueSky password to use." }
            },
            required: []
          },
          outputSchema: {
            type: "object",
            properties: {
              cursor: { type: "string", description: "Cursor for pagination, if more data is available." },
              posts: {
                type: "array",
                items: {
                  type: "object",
                  properties: {
                    indexedAt: { type: "string", description: "ISO timestamp when the post was indexed." },
                    author: { type: "string", description: "BlueSky handle of the author." },
                    authorName: { type: "string", description: "Name of the author, if available." },
                    postURI: { type: "string", description: "URI of the post." },
                    text: { type: "string", description: "Text content of the post." },
                    likeCount: { type: "number", description: "Number of likes.", nullable: true },
                    replyCount: { type: "number", description: "Number of replies.", nullable: true },
                    repostCount: { type: "number", description: "Number of reposts.", nullable: true },
                    quoteCount: { type: "number", description: "Number of quotes.", nullable: true }
                  },
                  required: ["indexedAt", "author", "postURI", "text"]
                }
              }
            }
          }
        },
        {
          name: "threads",
          description:
            "Fetch a thread by post URI, it returns all the replies and replies to replies, the whole bunch. " +
            "If you're already logged in, this will fetch the thread as viewed by the logged in user (or you can provide handle/password directly). " +
            "If the handle is a special placeholder value 'anonymous', it will fetch the thread in incognito mode, " +
            "that sometimes yields more if your logged in account is blocked by other posters. " +
            "Note that messages in the thread are sometimes called skeets, tweets, or posts, but they are all the same thing.",
          inputSchema: {
            type: "object",
            properties: {
              postURI: { type: "string", description: "The BlueSky URL of the post, or also can be at:// URI of the post to fetch the thread for." },
              cursor: { type: "string", description: "(Optional) Cursor for pagination." },
              handle: { type: "string", description: "(Optional) BlueSky handle to use for authenticated fetch." },
              password: { type: "string", description: "(Optional) BlueSky password to use." }
            },
            required: ["postURI"]
          },
          outputSchema: {
            type: "object",
            properties: {
              cursor: { type: "string", description: "Cursor for pagination, if more data is available." },
              posts: {
                type: "array",
                items: {
                  type: "object",
                  properties: {
                    indexedAt: { type: "string", description: "ISO timestamp when the post was indexed." },
                    author: { type: "string", description: "BlueSky handle of the author." },
                    authorName: { type: "string", description: "Name of the author, if available." },
                    postURI: { type: "string", description: "URI of the post." },
                    replyToURI: { type: "string", description: "URI of the post being replied to, if any." },
                    text: { type: "string", description: "Text content of the post." },
                    likeCount: { type: "number", description: "Number of likes.", nullable: true },
                    replyCount: { type: "number", description: "Number of replies.", nullable: true },
                    repostCount: { type: "number", description: "Number of reposts.", nullable: true },
                    quoteCount: { type: "number", description: "Number of quotes.", nullable: true }
                  },
                  required: ["indexedAt", "author", "postURI", "text"]
                }
              }
            }
          }
        },
        {
          name: "delete",
          description: "Delete a post by URI (authenticated only).",
          inputSchema: {
            type: "object",
            properties: {
              postURI: { type: "string", description: "The URI of the post to delete." },
              handle: { type: "string", description: "(Optional) BlueSky handle to authenticate as, if not logged in already." },
              password: { type: "string", description: "(Optional) BlueSky password to use." }
            },
            required: ["postURI"]
          },
          outputSchema: {
            type: "object",
            properties: {
              success: { type: "boolean" },
              message: { type: "string" }
            },
            required: ["success", "message"]
          }
        },
        {
          name: "like",
          description: "Like a post by URI or BlueSky URL.",
          inputSchema: {
            type: "object",
            properties: {
              postURI: { type: "string", description: "The BlueSky URL or at:// URI of the post to like." },
              handle: { type: "string", description: "(Optional) BlueSky handle to authenticate as. Leave empty for already logged in user." },
              password: { type: "string", description: "(Optional) BlueSky password to use. Leave empty for already logged in user." }
            },
            required: ["postURI"]
          }
        },
        {
          name: "repost",
          description: "Repost a post by URI or BlueSky URL.",
          inputSchema: {
            type: "object",
            properties: {
              postURI: { type: "string", description: "The BlueSky URL or at:// URI of the post to repost." },
              handle: { type: "string", description: "(Optional) BlueSky handle to authenticate as. Leave empty for already logged in user." },
              password: { type: "string", description: "(Optional) BlueSky password to use. Leave empty for already logged in user." }
            },
            required: ["postURI"]
          }
        }
      ]
    };
  });

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    try {
      const { name, arguments = {} } = request.params;

      if (!name) throw new Error('Tool name is required.');

      switch (name) {
        case "login":
          return await handleLogin(/** @type {any} */(arguments));
        case "post":
          return await handlePost(/** @type {any} */(arguments));
        case "feed":
          return await handleFeed(/** @type {any} */(arguments));
        case "profile":
          return await handleProfile(/** @type {any} */(arguments));
        case "search":
          return await handleSearch(/** @type {any} */(arguments));
        case "delete":
          return await handleDelete(/** @type {any} */(arguments));
        case "threads":
          return await handleThreads(/** @type {any} */(arguments));
        case "like":
          return await handleLike(/** @type {any} */(arguments));
        case "repost":
          return await handleRepost(/** @type {any} */(arguments));
        default:
          throw new Error(`Tool ${name} is not supported.`);
      }
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: 'Error: ' + error.message,
            error: error.message
          }
        ],
        isError: true
      };
    }
  });

  // Handles post-initialization setup, specifically checking for and fetching MCP roots.
  server.oninitialized = async () => {
    const clientCapabilities = server.getClientCapabilities();

    if (clientCapabilities?.roots) {
      const response = await server.listRoots();
      // console.log(response);
    } else {
    }
  };

  const transport = new StdioServerTransport();
  server.connect(transport).then(() => {
  });

}

function getGeminiSettingsPath(globalMode) {
  if (globalMode) {
    const home = os.homedir();
    return path.join(home, '.gemini', 'settings.json');
  } else {
    return path.join(process.cwd(), '.gemini', 'settings.json');
  }
}

async function localInstall(globalMode = true) {
  const settingsPath = getGeminiSettingsPath(globalMode);
  process.stdout.write('Installing autoreply MCP to Gemini CLI at ' + settingsPath + '..');
  fs.mkdirSync(path.dirname(settingsPath), { recursive: true });
  process.stdout.write('.');
  let settingsJson = {};
  if (fs.existsSync(settingsPath)) {
    try { settingsJson = JSON.parse(fs.readFileSync(settingsPath, 'utf8')); } catch { }
  }

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
        command: 'node',
        args: [
          path.resolve(__filename)
        ],
      }
    }
  };

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
  process.stdout.write('Installing autoreply MCP to VSCode at ' + mcpJsonPath + '..');
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

  console.log('  autoreply MCP server at: ' + path.resolve(__filename));
}

async function localLogin() {
  try {
    const keytar = await keytarOrPromise;
    const handle = prompt('BlueSky handle: ');
    const password = prompt('BlueSky password: ', { echo: '' });
    await keytar.setPassword(name, handle, password);
    await keytar.setPassword(name, 'default_handle', handle);
    console.log('Login successful. Credentials stored.');
  } catch (e) {
    console.error('Login failed:', e.message);
  }
}

async function printFeedPreview() {
  console.log();
  const feed = await handleFeed({});
  const posts = feed.structuredContent.posts.slice(0, 10);
  console.log('Current feed:');
  const now = new Date();
  let output = [];
  for (const post of posts) {
    const dtPost = new Date(post.indexedAt);
    const dtStr =
      dtPost.toISOString().split('T')[0] === now.toISOString().split('T')[0] ?
        dtPost.toLocaleTimeString() :
        dtPost.toLocaleDateString();

    const text = post.text.trim().split('\n').filter(ln => ln.trim())[0];
    if (!text) continue;

    output.push(
      '  ' + dtStr + ' @' + post.author + (text.length > 60 ? text.slice(0, 55) + '...' : text)
    );

    if (output.length >= 15) break;
  }

  console.log(output.length ? output.join('\n') : 'No posts found in the feed.');
}

async function runInteractive() {
  const args = process.argv.slice(2);
  if (args[0] === 'install') {
    await localInstall(args[1] !== 'local');
  } else if (args[0] === 'login') {
    await localLogin();
  } else {
    console.log(name + ' MCP  v' + version);
    console.log('Usage: autoreply [install|login]');
    console.log('If no arguments, shows feed preview.');
    await printFeedPreview();
  }
}

if (require.main === module) {
  if (process.stdin.isTTY && process.stdout.isTTY) {
    runInteractive();
  } else {
    runMCP();
  }
}