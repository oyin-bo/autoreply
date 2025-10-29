# Post and React Tools

This document describes the `post` and `react` tools added to the Go server implementation.

## Post Tool

Create new posts on Bluesky with optional reply functionality.

### Usage

```json
{
  "text": "Hello, Bluesky!",
  "postAs": "alice.bsky.social",  // optional, defaults to authenticated account
  "replyTo": "at://did:plc:abc/app.bsky.feed.post/xyz"  // optional
}
```

### Parameters

- **text** (required): The text content of the post
- **postAs** (optional): Handle or DID to post as. Defaults to the default authenticated account.
- **replyTo** (optional): URI or URL of the post to reply to. Accepts both:
  - AT Protocol URIs: `at://did:plc:abc/app.bsky.feed.post/xyz`
  - Bluesky URLs: `https://bsky.app/profile/alice.bsky.social/post/xyz`

### Reply Handling

When replying to a post:
- The tool automatically resolves the reply chain
- Properly sets both `root` and `parent` references
- Maintains thread context for nested replies

### Example

Simple post:
```json
{
  "text": "Just setting up my Bluesky account!"
}
```

Reply to a post:
```json
{
  "text": "Great point!",
  "replyTo": "https://bsky.app/profile/bob.bsky.social/post/abc123"
}
```

## React Tool

Perform batch reactions on posts: like, unlike, repost, or delete.

### Usage

```json
{
  "reactAs": "alice.bsky.social",  // optional
  "like": [
    "at://did:plc:abc/app.bsky.feed.post/xyz",
    "https://bsky.app/profile/bob.bsky.social/post/123"
  ],
  "repost": [
    "at://did:plc:def/app.bsky.feed.post/456"
  ],
  "delete": [
    "at://did:plc:myid/app.bsky.feed.post/789"  // only your own posts
  ]
}
```

### Parameters

- **reactAs** (optional): Handle or DID to react as. Defaults to the default authenticated account.
- **like** (optional): Array of post URIs to like
- **unlike** (optional): Array of post URIs to unlike (remove like)
- **repost** (optional): Array of post URIs to repost
- **delete** (optional): Array of post URIs to delete (only works for your own posts)

### Features

- **Batching**: Process multiple operations in a single call
- **Partial Success**: Each operation succeeds or fails independently
- **Mixed Formats**: Can mix AT URIs and HTTP URLs in the same request
- **Detailed Results**: Returns markdown with success/failure status for each operation

### Example

Like multiple posts:
```json
{
  "like": [
    "at://did:plc:abc/app.bsky.feed.post/xyz",
    "https://bsky.app/profile/alice.bsky.social/post/123"
  ]
}
```

Mixed operations:
```json
{
  "like": ["at://did:plc:abc/app.bsky.feed.post/xyz"],
  "repost": ["https://bsky.app/profile/bob.bsky.social/post/456"],
  "delete": ["at://did:plc:myid/app.bsky.feed.post/old"]
}
```

## URI Format Support

Both tools accept URIs in two formats:

### AT Protocol URI Format
```
at://did:plc:abc123/app.bsky.feed.post/xyz789
```

### Bluesky Web URL Format
```
https://bsky.app/profile/alice.bsky.social/post/xyz789
https://bsky.app/profile/did:plc:abc123/post/xyz789
```

Also supported:
- `https://gist.ing/profile/...` 
- Other Bluesky-compatible URL formats

## Authentication

Both tools require authentication:

1. **Default Account**: If `postAs`/`reactAs` is not specified, uses the default authenticated account
2. **Specific Account**: Specify a handle or DID to post/react as a specific account
3. **Login Required**: You must login first using the `login` tool

Example login:
```json
{
  "handle": "alice.bsky.social",
  "password": "your-app-password"
}
```

Or with OAuth (recommended):
```json
{
  "handle": "alice.bsky.social"
}
```

## Error Handling

### Post Tool
- Returns error if text is missing or empty
- Returns error if no authenticated account found
- Returns error if reply target cannot be resolved
- Returns error if API call fails

### React Tool
- Returns error if no operations specified
- Returns error if no authenticated account found
- Individual operations fail independently (partial success)
- Delete validates post ownership
- Returns detailed markdown showing success/failure for each operation

## Output Format

Both tools return markdown-formatted results:

### Post Tool Output
```markdown
# Post Created

**Posted:** Hello, Bluesky!

**Post URI:** at://did:plc:myid/app.bsky.feed.post/xyz

**Posted as:** @alice.bsky.social
```

### React Tool Output
```markdown
# Reaction Results

**Acting as:** @alice.bsky.social

**Summary:** 2 of 3 operations successful

## Likes

✅ Liked: at://did:plc:abc/app.bsky.feed.post/xyz
❌ Failed to like at://did:plc:def/app.bsky.feed.post/bad: invalid post record: missing CID

## Reposts

✅ Reposted: https://bsky.app/profile/bob.bsky.social/post/123
```

## Implementation Notes

- Built on AT Protocol standards
- Uses authenticated API calls via `com.atproto.repo.createRecord`, `deleteRecord`, etc.
- Properly handles DID resolution for handles
- Validates post ownership for delete operations
- Supports reply threading with root/parent tracking
- Comprehensive error messages for debugging
