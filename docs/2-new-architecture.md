# AutoReply v2: Conceptual Architecture

## Your Vision Summary

**Current Problems:**
- Tech foundation is solid (atcute migration, minimal deps)
- Code is hairy and opportunistic - not maintainable
- BlueSky data model too verbose for LLMs
- API calls too chunky/slow

**Your Solution:**
- Ground-up rewrite
- Aggressive local caching
- Slim default tools with rich drill-down option
- Fast cache-backed operations

## Key Architectural Questions

### Tool Universe: Function-First Design
Current: 8+ fragmented tools (feed, profile, search, thread, post, like, repost, delete)
**Solution**: Organize by actual social media use patterns, not tech abstractions.

**Functional breakdown**:
- `feed` - streamed feed/search, paginated, cache-optimized for recency
- `thread` - snapshot of thread/post with all replies and context, plus basic engagement metrics
- `post_details` - drill-down into engagement: all likers, quotes, retweets of a post, paginated
- `profile` - user profile with followers/following/blocked/lists - attempt mining CAR downloads for efficiency
- `post` - content creation: posting/retweeting/quoting with auto-splitting for length, reaction support
- `delete` - content removal with batch support for multiple posts
- `like` - engagement actions with batch support for multiple posts
- `login` - authentication via app password or OAuth

**Key insight**: Each tool maps to a distinct user/AI mental model and caching pattern.

### Data Model: Slim vs Rich
**Core insight**: LLMs get overwhelmed by BlueSky's verbosity but sometimes need the detail.

**Question**: What's the minimal viable "slim tweet" format? What goes in "rich drill-down"?

**Thinking**: 
- Slim: author, text, timestamp, basic metrics, reply/quote indicators
- Rich: full engagement data, media details, thread structure, user profiles

### Compact Caching Strategy
**Three viable candidates emerged**:

**Option A: CAR (Content Addressable aRchive)**
- Native to BlueSky ecosystem, content-addressable by design
- Perfect deduplication, cryptographic integrity
- Challenge: Implementation complexity, limited tooling outside atproto

**Option B: LevelDB with WASM**
- Proven LSM-tree storage, excellent compression
- WASM version available = fast + portable
- Key-value model fits social media data well
- Challenge: C++ dependency (though WASM mitigates this)

**Option C: MessagePack + Files**
- Simple binary format, ~30% smaller than JSON
- Minimal dependencies, easy debugging
- File-per-timerange organization
- Challenge: Manual indexing, no built-in compression

**Caching patterns by tool**:
- `feed`: Time-bucketed, recent bias, aggressive eviction
- `thread`: Content-addressed, long retention (threads don't change much)
- `post_details`: Engagement data, medium volatility
- `profile`: User-keyed, moderate refresh rates

### Auth Model Evolution
App password + OAuth is complex. Most AI agents probably just need app passwords.

**Question**: Start simple with app passwords, add OAuth later?

### Multi-Platform Future
You mentioned unified "social media" format for Mastodon, Twitter, Facebook expansion.

**Question**: How much abstraction is realistic? Each platform has unique features.

### Media Interface
Images, videos need special handling - probably references with lazy loading.

**Question**: What's the interaction model? Count-only by default, details on demand?

### Sampling Capabilities
**Good insight**: LLMs often work better with representative samples than full datasets.

**Question**: What sampling strategies matter? Recent? Popular? Diverse? Random?

## Hidden Pitfalls & Pragmatic Concerns

**Cache Invalidation Hell**: Social media data changes constantly. How do we balance freshness vs performance?

**Rate Limiting**: Aggressive caching means bulk fetching initially. Platforms don't like that.

**Storage Explosion**: Caching everything could eat disk space fast.

**Over-Engineering Risk**: Complex architecture might be overkill for the actual use cases.

**Migration Complexity**: How do we transition from current system without breaking users?

## Core Architecture Decisions

### Storage Technology Choice
**CAR vs LevelDB/WASM vs MessagePack** - each has distinct trade-offs:
- CAR: Native ecosystem fit, but complex implementation
- LevelDB: Battle-tested performance, WASM portability
- MessagePack: Simplest implementation, adequate performance

### Tool-Cache Alignment
Each functional tool needs its own caching strategy:
- Stream tools (`feed`) = time-oriented, high churn
- Snapshot tools (`thread`) = content-addressed, stable
- Drill-down tools (`post_details`) = engagement-focused, medium volatility

### Data Compactness
**Critical**: Default slim format must be dramatically smaller than current BlueSky verbosity
- Strip unnecessary metadata by default
- Progressive enhancement for rich details
- Cache both formats separately

## Dynamic Storage Clustering Strategy

### Dual-Clustering Approach
**Core insight**: Social media data naturally clusters around two distinct access patterns:
- **Account-centric**: User profiles, user's posts, follower lists, engagement by user
- **Conversation-centric**: Threads, reply chains, quote networks, cross-references

**Dynamic migration**: Data starts in one clustering either account-centric from CAR API ingress, or conversation-centric from thread API ingress, and migrates to conversation-centric when access patterns justify the transition.

### Garbage-Collection-Inspired Migration
**Copy-first, cleanup-later model**:
1. **Migration trigger**: Detected conversation-centric access patterns
2. **Copy phase**: Duplicate relevant data into conversation-clustered storage
3. **Cooling period**: Both layouts coexist, serving different access patterns
4. **Garbage collection**: Remove duplicates when access patterns stabilize

**Benefits of this approach**:
- Operating while migration happens in background?
- Graceful handling of access pattern uncertainty
- Fallback if migration was premature?

### Conversation Merging Dynamics
**Conversations dynamically merge** when:
- Quote-tweets create cross-thread references
- Near-quotes via URL reference discussions?
- Semantic links/similarity detected via various signals?

**Storage implications**:
- Conversation boundaries: are they fluid or fixed?
- Merge operations may need re-index
- Post data may belong to multiple conversation clusters simultaneously

### Cache Pressure Management
**Duplicate tolerance requires compact storage** - why slim format is critical.

**Pressure relief strategies**:
1. **Time-based cleanup**: Remove duplicates after cooling period (hours/days)
2. **Size-based eviction**: When cache exceeds thresholds, evict predictively
3. **Access-pattern eviction**: Remove data that hasn't matched recent patterns
4. **Partial eviction**: Keep slim format, evict rich format under pressure

**Predictive eviction policy considerations**:
- Account-centric data: Evict based on user activity recency
- Conversation-centric data: Evict based on thread engagement decay
- Cross-referenced data: Higher retention priority due to merge complexity

### Implementation Challenges
**Complexity trade-offs**:
- Storage overhead during migration periods
- Index maintenance across clustering strategies  
- Conversation boundary detection algorithms
- Pressure threshold tuning without thrashing

**Success metrics**:
- Cache hit rates for different access patterns
- Storage efficiency (duplication ratio over time)
- Migration decision accuracy (false positive rate)
- Query response times across clustering strategies

This approach embraces the messy, interconnected nature of social media data while providing systematic optimization for real usage patterns.

## Next Steps for Exploration

Rather than diving into implementation, we should probably:
- Sketch the slim data format with real examples
- Define the core tool interactions
- Prototype the caching approach with a small dataset
- Test LLM performance with slim vs rich formats

What aspects do you want to explore deeper first?