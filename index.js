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
} = require('@modelcontextprotocol/sdk/types.js');

const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio.js');

const { Server } = require('@modelcontextprotocol/sdk/server/index.js');

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

const postSchema = {
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

/**
 * @param {{ handle?: string, password?: string }} args
 */
async function mcpLogin({ handle: login, password }) {
  if (!login || !password)
    throw new Error('Login handle and password are required.');
  const keytar = await keytarOrPromise;

  await keytar.setPassword(name, login, password);
  await keytar.setPassword(name, 'default_handle', login);
  return {
    content: [{
      type: 'text',
      text: 'Credentials stored and default handle set to ' + login + '.'
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
  if (!handle) handle = await keytar.getPassword(name, 'default_handle') || undefined;
  if (!handle) throw new Error('BlueSky login is required.');
  password = await keytar.getPassword(name, handle);
  if (!password) throw new Error('Password for ' + handle + ' is lost, please login again.');
  return { handle, password };
}

/**
 * @param {{
 *  cursor?: string,
 *  feed?: string,
 *  loginHandle?: string,
 *  password?: string
 * }} _
 */
async function mcpFeed({ cursor, feed, loginHandle, password }) {
  const keytar = await keytarOrPromise;
  if (!loginHandle) loginHandle = (await keytar.getPassword(name, 'default_handle')) || undefined;
  if (loginHandle === 'anonymous') loginHandle = undefined;

  if (loginHandle && !password) [{ password }] = [await getCredentials(loginHandle)];

  let feedData;
  let agent;
  if (!loginHandle) {
    agent = new AtpAgent({ service: 'https://api.bsky.app' });
  } else {
    agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({
      identifier: loginHandle,
      password: /** @type {string} */(password)
    });
  }

  if (feed || !loginHandle) {
    if (feed) {
      const fullFeedUri = breakFeedURI(feed);
      if (!fullFeedUri) {
        const likelyFeeds = await agent.app.bsky.unspecced.getPopularFeedGenerators({
          query: feed
        });
        if (likelyFeeds.data.feeds.length) {
          feed = likelyFeeds.data.feeds[0].uri;
        }
      }
    }

    feedData = (await agent.app.bsky.feed.getFeed({
      feed: feed || 'at://did:plc:z72i7hdynmk6r22z27h6tvur/app.bsky.feed.generator/whats-hot',
      cursor
    })).data;
  } else {
    feedData = (await agent.getTimeline()).data;
  }

  const formatted = /** @type {ReturnType<typeof formatPost>[]} */(feedData.feed.map(post =>
    !post.post ? undefined : formatPost(post.post)
  ).filter(Boolean));

  return {
    content: [
      {
        type: 'text',
        text:
          'cursor: ' + feedData.cursor + '\n' +
          'feed:\n\n' + formatted.map(post => post.textual).join('\n\n')
      }
    ],
    structuredContent: {
      cursor: feedData.cursor,
      posts: formatted.map(post => post.structured)
    }
  };
}

async function mcpProfile({ user, cursor }) {
  const agent = new AtpAgent({ service: 'https://api.bsky.app' });
  const [followersCursor, followsCursor] = cursor ? JSON.parse(cursor) : [undefined, undefined];

  if (likelyDID(user)) {
    user = unwrapShortDID(user);
  } else {
    user = unwrapShortHandle(user);

    if (/\s/.test(user) || !/\./.test(user)) {
      // need to search for the user
      const actors = await agent.searchActors({
        q: user
      });
      if (actors.data.actors.length) {
        user = actors.data.actors[0].did;
      }
    }

  }

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
    avatar: profile.data.avatar,
    banner: profile.data.banner,
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
          (structuredContent.avatar ? `Avatar: ${structuredContent.avatar}\n` : '') +
          (structuredContent.banner ? `Banner: ${structuredContent.banner}\n` : '') +
          `Followers: ${structuredContent.followersCount}, Following: ${structuredContent.followingCount}, Posts: ${structuredContent.postsCount}\n` +
          `\nFollowers:\n${structuredContent.followers.join(', ')}\n` +
          `\nFollowing:\n${structuredContent.following.join(', ')}`
      }
    ],
    structuredContent
  };
}

async function mcpSearch({ from, query, login, password, cursor }) {
  const keytar = await keytarOrPromise;
  if (!login) login = await keytar.getPassword(name, 'default_handle');
  if (login === 'anonymous') login = undefined;

  if (login && !password) [{ password }] = [await getCredentials(login)];

  if (!query && !from) query = '*';

  let agent;
  if (!login) {
    agent = new AtpAgent({ service: 'https://api.bsky.app' });
  } else {
    if (!password) [{ password }] = [await getCredentials(login)];
    agent = new AtpAgent({ service: 'https://bsky.social' });
    await agent.login({ identifier: login, password });
  }

  if (from) {
    if (likelyDID(from)) {
      const resolved = await agent.getProfile({ actor: unwrapShortDID(from) });
      from = resolved.data.handle
    } else {
      from = unwrapShortHandle(from);
    }
  }

  let feed;
  if (!login) {
    // Unauthenticated search: use public feed and filter
    feed = await agent.app.bsky.feed.searchPosts({
      q: (query || '') + (from ? ' from:' + from : ''),
      cursor
    });
  } else {
    feed = await agent.app.bsky.feed.searchPosts({
      q: (query || '') + (from ? ' from:' + from : ''),
      cursor
    });
  }

  const formatted = feed.data.posts.map(post => formatPost(post));

  return {
    content: [
      {
        type: 'text',
        text:
          'cursor: ' + feed.data.cursor + '\n' +
          'search feed:\n\n' +
          formatted.map(post => post.textual).join('\n\n')
      }
    ],
    structuredContent: {
      cursor: feed.data.cursor,
      posts: formatted.map(post => post.structured)
    }
  };
}

async function mcpPost({ text, handle, password, replyToURI }) {
  if (!handle || !password) {
    [{ handle, password }] = [await getCredentials(handle)];
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
          replyTracking ? 'Replied to ' + replyTracking + ' with ' + posted.uri + ':\n' + text :
            replyToURI ? 'Could not split ' + JSON.stringify(replyToURI) + '/' + JSON.stringify(postRef) + ', posted alone ' + posted.uri + ':\n' + text :
              'Posted ' + posted.uri + ':\n' + text
      }
    ]
  };
}

