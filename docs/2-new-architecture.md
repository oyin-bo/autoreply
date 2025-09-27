# Autoreply v2

## Tools

The tools are shaped by functional use, not what the API offers.

Feed merged with search is the most used tool, because the biggest need served by **autoreply** is to get posts en masse to analyse, summarise, gauge temperature etc.

Converation/thread is a unit of interaction, and should provide contents in a packaged easy to consume format for the AI.

Posting is a distinct activity, and so is liking and deleting.

Drill-down tools for post or profile details are probably needed on more ad-hoc basis. Batching for these is not an obvious need.

- `feed` - streamed feed/search, paginated, cache-optimized for recency
- `thread` - snapshot of thread/post with all replies and context, plus basic engagement metrics
- `post_details` - drill-down into engagement: all likers, quotes, retweets of a post, paginated
- `profile` - user profile with followers/following/blocked/lists - attempt mining CAR downloads for efficiency
- `post` - content creation: posting/retweeting/quoting with auto-splitting for length, reaction support
- `delete` - content removal with batch support for multiple posts
- `like` - engagement actions with batch support for multiple posts
- `login` - authentication via app password or OAuth

## Data Model

BlueSky data is too rich and verbose for LLM in most cases. **Autoreply** will store the raw BlueSky data in a cache, but expose a slim, minimalist, focused natural content to the LLM.

### CAR/CBOR

Natural fit for account-clustered data. Compact, coarse-chunked and very accessible in BlueSky APIs. Will be used extensively for a large extent of the data cached.

May want to repack from CAR to enriched format for fusing with other sources?

### AppView sugar

AppView provides valuable aggregations not easily derived from account-clustered CARs. We will store that in a separate cache.

Generally this sugar is either extra metrics per post, or sets of posts.

* threads - sets of posts: let's keep URI only and propagate post into "partial" CARs
* likes/reposts - metrics per post from thread/feed post views
* feeds - sequence of posts: let's keep URI only

### Semantic metrics

For searching purpose it is useful to have RAG over this cache. We will consider implementation details of this, and see how it can be achieved.

A promising option is to use Model2Vec to extract embeddings for posts and profiles, with 64 dimensions and 8-bit quantization. May need a toy project to explore/validate.