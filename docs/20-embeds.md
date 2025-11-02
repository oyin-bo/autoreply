# Plan: Enhanced Markdown Rendering for Posts with Embeddings and Facets

## Overview

Currently, the autoreply server returns basic text content for posts with **facet processing now implemented** for mentions, links, and hashtags. This plan outlines the remaining work to enhance Markdown rendering with rich media embeddings.

## ✅ COMPLETED: Facet Processing (Phase 1)

**Status:** ✅ Implemented and tested (26 Rust tests + 8 Go tests passing)

Facets (text annotations) are now fully processed and converted to Markdown in **both Rust and Go implementations**:

- **Mentions** (`@username.bsky.social`): Rendered as `[@username](https://bsky.app/profile/username)`
- **Links** (URLs in text): Rendered as `[url text](https://url.com)`
- **Hashtags** (`#tag`): Rendered as `[#tag](https://bsky.app/hashtag/tag)`
- **UTF-8 safe**: Properly handles emoji and multi-byte characters
- **Byte-indexed**: Correctly processes AT Protocol's byte-based facet indices

### Implementation Details

#### Rust Implementation
- Module: `rust-server/src/tools/post_format.rs`
- Functions:
  - `apply_facets_to_text()`: Applies facets to text with proper byte-to-char conversion
  - `format_facet_feature()`: Formats individual facet types as Markdown links
  - `blockquote_content_with_facets()`: Convenience function combining facets + blockquoting
- Integration:
  - ✅ Search results (`tools/search.rs`)
  - ✅ Thread display (`tools/thread.rs`)
  - ✅ Feed display (`tools/feed.rs`)
- Tests: 26 passing (including UTF-8/emoji edge cases)

#### Go Implementation
- Module: `go-server/internal/tools/postformat.go`
- Functions:
  - `ApplyFacetsToText()`: Applies facets to text with proper byte indexing
  - `formatFacetFeature()`: Formats individual facet types as Markdown links
  - `BlockquoteContentWithFacets()`: Convenience function combining facets + blockquoting
- Integration:
  - ✅ Search results (`tools/search.go`)
  - ✅ Thread display (`tools/thread.go`)
  - ✅ Feed display (`tools/feed.go`)
- Tests: 8 passing (including mention, link, hashtag, emoji cases)

## Example Current Output

```markdown
@pfrazee.com/3lfax6juhxk2v
> Yeah that's a super common limitation of crypto-structure networks like atproto, but we were pretty [**car**eful](https://example.com) not to let that happen
2025-01-08T19:58:28Z
```

## Example Target Output (After Embed Implementation)

```markdown
@pfrazee.com/3lfax6juhxk2v
> Yeah that's a super common limitation of crypto-structure networks like [@atproto](https://bsky.app/profile/atproto), but we were pretty [**car**eful](https://example.com) not to let that happen
>
> ![image descr](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc.../cid@jpeg)
> [link title](...)
> [link title
> link description](...)
2025-01-08T19:58:28Z
```

## Remaining Work: Embeds (Phases 2-4)

## Bluesky Content Types to Support

### 1. ✅ Facets (Text Annotations) - IMPLEMENTED

Facets are inline annotations in the post text that add semantic meaning:

#### 1.1 ✅ Mentions (`app.bsky.richtext.facet#mention`)
- **Status:** ✅ Implemented
- Format: `@username.bsky.social` or `@did:plc:...`
- Render as: `[@username](https://bsky.app/profile/username)`
- Extract from: `post.facets[].features[]` where `$type` is mention

#### 1.2 ✅ Links (`app.bsky.richtext.facet#link`)
- **Status:** ✅ Implemented
- Format: Inline URLs in text
- Render as: `[visible text](https://url.com)`
- Extract from: `post.facets[].features[]` where `$type` is link
- Use `byteStart` and `byteEnd` to identify position in text

#### 1.3 ✅ Hashtags (`app.bsky.richtext.facet#tag`)
- **Status:** ✅ Implemented
- Format: `#hashtag`
- Render as: `[#hashtag](https://bsky.app/hashtag/hashtag)`
- Extract from: `post.facets[].features[]` where `$type` is tag

#### 1.4 Rich Text Formatting
- **Status:** 🔮 Future
- Bold, italic, code (if supported in future Bluesky versions)
- Currently limited, but plan for extensibility

### 2. Embeds (Rich Media Attachments) - TODO

Embeds are structured attachments to posts:

