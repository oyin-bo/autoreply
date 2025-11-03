package tools

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
)

func TestCompactPostID(t *testing.T) {
	tests := []struct {
		name      string
		handle    string
		rkey      string
		seenPosts map[string]bool
		want      string
	}{
		{
			name:      "first mention",
			handle:    "alice.bsky.social",
			rkey:      "3m4jnj3efp22t",
			seenPosts: make(map[string]bool),
			want:      "@alice.bsky.social/3m4jnj3efp22t",
		},
		{
			name:   "subsequent mention",
			handle: "alice.bsky.social",
			rkey:   "3m4jnj3efp22t",
			seenPosts: map[string]bool{
				"alice.bsky.social/3m4jnj3efp22t": true,
			},
			want: "@a/…p22t",
		},
		{
			name:      "short rkey",
			handle:    "bob.bsky.social",
			rkey:      "abc",
			seenPosts: map[string]bool{"bob.bsky.social/abc": true},
			want:      "@b/…abc",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := CompactPostID(tt.handle, tt.rkey, tt.seenPosts)
			if got != tt.want {
				t.Errorf("CompactPostID() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestUltraCompactID(t *testing.T) {
	tests := []struct {
		name   string
		handle string
		rkey   string
		want   string
	}{
		{
			name:   "normal case",
			handle: "alice.bsky.social",
			rkey:   "3m4jnj3efp22t",
			want:   "@a/…p22t",
		},
		{
			name:   "short handle",
			handle: "a",
			rkey:   "3m4jnj3efp22t",
			want:   "@a/…p22t",
		},
		{
			name:   "short rkey",
			handle: "bob.bsky.social",
			rkey:   "xyz",
			want:   "@b/…xyz",
		},
		{
			name:   "empty handle",
			handle: "",
			rkey:   "3m4jnj3efp22t",
			want:   "@?/…p22t",
		},
		{
			name:   "one char rkey",
			handle: "alice.bsky.social",
			rkey:   "x",
			want:   "@a/…x",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := UltraCompactID(tt.handle, tt.rkey)
			if got != tt.want {
				t.Errorf("UltraCompactID() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestBlockquoteContent(t *testing.T) {
	tests := []struct {
		name string
		text string
		want string
	}{
		{
			name: "single line",
			text: "Hello world",
			want: "> Hello world",
		},
		{
			name: "multiple lines",
			text: "Line 1\nLine 2\nLine 3",
			want: "> Line 1\n> Line 2\n> Line 3",
		},
		{
			name: "empty string",
			text: "",
			want: "> \n",
		},
		{
			name: "line with empty lines",
			text: "First\n\nThird",
			want: "> First\n> \n> Third",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := BlockquoteContent(tt.text)
			if got != tt.want {
				t.Errorf("BlockquoteContent() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestFormatStats(t *testing.T) {
	tests := []struct {
		name    string
		likes   int
		reposts int
		quotes  int
		replies int
		want    string
	}{
		{
			name:    "all stats",
			likes:   10,
			reposts: 5,
			quotes:  3,
			replies: 7,
			want:    "👍 10  ♻️ 8  💬 7",
		},
		{
			name:    "only likes",
			likes:   42,
			reposts: 0,
			quotes:  0,
			replies: 0,
			want:    "👍 42",
		},
		{
			name:    "no reposts",
			likes:   10,
			reposts: 0,
			quotes:  0,
			replies: 5,
			want:    "👍 10  💬 5",
		},
		{
			name:    "all zero",
			likes:   0,
			reposts: 0,
			quotes:  0,
			replies: 0,
			want:    "",
		},
		{
			name:    "reshares combined",
			likes:   0,
			reposts: 3,
			quotes:  2,
			replies: 0,
			want:    "♻️ 5",
		},
		{
			name:    "only quotes",
			likes:   0,
			reposts: 0,
			quotes:  7,
			replies: 0,
			want:    "♻️ 7",
		},
		{
			name:    "only replies",
			likes:   0,
			reposts: 0,
			quotes:  0,
			replies: 15,
			want:    "💬 15",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FormatStats(tt.likes, tt.reposts, tt.quotes, tt.replies)
			if got != tt.want {
				t.Errorf("FormatStats() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestFormatTimestamp(t *testing.T) {
	tests := []struct {
		name      string
		timestamp string
		want      string
	}{
		{
			name:      "with milliseconds",
			timestamp: "2024-02-24T12:16:20.637Z",
			want:      "2024-02-24T12:16:20Z",
		},
		{
			name:      "already formatted",
			timestamp: "2024-02-24T12:16:20Z",
			want:      "2024-02-24T12:16:20Z",
		},
		{
			name:      "with timezone offset",
			timestamp: "2024-02-24T12:16:20+00:00",
			want:      "2024-02-24T12:16:20Z",
		},
		{
			name:      "without Z suffix",
			timestamp: "2024-02-24T12:16:20",
			want:      "2024-02-24T12:16:20Z",
		},
		{
			name:      "with nanoseconds",
			timestamp: "2024-02-24T12:16:20.123456789Z",
			want:      "2024-02-24T12:16:20Z",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FormatTimestamp(tt.timestamp)
			if got != tt.want {
				t.Errorf("FormatTimestamp() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestExtractRkey(t *testing.T) {
	tests := []struct {
		name string
		uri  string
		want string
	}{
		{
			name: "normal at:// URI",
			uri:  "at://did:plc:abc123/app.bsky.feed.post/3m4jnj3efp22t",
			want: "3m4jnj3efp22t",
		},
		{
			name: "empty URI",
			uri:  "",
			want: "unknown",
		},
		{
			name: "URI without slashes",
			uri:  "notseparated",
			want: "notseparated",
		},
		{
			name: "trailing slash",
			uri:  "at://did:plc:abc123/app.bsky.feed.post/3m4jnj3efp22t/",
			want: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ExtractRkey(tt.uri)
			if got != tt.want {
				t.Errorf("ExtractRkey() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestThreadingIndicator(t *testing.T) {
	tests := []struct {
		name           string
		depth          int
		replyToCompact string
		authorID       string
		want           string
	}{
		{
			name:           "root post",
			depth:          0,
			replyToCompact: "",
			authorID:       "@alice/3m4jnj3efp22t",
			want:           "@alice/3m4jnj3efp22t",
		},
		{
			name:           "first level reply",
			depth:          1,
			replyToCompact: "@a/…p22t",
			authorID:       "@bob/xyz123",
			want:           "└─@a/…p22t → @bob/xyz123",
		},
		{
			name:           "second level reply",
			depth:          2,
			replyToCompact: "@b/…123",
			authorID:       "@charlie/abc456",
			want:           "  └─@b/…123 → @charlie/abc456",
		},
		{
			name:           "third level reply",
			depth:          3,
			replyToCompact: "@c/…456",
			authorID:       "@dave/def789",
			want:           "    └─@c/…456 → @dave/def789",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ThreadingIndicator(tt.depth, tt.replyToCompact, tt.authorID)
			if got != tt.want {
				t.Errorf("ThreadingIndicator() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestGetIntField(t *testing.T) {
	tests := []struct {
		name string
		m    map[string]interface{}
		key  string
		want int
	}{
		{
			name: "int value",
			m:    map[string]interface{}{"count": 42},
			key:  "count",
			want: 42,
		},
		{
			name: "int32 value",
			m:    map[string]interface{}{"count": int32(100)},
			key:  "count",
			want: 100,
		},
		{
			name: "int64 value",
			m:    map[string]interface{}{"count": int64(999)},
			key:  "count",
			want: 999,
		},
		{
			name: "float64 value",
			m:    map[string]interface{}{"count": float64(123.7)},
			key:  "count",
			want: 123,
		},
		{
			name: "float32 value",
			m:    map[string]interface{}{"count": float32(99.9)},
			key:  "count",
			want: 99,
		},
		{
			name: "missing key",
			m:    map[string]interface{}{"other": 42},
			key:  "count",
			want: 0,
		},
		{
			name: "wrong type",
			m:    map[string]interface{}{"count": "not a number"},
			key:  "count",
			want: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := GetIntField(tt.m, tt.key)
			if got != tt.want {
				t.Errorf("GetIntField() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestHighlightQuery(t *testing.T) {
	tests := []struct {
		name  string
		text  string
		query string
		want  string
	}{
		{
			name:  "simple match",
			text:  "Hello world",
			query: "world",
			want:  "Hello **world**",
		},
		{
			name:  "case insensitive",
			text:  "Hello World",
			query: "world",
			want:  "Hello **World**",
		},
		{
			name:  "multiple matches",
			text:  "rust is great and rust is fast",
			query: "rust",
			want:  "**rust** is great and **rust** is fast",
		},
		{
			name:  "no match",
			text:  "Hello world",
			query: "xyz",
			want:  "Hello world",
		},
		{
			name:  "empty query",
			text:  "Hello world",
			query: "",
			want:  "Hello world",
		},
		{
			name:  "partial word match",
			text:  "programming",
			query: "gram",
			want:  "pro**gram**ming",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := HighlightQuery(tt.text, tt.query)
			if got != tt.want {
				t.Errorf("HighlightQuery() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestParseTimestamp(t *testing.T) {
	tests := []struct {
		name       string
		timestamp  string
		wantErr    bool
		checkYear  int
		checkMonth time.Month
		checkDay   int
	}{
		{
			name:       "RFC3339",
			timestamp:  "2024-02-24T12:16:20Z",
			wantErr:    false,
			checkYear:  2024,
			checkMonth: time.February,
			checkDay:   24,
		},
		{
			name:       "RFC3339Nano",
			timestamp:  "2024-02-24T12:16:20.637Z",
			wantErr:    false,
			checkYear:  2024,
			checkMonth: time.February,
			checkDay:   24,
		},
		{
			name:       "without timezone",
			timestamp:  "2024-02-24T12:16:20",
			wantErr:    false,
			checkYear:  2024,
			checkMonth: time.February,
			checkDay:   24,
		},
		{
			name:      "invalid format",
			timestamp: "not-a-timestamp",
			wantErr:   true,
		},
		{
			name:      "empty string",
			timestamp: "",
			wantErr:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParseTimestamp(tt.timestamp)
			if (err != nil) != tt.wantErr {
				t.Errorf("ParseTimestamp() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if !tt.wantErr {
				if got.Year() != tt.checkYear || got.Month() != tt.checkMonth || got.Day() != tt.checkDay {
					t.Errorf("ParseTimestamp() = %v, want year=%d, month=%d, day=%d",
						got, tt.checkYear, tt.checkMonth, tt.checkDay)
				}
			}
		})
	}
}

// TestBlockquoteContentPreservesFormatting ensures that blockquote formatting
// preserves the exact line structure of the input
func TestBlockquoteContentPreservesFormatting(t *testing.T) {
	input := "First line\n\nThird line\n  Indented\n\nSixth"
	expected := "> First line\n> \n> Third line\n>   Indented\n> \n> Sixth"

	got := BlockquoteContent(input)
	if got != expected {
		t.Errorf("BlockquoteContent() formatting mismatch:\ngot:\n%s\nwant:\n%s", got, expected)
	}
}

// TestHighlightQueryWithOverlap ensures highlighting works with overlapping patterns
func TestHighlightQueryWithOverlap(t *testing.T) {
	// For very short queries (≤3 chars), word boundaries are required
	// So "aa" won't match inside "aaaa" without word boundaries
	text := "test aa test aaaa"
	query := "aa"
	got := HighlightQuery(text, query)

	// Should highlight standalone "aa" with word boundaries: "test **aa** test aaaa"
	expected := "test **aa** test aaaa"
	if got != expected {
		t.Errorf("HighlightQuery() = %q, want %q", got, expected)
	}
}

// TestFormatStatsOrdering ensures stats are always in correct order
func TestFormatStatsOrdering(t *testing.T) {
	result := FormatStats(10, 5, 3, 7)

	// Should be in order: likes, reshares (reposts+quotes), replies
	parts := strings.Split(result, "  ")
	if len(parts) != 3 {
		t.Errorf("Expected 3 parts, got %d: %s", len(parts), result)
	}

	if !strings.HasPrefix(parts[0], "👍") {
		t.Errorf("First stat should be likes, got: %s", parts[0])
	}
	if !strings.HasPrefix(parts[1], "♻️") {
		t.Errorf("Second stat should be reshares, got: %s", parts[1])
	}
	if !strings.HasPrefix(parts[2], "💬") {
		t.Errorf("Third stat should be replies, got: %s", parts[2])
	}
}

func TestFormatEmbed(t *testing.T) {
	did := "did:plc:test"
	tests := []struct {
		name  string
		embed *bluesky.Embed
		want  string
	}{
		{
			name: "single image embed",
			embed: &bluesky.Embed{
				Type: bluesky.EmbedImages,
				Images: []*bluesky.ImageEmbed{
					{
						Alt:   "A beautiful sunset",
						Image: &bluesky.BlobRef{Ref: "bafkreihd..."},
					},
				},
			},
			want: "![A beautiful sunset](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkreihd...@jpeg)",
		},
		{
			name: "multiple images embed",
			embed: &bluesky.Embed{
				Type: bluesky.EmbedImages,
				Images: []*bluesky.ImageEmbed{
					{
						Alt:   "Image 1",
						Image: &bluesky.BlobRef{Ref: "bafkrei_img1..."},
					},
					{
						Alt:   "Image 2",
						Image: &bluesky.BlobRef{Ref: "bafkrei_img2..."},
					},
				},
			},
			want: "![Image 1](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_img1...@jpeg)\n![Image 2](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_img2...@jpeg)",
		},
		{
			name: "external link embed",
			embed: &bluesky.Embed{
				Type: bluesky.EmbedExternal,
				External: &bluesky.ExternalEmbed{
					URI:         "https://example.com",
					Title:       "Example Title",
					Description: "This is a description.",
					Thumb:       &bluesky.BlobRef{Ref: "bafkrei_thumb..."},
				},
			},
			want: "[Example Title](https://example.com)\n> This is a description.\n![thumb](https://cdn.bsky.app/img/feed_thumbnail/plain/did:plc:test/bafkrei_thumb...@jpeg)",
		},
		{
			name: "record embed (quote post)",
			embed: &bluesky.Embed{
				Type:   bluesky.EmbedRecord,
				Record: &bluesky.RecordEmbed{URI: "at://did:plc:test/app.bsky.feed.post/3kxyz"},
			},
			want: "> Quoted post: at://did:plc:test/app.bsky.feed.post/3kxyz",
		},
		{
			name: "record with image media",
			embed: &bluesky.Embed{
				Type:   bluesky.EmbedRecordWithMedia,
				Record: &bluesky.RecordEmbed{URI: "at://did:plc:quote/app.bsky.feed.post/3kabc"},
				Media:  makeRawMessage(t, &bluesky.Embed{Type: bluesky.EmbedImages, Images: []*bluesky.ImageEmbed{{Alt: "A cat", Image: &bluesky.BlobRef{Ref: "bafkrei_cat..."}}}}),
			},
			want: "> Quoted post: at://did:plc:quote/app.bsky.feed.post/3kabc\n![A cat](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_cat...@jpeg)",
		},
		{
			name:  "nil embed",
			embed: nil,
			want:  "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FormatEmbed(tt.embed, did)
			if got != tt.want {
				t.Errorf("FormatEmbed() = %q, want %q", got, tt.want)
			}
		})
	}
}

func makeRawMessage(t *testing.T, v interface{}) *json.RawMessage {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("Failed to marshal to json.RawMessage: %v", err)
	}
	raw := json.RawMessage(b)
	return &raw
}

func TestApplyFacetsToText(t *testing.T) {
	tests := []struct {
		name   string
		text   string
		facets []bluesky.Facet
		want   string
	}{
		{
			name:   "no facets",
			text:   "Plain text",
			facets: []bluesky.Facet{},
			want:   "Plain text",
		},
		{
			name: "mention facet",
			text: "Hello @alice.bsky.social how are you?",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 6, ByteEnd: 24},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#mention",
							DID:  "did:plc:abc123",
						},
					},
				},
			},
			want: "Hello [@alice.bsky.social](https://bsky.app/profile/alice.bsky.social) how are you?",
		},
		{
			name: "link facet",
			text: "Check out https://example.com for more info",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 10, ByteEnd: 29},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#link",
							URI:  "https://example.com",
						},
					},
				},
			},
			want: "Check out [https://example.com](https://example.com) for more info",
		},
		{
			name: "hashtag facet",
			text: "This is #awesome stuff",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 8, ByteEnd: 16},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#tag",
							Tag:  "awesome",
						},
					},
				},
			},
			want: "This is [#awesome](https://bsky.app/hashtag/awesome) stuff",
		},
		{
			name: "multiple facets",
			text: "Hey @bob check https://test.com and #cool",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 4, ByteEnd: 8},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#mention",
							DID:  "did:plc:xyz",
						},
					},
				},
				{
					Index: bluesky.IndexRange{ByteStart: 15, ByteEnd: 31},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#link",
							URI:  "https://test.com",
						},
					},
				},
				{
					Index: bluesky.IndexRange{ByteStart: 36, ByteEnd: 41},
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#tag",
							Tag:  "cool",
						},
					},
				},
			},
			want: "Hey [@bob](https://bsky.app/profile/bob) check [https://test.com](https://test.com) and [#cool](https://bsky.app/hashtag/cool)",
		},
		{
			name: "emoji with facet",
			text: "Hello 👋 @alice",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 11, ByteEnd: 17}, // After "Hello 👋 " (👋 is 4 bytes)
					Features: []bluesky.FacetFeature{
						{
							Type: "app.bsky.richtext.facet#mention",
							DID:  "did:plc:test",
						},
					},
				},
			},
			want: "Hello 👋 [@alice](https://bsky.app/profile/alice)",
		},
		{
			name: "unsorted facets",
			text: "Hey @bob check #cool",
			facets: []bluesky.Facet{
				{ // #cool
					Index: bluesky.IndexRange{ByteStart: 15, ByteEnd: 20},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#tag", Tag: "cool"},
					},
				},
				{ // @bob
					Index: bluesky.IndexRange{ByteStart: 4, ByteEnd: 8},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#mention", DID: "did:plc:xyz"},
					},
				},
			},
			want: "Hey [@bob](https://bsky.app/profile/bob) check [#cool](https://bsky.app/hashtag/cool)",
		},
		{
			name: "overlapping facets (link over mention)",
			text: "Check out @alice.bsky.social",
			facets: []bluesky.Facet{
				{ // @alice.bsky.social (mention)
					Index: bluesky.IndexRange{ByteStart: 10, ByteEnd: 28},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#mention", DID: "did:plc:abc"},
					},
				},
				{ // Check out @alice.bsky.social (link)
					Index: bluesky.IndexRange{ByteStart: 0, ByteEnd: 28},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#link", URI: "https://example.com"},
					},
				},
			},
			// The larger facet (link) should win
			want: "[Check out @alice.bsky.social](https://example.com)",
		},
		{
			name: "adjacent facets",
			text: "#one#two",
			facets: []bluesky.Facet{
				{
					Index: bluesky.IndexRange{ByteStart: 0, ByteEnd: 4},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#tag", Tag: "one"},
					},
				},
				{
					Index: bluesky.IndexRange{ByteStart: 4, ByteEnd: 8},
					Features: []bluesky.FacetFeature{
						{Type: "app.bsky.richtext.facet#tag", Tag: "two"},
					},
				},
			},
			want: "[#one](https://bsky.app/hashtag/one)[#two](https://bsky.app/hashtag/two)",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ApplyFacetsToText(tt.text, tt.facets)
			if got != tt.want {
				t.Errorf("ApplyFacetsToText() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestBlockquoteContentWithFacets(t *testing.T) {
	text := "Hello @alice check this out"
	facets := []bluesky.Facet{
		{
			Index: bluesky.IndexRange{ByteStart: 6, ByteEnd: 12},
			Features: []bluesky.FacetFeature{
				{
					Type: "app.bsky.richtext.facet#mention",
					DID:  "did:plc:abc",
				},
			},
		},
	}

	got := BlockquoteContentWithFacets(text, facets)
	want := "> Hello [@alice](https://bsky.app/profile/alice) check this out"

	if got != want {
		t.Errorf("BlockquoteContentWithFacets() = %q, want %q", got, want)
	}
}

func TestApplyFacetsToText_EdgeCases(t *testing.T) {
	tests := []struct {
		name   string
		text   string
		facets []bluesky.Facet
		want   string
	}{
		{
			name: "invalid facet indices (out of bounds)",
			text: "A text with a bad facet.",
			facets: []bluesky.Facet{
				{
					Index:    bluesky.IndexRange{ByteStart: 15, ByteEnd: 100},
					Features: []bluesky.FacetFeature{{Type: "app.bsky.richtext.facet#tag", Tag: "bad"}},
				},
			},
			want: "A text with a bad facet.",
		},
		{
			name: "invalid facet indices (inverted)",
			text: "A text with a bad facet.",
			facets: []bluesky.Facet{
				{
					Index:    bluesky.IndexRange{ByteStart: 10, ByteEnd: 5},
					Features: []bluesky.FacetFeature{{Type: "app.bsky.richtext.facet#tag", Tag: "inverted"}},
				},
			},
			want: "A text with a bad facet.",
		},
		{
			name: "malformed facet data (no features)",
			text: "Text with a featureless facet.",
			facets: []bluesky.Facet{
				{
					Index:    bluesky.IndexRange{ByteStart: 10, ByteEnd: 21},
					Features: []bluesky.FacetFeature{},
				},
			},
			want: "Text with a featureless facet.",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// This function should not panic and should return the original text
			// for invalid facets. A more robust implementation might log errors.
			got := ApplyFacetsToText(tt.text, tt.facets)
			if got != tt.want {
				t.Errorf("ApplyFacetsToText() with edge cases = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestFormatPost_Combinations(t *testing.T) {
	did := "did:plc:test"
	tests := []struct {
		name   string
		text   string
		facets []bluesky.Facet
		embed  *bluesky.Embed
		want   string
	}{
		{
			name:   "text with facets and embed",
			text:   "More info at example.com",
			facets: []bluesky.Facet{{Index: bluesky.IndexRange{ByteStart: 13, ByteEnd: 24}, Features: []bluesky.FacetFeature{{Type: "app.bsky.richtext.facet#link", URI: "https://example.com"}}}},
			embed: &bluesky.Embed{
				Type:     bluesky.EmbedExternal,
				External: &bluesky.ExternalEmbed{URI: "https://anotherexample.com", Title: "Another Example", Description: "Description here."},
			},
			want: "> More info at [example.com](https://example.com)\n\n[Another Example](https://anotherexample.com)\n> Description here.",
		},
		{
			name:   "text without facets and embed",
			text:   "Check out this cool picture!",
			facets: []bluesky.Facet{},
			embed: &bluesky.Embed{
				Type:   bluesky.EmbedImages,
				Images: []*bluesky.ImageEmbed{{Alt: "A cool picture", Image: &bluesky.BlobRef{Ref: "bafy_cool..."}}},
			},
			want: "> Check out this cool picture!\n\n![A cool picture](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafy_cool...@jpeg)",
		},
		{
			name:   "embed with empty text",
			text:   "",
			facets: []bluesky.Facet{},
			embed: &bluesky.Embed{
				Type:   bluesky.EmbedImages,
				Images: []*bluesky.ImageEmbed{{Alt: "An image on its own", Image: &bluesky.BlobRef{Ref: "bafy_solo..."}}},
			},
			want: "![An image on its own](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafy_solo...@jpeg)",
		},
		{
			name:   "record with external media",
			text:   "Complex embed.",
			facets: []bluesky.Facet{},
			embed: &bluesky.Embed{
				Type:   bluesky.EmbedRecordWithMedia,
				Record: &bluesky.RecordEmbed{URI: "at://did:plc:quote/app.bsky.feed.post/3kdef"},
				Media: makeRawMessage(t, &bluesky.Embed{
					Type:     bluesky.EmbedExternal,
					External: &bluesky.ExternalEmbed{URI: "https://dev.blueskyweb.xyz/", Title: "Bluesky Dev", Description: "Dev docs"},
				}),
			},
			want: "> Complex embed.\n\n> Quoted post: at://did:plc:quote/app.bsky.feed.post/3kdef\n[Bluesky Dev](https://dev.blueskyweb.xyz/)\n> Dev docs",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var finalMd string
			textMd := BlockquoteContentWithFacets(tt.text, tt.facets)
			embedMd := FormatEmbed(tt.embed, did)

			if tt.text == "" {
				finalMd = embedMd
			} else {
				finalMd = fmt.Sprintf("%s\n\n%s", textMd, embedMd)
			}

			if finalMd != tt.want {
				t.Errorf("Formatted Post = %q, want %q", finalMd, tt.want)
			}
		})
	}
}

func TestSearchAndHighlightInEmbeds(t *testing.T) {
	// 1. Setup: Create a post with rich embeds
	post := &bluesky.ParsedPost{
		PostRecord: &bluesky.PostRecord{
			URI:       "at://did:plc:test/app.bsky.feed.post/embed_search",
			CID:       "cid_embed_search",
			Text:      "This post has an image and a link.",
			CreatedAt: "2024-01-01T00:00:00Z",
			Embed: &bluesky.Embed{
				Type: bluesky.EmbedRecordWithMedia,
				Record: &bluesky.RecordEmbed{
					URI: "at://did:plc:quoted/app.bsky.feed.post/quoted_post",
					CID: "cid_quoted",
				},
				Media: makeRawMessage(t, &bluesky.Embed{
					Type: bluesky.EmbedImages,
					Images: []*bluesky.ImageEmbed{
						{
							Alt:   "A detailed photo of a fuzzy brown cat",
							Image: &bluesky.BlobRef{Ref: "bafkrei_cat_fuzzy..."},
						},
					},
				}),
			},
		},
		DID: "did:plc:test",
	}

	// 2. Test Searchable Text Extraction
	searchable := post.GetSearchableText()
	expectedSearchable := []string{
		"This post has an image and a link.",
		"A detailed photo of a fuzzy brown cat",
	}
	if len(searchable) != len(expectedSearchable) {
		t.Fatalf("GetSearchableText() returned %d items, want %d", len(searchable), len(expectedSearchable))
	}
	for i, s := range searchable {
		if s != expectedSearchable[i] {
			t.Errorf("Searchable text item %d = %q, want %q", i, s, expectedSearchable[i])
		}
	}

	// 3. Test Highlighting in Formatted Output
	searchTool := NewSearchTool()
	query := "fuzzy cat"
	markdown := searchTool.formatSearchResults("test.bsky.social", query, []*bluesky.ParsedPost{post})

	// The blockquoted content should contain the main text and the formatted, highlighted embed.
	expectedHighlightInAlt := "![A detailed photo of a **fuzzy** brown **cat**](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:test/bafkrei_cat_fuzzy...@jpeg)"
	expectedQuotedPost := "> Quoted post: at://did:plc:quoted/app.bsky.feed.post/quoted_post"

	// Check that the final markdown contains the highlighted part within the blockquote.
	// We need to check for the blockquoted version of the highlight.
	if !strings.Contains(markdown, expectedQuotedPost) {
		t.Errorf("Formatted markdown does not contain the quoted post line.\nGot:\n%s", markdown)
	}
	if !strings.Contains(markdown, expectedHighlightInAlt) {
		t.Errorf("Formatted markdown does not contain the highlighted image alt text.\nGot:\n%s", markdown)
	}

	// Also check the main text is present and blockquoted
	if !strings.Contains(markdown, "> This post has an image and a link.") {
		t.Errorf("Formatted markdown does not contain the blockquoted main text.\nGot:\n%s", markdown)
	}
}
