// Package bluesky provides AT Protocol utilities
package bluesky

import (
	"fmt"
	"regexp"
	"strings"
)

var (
	// Regular expressions for parsing Bluesky URLs
	bskyPostURLRegex     = regexp.MustCompile(`^https://bsky\.app/profile/([^/]+)/post/([a-z0-9]+)$`)
	gistingPostURLRegex  = regexp.MustCompile(`^https://gist\.ing/profile/([^/]+)/post/([a-z0-9]+)$`)
	bskyStylePostURLRegex = regexp.MustCompile(`^https://[^/]+/profile/([^/]+)/post/([a-z0-9]+)$`)
	atURIRegex           = regexp.MustCompile(`^at://([^/]+)/([^/]+)/([a-z0-9]+)$`)
	didRegex             = regexp.MustCompile(`^did:`)
)

// PostRef represents a parsed post reference
type PostRef struct {
	DID        string // Full DID or handle
	Collection string // e.g., "app.bsky.feed.post"
	RKey       string // Record key
}

// ParsePostURI parses either an at:// URI or a https://bsky.app/... URL into a PostRef
func ParsePostURI(uri string) (*PostRef, error) {
	if uri == "" {
		return nil, fmt.Errorf("empty URI")
	}

	uri = strings.TrimSpace(uri)

	// Try at:// URI format first: at://did:plc:xxx/app.bsky.feed.post/rkey
	if matches := atURIRegex.FindStringSubmatch(uri); matches != nil {
		return &PostRef{
			DID:        matches[1],
			Collection: matches[2],
			RKey:       matches[3],
		}, nil
	}

	// Try bsky.app URL format: https://bsky.app/profile/handle/post/rkey
	if matches := bskyPostURLRegex.FindStringSubmatch(uri); matches != nil {
		return &PostRef{
			DID:        matches[1], // Could be handle or DID
			Collection: "app.bsky.feed.post",
			RKey:       matches[2],
		}, nil
	}

	// Try gist.ing URL format
	if matches := gistingPostURLRegex.FindStringSubmatch(uri); matches != nil {
		return &PostRef{
			DID:        matches[1],
			Collection: "app.bsky.feed.post",
			RKey:       matches[2],
		}, nil
	}

	// Try generic bsky-style URL
	if matches := bskyStylePostURLRegex.FindStringSubmatch(uri); matches != nil {
		return &PostRef{
			DID:        matches[1],
			Collection: "app.bsky.feed.post",
			RKey:       matches[2],
		}, nil
	}

	return nil, fmt.Errorf("invalid post URI format: %s", uri)
}

// MakePostURI creates an at:// URI from DID and rkey
func MakePostURI(did, rkey string) string {
	return fmt.Sprintf("at://%s/app.bsky.feed.post/%s", did, rkey)
}

// MakeLikeURI creates an at:// URI for a like record
func MakeLikeURI(did, rkey string) string {
	return fmt.Sprintf("at://%s/app.bsky.feed.like/%s", did, rkey)
}

// MakeRepostURI creates an at:// URI for a repost record
func MakeRepostURI(did, rkey string) string {
	return fmt.Sprintf("at://%s/app.bsky.feed.repost/%s", did, rkey)
}

// IsLikelyDID checks if a string looks like a DID
func IsLikelyDID(s string) bool {
	return didRegex.MatchString(strings.TrimSpace(s))
}

// NormalizeHandle removes @ prefix and trims whitespace
func NormalizeHandle(handle string) string {
	return strings.TrimPrefix(strings.TrimSpace(handle), "@")
}
