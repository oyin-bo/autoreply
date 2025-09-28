// @ts-check

const PostSchema = {
  type: 'object',
  properties: {
    indexedAt: { type: 'string', description: 'ISO timestamp when the post was indexed by BlueSky.' },
    author: { type: 'string', description: 'BlueSky handle of the post author (e.g., user.bsky.social).' },
    authorName: { type: 'string', description: 'Display name of the author, if available.' },
    postURI: { type: 'string', description: 'Unique AT Protocol URI of the post (at://did:plc:.../app.bsky.feed.post/...).' },
    replyToURI: { type: 'string', description: 'URI of the post being replied to, if this is a reply.' },
    text: { type: 'string', description: 'Text content of the post.' },
    likeCount: { type: 'number', description: 'Number of likes this post has received.', nullable: true },
    replyCount: { type: 'number', description: 'Number of replies this post has received.', nullable: true },
    repostCount: { type: 'number', description: 'Number of times this post has been reposted.', nullable: true },
    quoteCount: { type: 'number', description: 'Number of times this post has been quoted.', nullable: true },
    links: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          url: { type: 'string', description: 'URL of the embedded link.' },
          title: { type: 'string', description: 'Title or description of the linked content, if available.' }
        }
      },
      description: 'List of embedded media and links in the post (images, videos, external URLs, quoted posts, etc.).'
    }
  }
};

module.exports = PostSchema;