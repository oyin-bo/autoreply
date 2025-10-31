# Output Format Examples: PR #36 vs PR #37

This document shows concrete output examples for both PRs to illustrate the differences.

---

## Feed Tool Output

### Current (Main Branch)
```markdown
# Feed Posts

## Post 1
**Author:** @alice.bsky.social (Alice Smith)
**URI:** at://did:plc:abc123/app.bsky.feed.post/xyz789
**Indexed:** 2024-01-15T10:30:00Z

This is a sample post about BlueSky!

*10 likes, 3 replies*

---
```

### PR #36 Output
```markdown
# Feed Posts

## Post 1
**Link:** https://bsky.app/profile/alice.bsky.social/post/xyz789
**Created:** 2024-01-15T10:30:00Z

This is a sample post about BlueSky!

*10 likes, 3 replies*

---
```

**Changes:**
- ✅ URI → Link (web URL)
- ✅ Indexed → Created
- ❌ **MISSING**: Author info removed (data loss!)
- ❌ **MISSING**: Header still "Feed Posts" not "BlueSky Feed"
- ❌ **MISSING**: No post count summary
- ❌ **MISSING**: Stats still italic, not bold

### PR #37 Output  
```markdown
# BlueSky Feed

Found 1 posts.

## Post 1

**@alice.bsky.social** (Alice Smith)

**Link:** https://bsky.app/profile/alice.bsky.social/post/xyz789

This is a sample post about BlueSky!

**Created:** 2024-01-15T10:30:00Z

**Stats:** 10 likes, 3 replies

---

**Next cursor:** `abc123xyz`
```

**Changes:**
- ✅ Header: "Feed Posts" → "BlueSky Feed"
- ✅ Post count summary added
- ✅ Author format: "**Author:** @handle" → "**@handle**"
- ✅ URI → Link (web URL)
- ✅ Indexed → Created (moved after text)
- ✅ Stats: italic → "**Stats:**" bold
- ✅ Cursor format improved with backticks
- ✅ All info preserved, no data loss

---

## Thread Tool Output

### Current (Main Branch)
```markdown
# Thread

## Post by @alice (Alice Smith)
**URI:** at://did:plc:abc123/app.bsky.feed.post/xyz789
**Indexed:** 2024-01-15T10:30:00Z

Main post text here.

*10 likes, 3 replies*

### Replies:

  ## Post by @bob (Bob Jones)
  **URI:** at://did:plc:def456/app.bsky.feed.post/abc123
  **Indexed:** 2024-01-15T10:35:00Z

  Reply text here.

  *5 likes, 0 replies*
```

### PR #36 Output
```markdown
# Thread

## Post 1
**Link:** https://bsky.app/profile/alice/post/xyz789
**Created:** 2024-01-15T10:30:00Z

Main post text here.

*10 likes, 3 replies*

### Replies:

  ## Post
  **Link:** https://bsky.app/profile/bob/post/abc123
  **Created:** 2024-01-15T10:35:00Z

  Reply text here.

  *5 likes, 0 replies*
```

**Changes:**
- ✅ URI → Link (web URL)
- ✅ Indexed → Created
- ⚠️ Header inconsistent: "Post 1" vs "Post"
- ❌ **MISSING**: Still has indentation and "Replies:" sections
- ❌ **MISSING**: Header still "Thread" not "BlueSky Thread"
- ❌ **MISSING**: No post count summary
- ❌ **MISSING**: Stats still italic

### PR #37 Output
```markdown
# BlueSky Thread

Found 2 posts in thread.

## Post 1

**@alice** (Alice Smith)

**Link:** https://bsky.app/profile/alice/post/xyz789

Main post text here.

**Created:** 2024-01-15T10:30:00Z

**Stats:** 10 likes, 3 replies

---

## Post 2

**@bob** (Bob Jones)

**Link:** https://bsky.app/profile/bob/post/abc123

Reply text here.

**Created:** 2024-01-15T10:35:00Z

**Stats:** 5 likes, 0 replies

---
```

