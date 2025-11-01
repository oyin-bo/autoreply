# Improvements to usability for MCP

## Account references

Tools like profile currently allows handle or DID. We want to auto-recognise some other formats to avoid back-end-forth bickering with LLM Agents about format and reduce friction.

Additionally when account reference does not fall into any recognised pattern, we want to perform a search and pick highest ranked result.

* Automatically parse Bsky.app profile URLs
* Allow "@handle" form too, strip @
* DIDs tend to have fixed suffix in did:plc:, so if a suffix is passed in without "did:plc:" in front, we can still detect it with high probability by the character count and the set of characters.
* Otherwise use unauthenticated BSky API search to find most likely candidate from the account reference input (strip spaces too).

When specifying account to post/react as, or perform any other activity authenticated, the search step should not be performed. If the direct pattern was not detected, fail and return error.

In all commands and tools we should streer clear from using "handle" as field/parameter name. Since we support many ways to specify account, let's use "account" to not imply incorrect meaning.

## Remove password from any activity except login

There should not be drive-by login options. Any tool that allows authenticated access can only allow account (handle, DID and others resolved using reasonable patterns) and never a password.

The tools must not call those account parameters as "login", instead should use clearly indicative name similar to postAs, reactAs etc. Use the "As" suffix plus a verb ideally.

## Pagination and limits

Tools should not force pagination or limits. If LLM Agent requests 1000 posts, a tool is expected to do as many batch calls as needed, and efficiently in large batches where possible, and produce the list of results of the required size if possible.

The default size for pagination should be at least 50 in all cases. Where it's likely the desire would be for more, make it higher number.

The framing of the pagination limit in the description should be as "desired number of posts, when omitted implied 50".

Pagination must not force AI Agent to think, it should be framed as optional concept that does not distract from the purpose of the tool. Because of that rename all fields like "cursor" to more like "continueAtCursor".

## Post references

We must already accept three reference patterns:
* at://12345 atproto URI
* https://bsky.app/profile/handle/post/12345 BlueSky web app URL
* @handle/12345 simplified form from our Markdown output

These should be supported everywhere where posts are referenced.

## Login command service

Service should not be surfaced in the API in any way. Autoreply will always resolve and handle that aspect, and there is no option nor any need to provide it.

## Search functionality

Search should improve substantially to mix other aspects into searching of the posts. We also want to introduce fuzzy search and eventually synonym search for local CARs.

Since our output format is Markdown, it is not in any way a problem to mix in heterogenous results.

This means unifying several complex input streams, and performing more complex local search with fuzzy matching.

### Multi-source Method

* Issue several search queries in parallel.
* Aggressively search both using whole search term and separate searches for individual words, but treat quoted parts of search query as atomic, and also do not issue word-separate search for common words like and, or, i and so on.
* BlueSky post search API requires authentication, so skip it when not authenticated but do engage when default authenticated account exists in the system.
* Also search for profiles and feeds (possibly trends in future).

### Multiplexing

* Multiple search results (CAR search, BSky API search output) need to be merged for the output, with deduplication and rank scoring.
* Search rank should be produced locally in Autoreply MCP, not relied upon BlueSky API provided order etc.

### Matching with fuzzy algorithm weighting

Efficient memory-frugal fuzzy matching algorithm needs to be researched. If there is a proven fast library able to handle our use case well, use it. If not, adopt an industry-class algorithm and implement it.

* Use fuzzy matching to produce ranking, and boost rank weight when matched words are close in the text. For matching non-alphanumeric characters can be stripped ("[text]" matches "text"). Unicode should be normalised. Unicode variants and mathematical symbols should match their Latin variants too, but preferred as match with exact Unicode then normalised. Full word match is heavier weight than partial. Beginning of the word match is heavier than the middle of the word and ending of the word gets a small preference to the middle too (car search: "car" > "carton" > "scar" > "scary").
* When search terms include quoted text, that indicates exact match is desired. The BlueSky API requests can still be sent for individual words, in case exact match is not found. But ranking should severely overweight exact matches for the quoted text in this case. And the matching should not include actual quote characters of course.

### Special patterns in search

We must detect special patterns to allow LLM Agents freedom of expressing common search needs.

All special patterns match verbatim text at higher weight, and symbolic match at lower weight. Normal fuzzy text matching also allowed at further lower weight still.

#### Regex

* Regex: detect when wrapped in slashes JavaScript-like. (Are there any other highly-recognisable patterns?). Regex pattern found in the search term will be searched both verbatim, and that match weighted higher. Also it will be used as a matcher by itself and their matches will be weighted lower.

#### Date and time

* Date/time: date/datetime will be recognised inside the search terms with several common formats, and inside the text if search terms include dates. Date/time in search terms represents date range (e.g. for date 2025-01-01 the range is from the moment day starts to its end). Matching of the date is done by any occurrence of such date in content, in ALT texts and by matching the natural timestamp fields on content.
* Date/time spans: common date/time formats joined by common interval characters such as '-', or Unicode long/short dashes, or '...' or Unicode ellipsis etc. In that case we treat this as an interval rather than a pair of dates, and match dates inside content or content timestamps to any date/time within the range.
* Naked time without date is rare as search criteria and does NOT need any handling.

#### Prefixes

* Standard expected social media prefix patterns need to be detected and implemented.
* from:<account> means account posted
* to:<account> means in reply to account, or as a quote of a post of an account