#### 2.1 Images (`app.bsky.embed.images`)
- Structure: `post.embed.images[]` with `alt`, `image.ref`, `aspectRatio`
- Render as: `![alt text](https://cdn.bsky.app/img/feed_thumbnail/...)`
- Support multiple images (up to 4)
- Include aspect ratio info if available

#### 2.2 External Links (`app.bsky.embed.external`)
- Structure: `post.embed.external` with `uri`, `title`, `description`, `thumb`
- Render as:
  ```markdown
  [link title](url)
  > link description
  > ![thumb](thumb_url)
  ```

#### 2.3 Record Embeds (Quote Posts) (`app.bsky.embed.record`)
- Structure: `post.embed.record` with nested post data
- Render as nested quote block:
  ```markdown
  > **@author**: quoted text
  > 2025-01-08T...
  ```
- Handle recursive embeds (quoted post with its own embeds)

#### 2.4 Record with Media (`app.bsky.embed.recordWithMedia`)
- Combines record embed with images or external links
- Render both components

#### 2.5 Video (`app.bsky.embed.video`) - Future
- Currently limited support in Bluesky
- Plan for: `![video alt](video_url)`

### 3. Post Metadata

- Author handle and DID
- Timestamp (ISO 8601)
- AT URI
- Reply/thread context (if applicable)
- Like/repost counts (optional)

## Implementation Plan

### ✅ Phase 1: Facet Processing - COMPLETED

**Status:** ✅ Implemented and tested

1. **✅ Parse facets from post record**
   - Extract `post.record.facets` array
   - Map byte ranges to UTF-8 character positions (properly handles emoji)

2. **✅ Apply facets to text**
   - Sort facets by `byteStart` to process in order
   - Build Markdown string with inline formatting
   - Handle overlapping facets gracefully

3. **✅ Render different facet types**
   - Mentions: Link to profile (`[@handle](https://bsky.app/profile/handle)`)
   - Links: Markdown link format (`[text](url)`)
   - Hashtags: Link to hashtag search (`[#tag](https://bsky.app/hashtag/tag)`)
   - Handle edge cases (malformed facets, invalid ranges)

4. **✅ Integration complete**
   - Search results with facets
   - Thread display with facets
   - Feed display with facets
   - 26 passing tests including UTF-8/emoji handling

### Phase 2: Basic Embeds - TODO

1. **Images**
   - Extract image refs from `embed.images`
   - Build blob URLs: `https://cdn.bsky.app/img/feed_fullsize/plain/{did}/{cid}@jpeg`
   - Render with alt text
   - Support multiple images (grid layout hint in Markdown)

2. **External links**
   - Extract from `embed.external`
   - Render title, description, and thumbnail
   - Format as distinct block after main text

### Phase 3: Advanced Embeds

1. **Quote posts**
   - Recursively render embedded records
   - Limit recursion depth (e.g., max 3 levels)
   - Format as nested quote blocks

2. **Record with media**
   - Combine quote post rendering with media rendering
   - Maintain visual hierarchy

### Phase 4: Polish and Edge Cases

1. **URL resolution**
   - Implement blob URL construction
   - Handle CDN variations (thumbnail vs fullsize)
   - Add error handling for missing blobs

2. **Text encoding**
   - Proper UTF-8 byte range to char index conversion
   - Handle emoji and multi-byte characters
   - Validate facet ranges

3. **Markdown escaping**
   - Escape special Markdown characters in text
   - Prevent injection in user content
   - Handle code blocks, quotes properly

4. **Performance**
   - Cache blob URLs if needed
   - Optimize facet application algorithm
   - Minimize string allocations

## Data Structures

### Facet Structure (AT Protocol)
```json
{
  "index": {
    "byteStart": 0,
    "byteEnd": 10
  },
  "features": [
    {
      "$type": "app.bsky.richtext.facet#mention",
      "did": "did:plc:..."
    }
  ]
}
```

### Image Embed Structure
```json
{
  "$type": "app.bsky.embed.images",
  "images": [
    {
      "alt": "Description",
      "image": {
        "ref": { "$link": "bafkreicid..." },
        "mimeType": "image/jpeg",
        "size": 123456
      },
      "aspectRatio": { "width": 16, "height": 9 }
    }
  ]
}
```

### External Link Embed Structure
```json
{
  "$type": "app.bsky.embed.external",
  "external": {
    "uri": "https://example.com",
    "title": "Link Title",
    "description": "Link description text",
    "thumb": {
      "ref": { "$link": "bafkreicid..." },
      "mimeType": "image/jpeg",
      "size": 12345
    }
  }
}
```