async function mcpLike({ postURI, handle, password }) {
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

async function mcpRepost({ postURI, handle, password }) {
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

async function mcpThread({ postURI, handle, password }) {
  if (!postURI) throw new Error('postURI is required.');
  const keytar = await keytarOrPromise;
  if (!handle) handle = await keytar.getPassword(name, 'default_handle');
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
          posts.map(post => post.text).join('\n\n')
      }
    ],
    structuredContent: {
      posts
    }
  };
}

async function mcpDelete({ postURI, handle, password }) {
  if (!postURI || !handle || !password) throw new Error('postURI, handle, and password are required.');
  const agent = new AtpAgent({ service: 'https://bsky.social' });
  await agent.login({ identifier: handle, password });
  await agent.deletePost(postURI);
  return { content: { type: 'text', success: true, text: 'Post deleted' } };
}

/**
 * @param {Omit<import('@atproto/api/dist/client/types/app/bsky/feed/defs').FeedViewPost['post'], '$type'>} post
 */
function formatPost(post) {
  /** @type {Partial<import('@atproto/api').AppBskyFeedPost.Record>} */
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
 * @param {import('@atproto/api').AppBskyFeedPost.Record['embed'] | undefined} embed
 */
function extractEmbeds(shortDID, embed) {
  if (!embed) return;

  /** @type {{ url: string, title?: string }[] | undefined} */
  let embeds = undefined;

  embeds = addEmbedImages(shortDID, /** @type {import('@atproto/api').AppBskyEmbedImages.Main} */(embed).images, embeds);
  embeds = addEmbedVideo(shortDID, /** @type {import('@atproto/api').AppBskyEmbedVideo.Main} */(embed), embeds);
  embeds = addEmbedExternal(shortDID, /** @type {import('@atproto/api').AppBskyEmbedExternal.Main} */(embed).external, embeds);
  embeds = addEmbedRecord(/** @type {import('@atproto/api').AppBskyEmbedRecord.Main} */(embed).record, embeds);
  embeds = addEmbedRecordMedia(shortDID, /** @type {import('@atproto/api').AppBskyEmbedRecordWithMedia.Main} */(embed), embeds);

  return embeds;
}

/**
 * @param {string} shortDID
 * @param {import('@atproto/api').AppBskyEmbedImages.Main['images'] | undefined} embedImages 
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
 * @param {import('@atproto/api').AppBskyEmbedVideo.Main | undefined} embedVideo 
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
 * @param {import('@atproto/api').AppBskyEmbedExternal.Main['external'] | undefined} embedExternal
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
 * @param {import('@atproto/api').AppBskyEmbedRecord.Main['record'] | undefined} embedRecord
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
 * @param {import('@atproto/api').AppBskyEmbedRecordWithMedia.Main | undefined} embedRecordMedia
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedRecordMedia(shortDID, embedRecordMedia, embeds) {
  embeds = addEmbedImages(
    shortDID,
    /** @type {import('@atproto/api').AppBskyEmbedImages.Main} */(embedRecordMedia?.media)?.images,
    embeds);

  embeds = addEmbedVideo(
    shortDID,
    /** @type {import('@atproto/api').AppBskyEmbedVideo.Main} */(embedRecordMedia?.media),
    embeds);

  embeds = addEmbedExternal(
    shortDID,
    /** @type {import('@atproto/api').AppBskyEmbedExternal.Main} */(embedRecordMedia?.media)?.external,
    embeds);

  embeds = addEmbedRecord(
    /** @type {import('@atproto/api').AppBskyEmbedRecord.Main} */(embedRecordMedia?.record)?.record,
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
          thread: ToolSchema,
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
          name: 'login',
          description: 'Login and cache BlueSky handle and password.',
          inputSchema: {
            type: 'object',
            properties: {
              login: { type: 'string', description: 'Your BlueSky handle, who are you on BlueSky?' },
              password: { type: 'string', description: 'Your BlueSky app password (better not share it).' }
            },
            required: ['handle', 'password']
          }
        },
        {
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
        },
        {
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
              cursor: { type: 'string', description: '(Optional) Cursor for pagination.' }
            },
            required: []
          },
          outputSchema: {
            type: 'object',
            properties: {
              cursor: { type: 'string', description: 'Cursor for pagination, if more data is available.' },
              posts: {
                type: 'array',
                items: postSchema
              }
            }
          }
        },
        {
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
        },
        {
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
              password: { type: 'string', description: '(Optional) BlueSky password to use.' }
            },
            required: []
          },
          outputSchema: {
            type: 'object',
            properties: {
              cursor: { type: 'string', description: 'Cursor for pagination, if more data is available.' },
              posts: {
                type: 'array',
                items: postSchema
              }
            }
          }
        },
        {
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
              cursor: { type: 'string', description: '(Optional) Cursor for pagination.' },
              login: { type: 'string', description: '(Optional) BlueSky handle to use for authenticated fetch.' },
              password: { type: 'string', description: '(Optional) BlueSky password to use.' }
            },
            required: ['postURI']
          },
          outputSchema: {
            type: 'object',
            properties: {
              cursor: { type: 'string', description: 'Cursor for pagination, if more data is available.' },
              posts: {
                type: 'array',
                items: postSchema
              }
            }
          }
        },
        {
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
        },
        {
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
        },
        {
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
        }
      ]
    };
  });

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    try {
      const { name, arguments = {} } = request.params;

      if (!name) throw new Error('Tool name is required.');

      switch (name) {
        case 'login':
          return await mcpLogin(/** @type {any} */(arguments));
        case 'post':
          return await mcpPost(/** @type {any} */(arguments));
        case 'feed':
          return await mcpFeed(/** @type {any} */(arguments));
        case 'profile':
          return await mcpProfile(/** @type {any} */(arguments));
        case 'search':
          return await mcpSearch(/** @type {any} */(arguments));
        case 'delete':
          return await mcpDelete(/** @type {any} */(arguments));
        case 'thread':
          return await mcpThread(/** @type {any} */(arguments));
        case 'like':
          return await mcpLike(/** @type {any} */(arguments));
        case 'repost':
          return await mcpRepost(/** @type {any} */(arguments));
        default:
          throw new Error(`Tool ${name} is not supported.`);
      }
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: 'Error: ' + error.message + ' for ' + name + ' v' + version,
            error: error.stack
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
    const login = prompt('BlueSky handle: ');
    const password = prompt('BlueSky password: ', { echo: '' });
    await keytar.setPassword(name, login, password);
    await keytar.setPassword(name, 'default_handle', login);
    console.log('Login successful. Credentials stored.');
  } catch (e) {
    console.error('Login failed:', e.message);
  }
}

async function printFeedPreview() {
  console.log();
  const feed = await mcpFeed({});
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