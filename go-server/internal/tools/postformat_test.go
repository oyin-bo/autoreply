package tools

import (
	"strings"
	"testing"
	"time"
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
