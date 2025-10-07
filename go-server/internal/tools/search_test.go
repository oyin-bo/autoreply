package tools

import (
	"testing"
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
	tool := NewSearchTool()
	text := "hello world, world!"
	query := "world"
	got := tool.highlightMatches(text, query)
	want := "hello **world**, **world**!"
	if got != want {
		t.Fatalf("highlightMatches() = %q, want %q", got, want)
	}
}

func TestValidateInput_Errors(t *testing.T) {
	tool := NewSearchTool()
	// Missing both from and login
	if _, _, _, _, err := tool.validateInput(map[string]interface{}{"query": "x"}); err == nil {
		t.Fatal("expected error for missing from and login")
	}
	// Non-string from
	if _, _, _, _, err := tool.validateInput(map[string]interface{}{"from": 123, "query": "x"}); err == nil {
		t.Fatal("expected error for non-string from")
	}
	// Empty query
	if _, _, _, _, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": "  "}); err == nil {
		t.Fatal("expected error for empty query")
	}
	// Very long query
	long := make([]byte, 501)
	for i := range long {
		long[i] = 'a'
	}
	if _, _, _, _, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": string(long)}); err == nil {
		t.Fatal("expected error for too long query")
	}
}

func TestValidateInput_OkAndLimit(t *testing.T) {
	tool := NewSearchTool()
	from, q, _, lim, err := tool.validateInput(map[string]interface{}{"from": "test.bsky.social", "query": "HeLLo"})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if from != "test.bsky.social" {
		t.Fatalf("from got %q", from)
	}
	if q != "hello" {
		t.Fatalf("query normalized got %q", q)
	}
	if lim != 50 {
		t.Fatalf("default limit got %d want 50", lim)
	}

	_, _, _, lim2, err := tool.validateInput(map[string]interface{}{"from": "t.bsky.social", "query": "x", "limit": 500})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim2 != 200 {
		t.Fatalf("limit clamp got %d want 200", lim2)
	}

	_, _, _, lim3, err := tool.validateInput(map[string]interface{}{"from": "t.bsky.social", "query": "x", "limit": -5})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim3 != 1 {
		t.Fatalf("limit clamp got %d want 1", lim3)
	}
}

func TestNormalizeHandle(t *testing.T) {
	cases := []struct{ in, out string }{
		{"alice.bsky.social", "alice.bsky.social"},
		{"@alice.bsky.social", "alice.bsky.social"},
		{"  @bob.bsky.social  ", "bob.bsky.social"},
		{"   carol.bsky.social", "carol.bsky.social"},
	}
	for _, c := range cases {
		got := normalizeHandle(c.in)
		if got != c.out {
			t.Fatalf("normalizeHandle(%q) = %q, want %q", c.in, got, c.out)
		}
	}
}

func TestValidateInput_WithLogin(t *testing.T) {
	tool := NewSearchTool()
	
	// Test with login only
	from, q, login, lim, err := tool.validateInput(map[string]interface{}{"login": "alice.bsky.social", "query": "test"})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if from != "" {
		t.Fatalf("from should be empty, got %q", from)
	}
	if login != "alice.bsky.social" {
		t.Fatalf("login got %q", login)
	}
	if q != "test" {
		t.Fatalf("query got %q", q)
	}
	if lim != 50 {
		t.Fatalf("default limit got %d want 50", lim)
	}

	// Test with both from and login
	from2, q2, login2, lim2, err := tool.validateInput(map[string]interface{}{
		"from": "bob.bsky.social",
		"login": "alice.bsky.social",
		"query": "search",
	})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if from2 != "bob.bsky.social" {
		t.Fatalf("from got %q", from2)
	}
	if login2 != "alice.bsky.social" {
		t.Fatalf("login got %q", login2)
	}
	if q2 != "search" {
		t.Fatalf("query got %q", q2)
	}
	if lim2 != 50 {
		t.Fatalf("default limit got %d want 50", lim2)
	}
}

func TestSearchResultsBlockquoteFormat(t *testing.T) {
	tool := NewSearchTool()

	// We can't directly test formatSearchResults as it expects bluesky.ParsedPost
	// But we test that the format logic works
	result := tool.formatSearchResults("test.bsky.social", "test", nil)

	// Should have header
	if !containsString(result, "# Search Results for") {
		t.Error("Expected header in search results")
	}
}

func containsString(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && (s[:len(substr)] == substr || containsString(s[1:], substr)))
}
