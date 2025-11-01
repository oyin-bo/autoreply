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
