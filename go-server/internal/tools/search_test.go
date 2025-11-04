package tools

import (
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
)

func TestNormalizeText(t *testing.T) {
	cases := []struct{ in, out string }{
		{"Hello World", "hello world"},
		{"Ｆｕｌｌｗｉｄｔｈ", "fullwidth"}, // full-width to ASCII via NFKC
	}
	for _, c := range cases {
		got := normalizeText(c.in)
		if got != c.out {
			t.Fatalf("normalizeText(%q) = %q, want %q", c.in, got, c.out)
		}
	}
}

func TestHighlightMatches(t *testing.T) {
	text := "hello world, world!"
	query := "world"
	got := HighlightQuery(text, query)
	want := "hello **world**, **world**!"
	if got != want {
		t.Fatalf("HighlightQuery() = %q, want %q", got, want)
	}
}

func TestValidateInput_Errors(t *testing.T) {
	tool := NewSearchTool()
	// Missing from
	if _, _, _, err := tool.validateInput(map[string]interface{}{"query": "x"}); err == nil {
		t.Fatal("expected error for missing from")
	}
	// Non-string from
	if _, _, _, err := tool.validateInput(map[string]interface{}{"from": 123, "query": "x"}); err == nil {
		t.Fatal("expected error for non-string from")
	}
	// Empty query
	if _, _, _, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": "  "}); err == nil {
		t.Fatal("expected error for empty query")
	}
	// Very long query
	long := make([]byte, 501)
	for i := range long {
		long[i] = 'a'
	}
	if _, _, _, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": string(long)}); err == nil {
		t.Fatal("expected error for too long query")
	}
}

func TestValidateInput_OkAndLimit(t *testing.T) {
	tool := NewSearchTool()
	acc, q, lim, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": "HeLLo"})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if acc != "test.bsky.social" {
		t.Fatalf("from got %q", acc)
	}
	if q != "hello" {
		t.Fatalf("query normalized got %q", q)
	}
	if lim != 50 {
		t.Fatalf("default limit got %d want 50", lim)
	}

	// Test with large limit - no longer clamped
	_, _, lim2, err := tool.validateInput(map[string]interface{}{"from": "t.bsky.social", "query": "x", "limit": 500})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim2 != 500 {
		t.Fatalf("limit got %d want 500 (no max limit)", lim2)
	}

	// Test negative limit - clamped to 1
	_, _, lim3, err := tool.validateInput(map[string]interface{}{"from": "t.bsky.social", "query": "x", "limit": -5})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim3 != 1 {
		t.Fatalf("limit clamp got %d want 1", lim3)
	}
}

func TestSearchAndHighlightInExternalEmbed(t *testing.T) {
	tool := NewSearchTool()
	post := &bluesky.ParsedPost{
		PostRecord: &bluesky.PostRecord{
			Text: "check this out",
			Embed: &bluesky.Embed{
				Type: bluesky.EmbedExternal,
				External: &bluesky.ExternalEmbed{
					URI:         "https://example.com",
					Title:       "A Great Website",
					Description: "This website has some great content.",
				},
			},
			CreatedAt: "2025-11-03T10:00:00Z",
			URI:       "at://did:plc:xyz/app.bsky.feed.post/3l456",
		},
		DID: "did:plc:xyz",
	}

	query := "great"
	handle := "test.bsky.social"
	markdown := tool.formatSearchResults(handle, query, []*bluesky.ParsedPost{post})

	// The final markdown should have "Great" and "great" highlighted.
	expectedHighlights := []string{
		"> [A **Great** Website](https://example.com)",
		"> > This website has some **great** content.",
	}

	for _, expected := range expectedHighlights {
		if !strings.Contains(markdown, expected) {
			t.Errorf("formatSearchResults() output is missing expected highlight.\nWant to contain: %q\nGot:\n%s", expected, markdown)
		}
	}
}

func TestSearchAndHighlightInPostAndEmbed(t *testing.T) {
	tool := NewSearchTool()
	post := &bluesky.ParsedPost{
		PostRecord: &bluesky.PostRecord{
			Text: "A wonderful picture of a cat.",
			Embed: &bluesky.Embed{
				Type: bluesky.EmbedImages,
				Images: []*bluesky.ImageEmbed{
					{
						Alt: "A wonderful black cat sitting on a chair.",
						Image: &bluesky.BlobRef{
							Ref: "link-to-image",
						},
					},
				},
			},
			CreatedAt: "2025-11-03T11:00:00Z",
			URI:       "at://did:plc:xyz/app.bsky.feed.post/3l789",
		},
		DID: "did:plc:xyz",
	}

	query := "wonderful"
	handle := "test.bsky.social"
	markdown := tool.formatSearchResults(handle, query, []*bluesky.ParsedPost{post})

	expectedHighlights := []string{
		"> A **wonderful** picture of a cat.",
		"> ![A **wonderful** black cat sitting on a chair.](https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:xyz/link-to-image@jpeg)",
	}

	for _, expected := range expectedHighlights {
		if !strings.Contains(markdown, expected) {
			t.Errorf("formatSearchResults() output is missing expected highlight.\nWant to contain: %q\nGot:\n%s", expected, markdown)
		}
	}
}

func TestFuzzyHighlighting(t *testing.T) {
	cases := []struct {
		name     string
		text     string
		query    string
		expected string
	}{
		{
			name:     "Simple fuzzy match",
			text:     "a black cat",
			query:    "abc",
			expected: "**a** **b**la**c**k cat",
		},
		{
			name:     "No match",
			text:     "hello world",
			query:    "xyz",
			expected: "hello world",
		},
		{
			name:     "Case-insensitive fuzzy match",
			text:     "A Black Cat",
			query:    "abc",
			expected: "**A** **B**la**c**k Cat",
		},
		{
			name:     "Substring match should take precedence",
			text:     "This is a test for abcde.",
			query:    "abc",
			expected: "This is a test for **abc**de.",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := HighlightQuery(tc.text, tc.query)
			if got != tc.expected {
				t.Errorf("FuzzyHighlightQuery() = %q, want %q", got, tc.expected)
			}
		})
	}
}
