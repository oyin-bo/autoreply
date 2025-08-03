

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
        following: ToolSchema
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
                "If unspecified, or specified as anonymous, the feed will be retrieved for the authenticated user."
            },
            password: { type: "string", description: "(Optional) BlueSky password to use." }
          },
          required: []
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
  return { toolResult: "Credentials stored and default handle set." };
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
  await agent.post({
    text,
    reply: replyToURI ? { root: replyToURI, parent: replyToURI } : undefined
  });
  return { toolResult: "Post successful!" };
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
    toolResult: posts.map(post =>
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
    )
  };
}

async function handleFollowers({ user }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const profile = await agent.getProfile({ actor: user });
  const followers = await agent.getFollowers({ actor: profile.data.did });
  const handles = followers.data.followers.map((follower) => follower.handle);
  return { toolResult: handles };
}

async function handleFollowing({ user }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const profile = await agent.getProfile({ actor: user });
  const following = await agent.getFollows({ actor: profile.data.did });
  const handles = following.data.follows.map((follow) => follow.handle);
  return { toolResult: handles };
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
      default:
        throw new Error(`Tool ${name} is not supported.`);
    }
  } catch (error) {
    return { toolResult: { error: error.message }, isError: true };
  }
});

// Handles post-initialization setup, specifically checking for and fetching MCP roots.
server.oninitialized = async () => {
  const clientCapabilities = server.getClientCapabilities();

  if (clientCapabilities?.roots) {
    const response = await server.listRoots();
    console.log(response);
  } else {
  }
};

const transport = new StdioServerTransport();
server.connect(transport).then(() => {
});
