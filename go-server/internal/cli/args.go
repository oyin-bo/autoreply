// Package cli provides command-line interface support for trial mode
package cli

// ProfileArgs defines arguments for the profile tool
type ProfileArgs struct {
	Account string `json:"account" jsonschema:"required,description=Handle (alice.bsky.social) or DID (did:plc:...)" short:"a" long:"account"`
}

// SearchArgs defines arguments for the search tool
type SearchArgs struct {
	From  string `json:"from" jsonschema:"required,description=Handle (alice.bsky.social) or DID (did:plc:...) to search posts from" short:"f" long:"from"`
	Query string `json:"query" jsonschema:"required,description=Search terms (case-insensitive)" short:"q" long:"query"`
	Limit int    `json:"limit,omitempty" jsonschema:"description=Maximum number of results (default 50)" short:"l" long:"limit"`
}

// LoginArgs defines arguments for the unified login tool with subcommands (matching Rust LoginCommand)
type LoginArgs struct {
	Command  string `json:"command,omitempty" jsonschema:"description=Subcommand: 'list' 'default' 'delete' or omit for login (default: login)" long:"command"`
	Handle   string `json:"handle,omitempty" jsonschema:"description=Bluesky handle (e.g. alice.bsky.social) (optional - will prompt if not provided)" short:"u" long:"handle"`
	Password string `json:"password,omitempty" jsonschema:"description=App password (if flag present, uses app password mode; will prompt if empty)" short:"p" long:"password"`
	Port     int    `json:"port,omitempty" jsonschema:"description=Local callback server port for OAuth (default: 8080)" long:"port"`
	Service  string `json:"service,omitempty" jsonschema:"description=Service URL (defaults to https://bsky.social)" short:"s" long:"service"`
}

// FeedArgs defines arguments for the feed tool
type FeedArgs struct {
	Feed   string `json:"feed,omitempty" jsonschema:"description=Feed URI or name (optional - defaults to What is Hot)" short:"f" long:"feed"`
	Login  string `json:"login,omitempty" jsonschema:"description=BlueSky handle for authenticated feed (optional - use 'anonymous' for incognito)" short:"u" long:"login"`
	Cursor string `json:"cursor,omitempty" jsonschema:"description=Cursor for pagination (optional)" short:"c" long:"cursor"`
	Limit  int    `json:"limit,omitempty" jsonschema:"description=Limit number of posts (default 20)" short:"l" long:"limit"`
}

// ThreadArgs defines arguments for the thread tool
type ThreadArgs struct {
	PostURI string `json:"postURI" jsonschema:"required,description=BlueSky URL or at:// URI of the post" short:"p" long:"post"`
	Login   string `json:"login,omitempty" jsonschema:"description=BlueSky handle for authenticated fetch (optional - use 'anonymous' for incognito)" short:"u" long:"login"`
}

// PostArgs defines arguments for the post tool
type PostArgs struct {
	PostAs  string `json:"postAs,omitempty" jsonschema:"description=Handle or DID to post as (uses default if not specified)" long:"post-as"`
	Text    string `json:"text" jsonschema:"required,description=Text content of the post" short:"t" long:"text"`
	ReplyTo string `json:"replyTo,omitempty" jsonschema:"description=Post URI (at://...) or URL (https://bsky.app/...) to reply to" short:"r" long:"reply-to"`
}

// ReactArgs defines arguments for the react tool
type ReactArgs struct {
	ReactAs string   `json:"reactAs,omitempty" jsonschema:"description=Handle or DID to react as (uses default if not specified)" long:"react-as"`
	Like    []string `json:"like,omitempty" jsonschema:"description=Post URIs/URLs to like" long:"like"`
	Unlike  []string `json:"unlike,omitempty" jsonschema:"description=Post URIs/URLs to unlike" long:"unlike"`
	Repost  []string `json:"repost,omitempty" jsonschema:"description=Post URIs/URLs to repost" long:"repost"`
	Delete  []string `json:"delete,omitempty" jsonschema:"description=Post URIs/URLs to delete" long:"delete"`
}