**Changes:**
- ✅ Header: "Thread" → "BlueSky Thread"
- ✅ Post count summary added
- ✅ **FLATTENED**: No indentation, all posts at same level
- ✅ **FLATTENED**: No "Replies:" sections
- ✅ Sequential numbering: "Post 1", "Post 2", etc.
- ✅ Author format matches feed tool
- ✅ URI → Link (web URL)
- ✅ Indexed → Created (moved after text)
- ✅ Stats: italic → "**Stats:**" bold
- ✅ Consistent formatting with feed tool

---

## Go Server Reference Format (Target)

### Feed Tool (go-server/internal/tools/feed.go)
```go
sb.WriteString("# BlueSky Feed\n\n")
sb.WriteString(fmt.Sprintf("Found %d posts.\n\n", len(feedArray)))
sb.WriteString(fmt.Sprintf("## Post %d\n\n", i+1))
sb.WriteString(fmt.Sprintf("**@%s**", handle))
sb.WriteString(fmt.Sprintf("**Link:** %s\n\n", webURL))
sb.WriteString(fmt.Sprintf("%s\n\n", text))
sb.WriteString(fmt.Sprintf("**Created:** %s\n\n", createdAt))
sb.WriteString("**Stats:** ")
sb.WriteString("---\n\n")
sb.WriteString(fmt.Sprintf("**Next cursor:** `%s`\n", cursor))
```

### Alignment:

**PR #36**: ❌ Partial
- Missing: Header change, post count, stats format, cursor format
- Data loss: Author info removed

**PR #37**: ✅ Perfect
- All format elements match exactly
- All information preserved
- Order matches Go implementation

---

## Search Tool Reference Format (rust-server/src/bluesky/records.rs)

### PostRecord::to_markdown()
```rust
let post_url = format!(
    "https://bsky.app/profile/{}/post/{}",
    handle,
    self.uri.split('/').next_back().unwrap_or("")
);
markdown.push_str(&format!("**Link:** {}\n", post_url));
markdown.push_str(&format!("**Created:** {}\n\n", self.created_at));
```

### Alignment:

**PR #36**: ✅ Partial match
- Link format: ✅
- Created format: ✅
- Missing other elements

**PR #37**: ✅ Complete match
- Link format: ✅ (uses shared utility with validation)
- Created format: ✅
- Adds all missing elements from Go reference

---

## Key Differences Summary

| Feature | Current | PR #36 | PR #37 |
|---------|---------|--------|--------|
| **Feed Header** | "Feed Posts" | "Feed Posts" ❌ | "BlueSky Feed" ✅ |
| **Thread Header** | "Thread" | "Thread" ❌ | "BlueSky Thread" ✅ |
| **Post Count** | None | None ❌ | "Found X posts" ✅ |
| **Author Info** | "**Author:** @handle" | Removed ❌ | "**@handle**" ✅ |
| **Link Format** | AT URI | Web URL ✅ | Web URL ✅ |
| **Created Label** | "Indexed" | "Created" ✅ | "Created" ✅ |
| **Stats Format** | `*stats*` | `*stats*` ❌ | `**Stats:** stats` ✅ |
| **Thread Structure** | Nested | Nested ❌ | Flat ✅ |
| **Cursor Format** | `*Cursor: X*` | Unchanged ❌ | `**Next cursor:** \`X\`` ✅ |
| **Go Alignment** | None | Partial | Complete ✅ |

---

## Verdict

**PR #37 produces output that:**
1. ✅ Matches Go server format exactly
2. ✅ Is consistent across feed and thread tools
3. ✅ Preserves all information (no data loss)
4. ✅ Uses modern, clean formatting
5. ✅ Has proper structure and hierarchy

**PR #36 produces output that:**
1. ❌ Doesn't match Go server format
2. ❌ Has inconsistencies between tools
3. ❌ Loses author information (regression)
4. ⚠️ Partial improvements only
5. ❌ Incomplete structural changes

**Recommendation: PR #37 is clearly superior.**