## Code Locations

### Rust Implementation
- **✅ Facet processing**: `rust-server/src/tools/post_format.rs`
  - `apply_facets_to_text()` - Main facet processing function
  - `format_facet_feature()` - Individual facet type formatting
  - `blockquote_content_with_facets()` - Combined facets + blockquoting
- **✅ Search integration**: `rust-server/src/tools/search.rs`
- **✅ Thread integration**: `rust-server/src/tools/thread.rs`
- **✅ Feed integration**: `rust-server/src/tools/feed.rs`

### Go Implementation
- **✅ Facet processing**: `go-server/internal/tools/postformat.go`
  - `ApplyFacetsToText()` - Main facet processing function
  - `formatFacetFeature()` - Individual facet type formatting
  - `BlockquoteContentWithFacets()` - Combined facets + blockquoting
- **✅ Search integration**: `go-server/internal/tools/search.go`
- **✅ Thread integration**: `go-server/internal/tools/thread.go`
- **✅ Feed integration**: `go-server/internal/tools/feed.go`

### Future Work
- **🔜 Embed rendering**: New module `rust-server/src/markdown/embeds.rs` (planned)
- **🔜 URL utilities**: `rust-server/src/markdown/urls.rs` (planned)

## Testing Strategy

1. **✅ Unit tests - Facets (COMPLETED)**
   - ✅ Facet byte range conversion
   - ✅ Markdown link formatting
   - ✅ UTF-8/emoji handling
   - ✅ Multiple facets in one post
   - ✅ Empty facets handling

2. **🔜 Unit tests - Embeds (TODO)**
   - Markdown escaping
   - URL construction

3. **🔜 Integration tests (TODO)**
   - Real posts with various embed types
   - Edge cases: empty embeds, missing fields
   - Malformed data handling

3. **Test cases**
   - Post with only text
   - Post with mentions and links
   - Post with single image
   - Post with multiple images
   - Post with external link preview
   - Quote post
   - Quote post with images
   - Deeply nested quotes
   - Posts with emoji and multi-byte chars

## Dependencies

- `unicode-segmentation`: Already in Cargo.toml, for proper UTF-8 handling
- `regex`: Already in Cargo.toml, for text processing
- Consider: `pulldown-cmark` for Markdown validation (optional)

## Migration Path

1. Keep current simple text rendering as fallback
2. Add feature flag `rich-markdown` (optional)
3. Implement incrementally, one embed type at a time
4. Add opt-in parameter to search tools: `format: "text" | "markdown-rich"`
5. Make rich Markdown default once stable

## Open Questions

1. How to handle very large images? Link to thumbnail vs fullsize?
2. Should we fetch and inline external link metadata if missing?
3. How to represent thread structure in Markdown?
4. Should we include interaction counts (likes, reposts)?
5. How to handle deleted/unavailable quoted posts?
6. Localization of labels like "Quote post:", "Replied to:", etc.?

## Future Enhancements

- Video embed support (when Bluesky adds it)
- GIF support via external or images
- Poll results rendering
- Thread visualization (parent/child posts)
- List embeds
- Starter pack embeds
- Feed generator embeds

## Success Criteria

- Posts with images display image links in Markdown
- Mentions and links are clickable in Markdown viewers
- Quote posts are clearly distinguished
- No loss of information from original post
- Graceful degradation for unsupported embed types
- Performance impact < 10ms per post for rendering

## Timeline Estimate

- ✅ Phase 1 (Facets): **COMPLETED** (2-3 days actual)
- 🔜 Phase 2 (Basic embeds): 2-3 days
- 🔜 Phase 3 (Advanced embeds): 3-4 days
- 🔜 Phase 4 (Polish): 2-3 days
- 🔜 Testing & refinement: 2-3 days

**Completed: Phase 1 (Facets)**  
**Remaining: ~2 weeks for full embed implementation**

## References

- [AT Protocol Specifications](https://atproto.com/specs/record-key)
- [Bluesky Lexicons](https://github.com/bluesky-social/atproto/tree/main/lexicons)
- [app.bsky.richtext.facet](https://github.com/bluesky-social/atproto/blob/main/lexicons/app/bsky/richtext/facet.json)
- [app.bsky.embed.*](https://github.com/bluesky-social/atproto/tree/main/lexicons/app/bsky/embed)
