// Package cli provides command-line interface support for trial mode
package cli

// ProfileArgs defines arguments for the profile tool
type ProfileArgs struct {
	Account string `json:"account" jsonschema:"required,description=Account to fetch profile for: handle @handle DID or Bsky.app URL" short:"a" long:"account"`
}

// SearchArgs defines arguments for the search tool
type SearchArgs struct {
	From  string `json:"from" jsonschema:"required,description=Account whose posts to search: handle @handle DID or Bsky.app URL" short:"f" long:"from"`
	Query string `json:"query" jsonschema:"required,description=Search terms (case-insensitive)" short:"q" long:"query"`
	Limit int    `json:"limit,omitempty" jsonschema:"description=Defaults to 50" short:"l" long:"limit"`
}

// LoginArgs defines arguments for the unified login tool with subcommands (matching Rust LoginCommand)
type LoginArgs struct {
	Command  string `json:"command,omitempty" jsonschema:"description=Subcommand: 'list' 'default' 'delete' or omit for login (default: login)" long:"command"`
	Handle   string `json:"handle,omitempty" jsonschema:"description=Bluesky handle (e.g. alice.bsky.social) (optional - will prompt if not provided)" short:"u" long:"handle"`
	Password string `json:"password,omitempty" jsonschema:"description=App password (if flag present, uses app password mode; will prompt if empty)" short:"p" long:"password"`
	Port     int    `json:"port,omitempty" jsonschema:"description=Local callback server port for OAuth (default: 8080)" long:"port"`
}

// FeedArgs defines arguments for the feed tool
type FeedArgs struct {
	Feed             string `json:"feed,omitempty" jsonschema:"description=Feed URI or name (optional - defaults to What is Hot)" short:"f" long:"feed"`
	ViewAs           string `json:"viewAs,omitempty" jsonschema:"description=Account to view feed as: handle @handle DID or anonymous (optional)" short:"v" long:"view-as"`
	ContinueAtCursor string `json:"continueAtCursor,omitempty" jsonschema:"description=Cursor for pagination (optional)" short:"c" long:"continue-at-cursor"`
	Limit            int    `json:"limit,omitempty" jsonschema:"description=Defaults to 50" short:"l" long:"limit"`
}

// ThreadArgs defines arguments for the thread tool
type ThreadArgs struct {
	PostURI string `json:"postURI" jsonschema:"required,description=Post reference: at:// URI https://bsky.app URL or @handle/rkey" short:"p" long:"post"`
	ViewAs  string `json:"viewAs,omitempty" jsonschema:"description=Account to view thread as: handle @handle DID or anonymous (optional)" short:"v" long:"view-as"`
}

// PostArgs defines arguments for the post tool
type PostArgs struct {
	PostAs  string `json:"postAs,omitempty" jsonschema:"description=Account to post as: handle @handle DID or Bsky.app URL (uses default if not specified)" long:"post-as"`
	Text    string `json:"text" jsonschema:"required,description=Text content of the post" short:"t" long:"text"`
	ReplyTo string `json:"replyTo,omitempty" jsonschema:"description=Post reference: at:// URI https://bsky.app URL or @handle/rkey" short:"r" long:"reply-to"`
}

// ReactArgs defines arguments for the react tool. Post references use at:// URIs, https://bsky.app/... URLs, or @handle/rkey format.
type ReactArgs struct {
	ReactAs string   `json:"reactAs,omitempty" jsonschema:"description=Account to react as: handle @handle DID or Bsky.app URL (uses default if not specified)" long:"react-as"`
	Like    []string `json:"like,omitempty" jsonschema:"description=Posts to like" long:"like"`
	Unlike  []string `json:"unlike,omitempty" jsonschema:"description=Posts to unlike (remove like)" long:"unlike"`
	Repost  []string `json:"repost,omitempty" jsonschema:"description=Posts to repost" long:"repost"`
	Delete  []string `json:"delete,omitempty" jsonschema:"description=Posts to delete (must be your own)" long:"delete"`
}
