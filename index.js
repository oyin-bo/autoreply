// @ts-check
const { AtpAgent } = require('@atproto/api');
const keytar = require('keytar');
const { Server } = require('@modelcontextprotocol/sdk/server/index.js');
const {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ToolSchema,
  RootsListChangedNotificationSchema,
} = require("@modelcontextprotocol/sdk/types.js");

const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio.js');
const { name, version } = require('./package.json');

const server = new Server(
  { name, version },
  {
    capabilities: {
      tools: {
        post: ToolSchema,
        feed: ToolSchema,
        followers: ToolSchema,
        following: ToolSchema,
        search: ToolSchema,
        delete: ToolSchema,
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
        description: "Post a message to BlueSky.",
        inputSchema: {
          type: "object",
          properties: {
            replyToURI: { type: "string", description: "The post URI to which the reply is made (if any)." },
            text: { type: "string", description: "The text to post." },
            handle: { type: "string", description: "(Optional) BlueSky handle to post the message as." },
            password: { type: "string", description: "(Optional) BlueSky password to use." }
          },
          required: ["text"]
        }
      },
      {
        name: "feed",
        description: "Get the latest feed from BlueSky.",
        inputSchema: {
          type: "object",
          properties: {
            handle: {
              type: "string", description:
                "(Optional) BlueSky handle for which the feed is requested. " +
                "If unspecified, or specified as anonymous, the feed will be retrieved in the incognito mode."
            },
            password: { type: "string", description: "(Optional) BlueSky password to use." }
          },
          required: []
        },
        outputSchema: {
          type: "object",
          properties: {
            posts: {
              type: "array",
              items: {
                type: "object",
                properties: {
                  indexedAt: { type: "string", description: "ISO timestamp when the post was indexed." },
                  author: { type: "string", description: "BlueSky handle of the author." },
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
        name: "followers",
        description: "Get followers for a user.",
        inputSchema: {
          type: "object",
          properties: {
            user: { type: "string", description: "The handle of the user to get followers for." }
          },
          required: ["user"]
        }
      },
      {
        name: "following",
        description: "Get following list for a user.",
        inputSchema: {
          type: "object",
          properties: {
            user: { type: "string", description: "The handle of the user to get following for." }
          },
          required: ["user"]
        }
      },
      {
        name: "search",
        description: "Search posts on BlueSky by text query.",
        inputSchema: {
          type: "object",
          properties: {
            from: { type: "string", description: "(Optional) Messages from who, a handle or say 'me' for self." },
            query: { type: "string", description: "(Optional) Text to search for in posts." },
            handle: { type: "string", description: "(Optional) BlueSky handle to use for authenticated search, anonymous to force unanuthenticated." },
            password: { type: "string", description: "(Optional) BlueSky password to use." }
          },
          required: []
        },
        outputSchema: {
          type: "object",
          properties: {
            posts: {
              type: "array",
              items: {
                type: "object",
                properties: {
                  indexedAt: { type: "string", description: "ISO timestamp when the post was indexed." },
                  author: { type: "string", description: "BlueSky handle of the author." },
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
      }

    ]
  };
});

/**
 * @param {{ handle?: string, password?: string }} args
 */
async function handleLogin({ handle, password }) {
  if (!handle || !password)
    throw new Error('Handle and password are required.');

  await keytar.setPassword(name, handle, password);
  await keytar.setPassword(name, "default_handle", handle);
  return { content: "Credentials stored and default handle set." };
}

/**
 * @param {string} [handle]
 */
async function getCredentials(handle) {
  if (!handle) handle = await keytar.getPassword(name, "default_handle") || undefined;
  if (!handle) throw new Error('Handle and password for BlueSky are required.');

  const password = await keytar.getPassword(name, handle);
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

  await agent.post({
    text,
    reply
  });

  return {
    content: [
      {
        type: 'text',
        text:
          replyTracking ? 'Replied to ' + replyTracking + ':\n' + text :
            replyToURI ? 'Could not split ' + JSON.stringify(replyToURI) + '/' + JSON.stringify(postRef) + ', posted alone:\n' + text :
              'Posted:\n' + text
      }
    ]
  };
}

async function handleFeed({ handle, password }) {
  if (!handle) handle = await keytar.getPassword(name, "default_handle");
  if (handle === 'anonymous') handle = undefined;

  if (handle && !password) [{ password }] = [await getCredentials(handle)];

  let posts;
  if (!handle) {
    const agent = new AtpAgent({ service: 'https://api.bsky.app' });
    const feed = await agent.app.bsky.feed.getFeed({
      feed: 'at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot'
    });
    posts = feed.data.feed;
  } else {
    const agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: handle, password });
    const feed = await agent.getTimeline();
    posts = feed.data.feed;
  }
  return {
    content: [
      {
        type: 'text',
        text:
          posts.map(post =>
            post.post.indexedAt + ' @' + post.post.author.handle + ' postURI: ' + post.post.uri + '\n' +
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
      posts: posts.map(post => ({
        indexedAt: post.post.indexedAt,
        author: post.post.author.handle,
        postURI: post.post.uri,
        text: post.post.record.text,
        likeCount: post.post.likeCount ?? null,
        replyCount: post.post.replyCount ?? null,
        repostCount: post.post.repostCount ?? null,
        quoteCount: post.post.quoteCount ?? null
      }))
    }
  };
}

async function handleFollowers({ user }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const profile = await agent.getProfile({ actor: user });
  const followers = await agent.getFollowers({ actor: profile.data.did });
  return {
    content: [{
      type: 'text',
      text:
        'Followers (' + followers.data.followers.length + '):\n' +
        followers.data.followers.map((follower) => '@' + follower.handle).join(', ')
    }]
  };
}

async function handleFollowing({ user }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const profile = await agent.getProfile({ actor: user });
  const following = await agent.getFollows({ actor: profile.data.did });
  return {
    content: [{
      type: 'text',
      text:
        'Following (' + following.data.follows.length + '):\n' +
        following.data.follows.map((follow) => '@' + follow.handle).join(', ')
    }]
  };
}

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  try {
    const { name, arguments = {} } = request.params;

    if (!name) throw new Error('Tool name is required.');

    switch (name) {
      case "login":
        return await handleLogin(arguments);
      case "post":
        return await handlePost(arguments);
      case "feed":
        return await handleFeed(arguments);
      case "followers":
        return await handleFollowers(arguments);
      case "following":
        return await handleFollowing(arguments);
      case "search":
        return await handleSearch(arguments);
      case "delete":
        return await handleDelete(arguments);
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

async function handleSearch({ from, query, handle, password }) {
  if (!handle) handle = await keytar.getPassword(name, "default_handle");
  if (handle === 'anonymous') handle = undefined;

  if (handle && !password) [{ password }] = [await getCredentials(handle)];

  if (!query && !from) query = '*';

  let posts;
  if (!handle) {
    // Unauthenticated search: use public feed and filter
    const agent = new AtpAgent({ service: 'https://api.bsky.app' });
    const feed = await agent.app.bsky.feed.searchPosts({
      q: query + ( from ? ' from:' + from : '' ),
    });
    posts = feed.data.posts;
  } else {
    // Authenticated search: get timeline and filter
    if (!password) [{ password }] = [await getCredentials(handle)];
    const agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: handle, password });
    const feed = await agent.app.bsky.feed.searchPosts({
      q: query + ( from ? ' from:' + from : '' ),
    });
    posts = feed.data.posts;
  }

  return {
    content: [
      {
        type: 'text',
        text:
          posts.map(post =>
            post.indexedAt + ' @' + post.author.handle + ' postURI: ' + post.uri + '\n' +
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
      posts: posts.map(post => ({
        indexedAt: post.indexedAt,
        author: post.author.handle,
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

async function handleDelete({ postURI, handle, password }) {
  if (!postURI || !handle || !password) throw new Error('postURI, handle, and password are required.');
  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });
  await agent.deletePost(postURI);
  return { content: { type: 'text', success: true, text: 'Post deleted' } };
}

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
