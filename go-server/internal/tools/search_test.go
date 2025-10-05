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
	// Missing account
	if _, _, _, err := tool.validateInput(map[string]interface{}{"query": "x"}); err == nil {
		t.Fatal("expected error for missing account")
	}
	// Non-string account
	if _, _, _, err := tool.validateInput(map[string]interface{}{"account": 123, "query": "x"}); err == nil {
		t.Fatal("expected error for non-string account")
	}
	// Empty query
	if _, _, _, err := tool.validateInput(map[string]interface{}{"account": "test.bsky.social", "query": "  "}); err == nil {
		t.Fatal("expected error for empty query")
	}
	// Very long query
	long := make([]byte, 501)
	for i := range long {
		long[i] = 'a'
	}
	if _, _, _, err := tool.validateInput(map[string]interface{}{"account": "test.bsky.social", "query": string(long)}); err == nil {
		t.Fatal("expected error for too long query")
	}
}

func TestValidateInput_OkAndLimit(t *testing.T) {
	tool := NewSearchTool()
	acc, q, lim, err := tool.validateInput(map[string]interface{}{"account": "test.bsky.social", "query": "HeLLo"})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if acc != "test.bsky.social" {
		t.Fatalf("account got %q", acc)
	}
	if q != "hello" {
		t.Fatalf("query normalized got %q", q)
	}
	if lim != 50 {
		t.Fatalf("default limit got %d want 50", lim)
	}

	_, _, lim2, err := tool.validateInput(map[string]interface{}{"account": "t.bsky.social", "query": "x", "limit": 500})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim2 != 200 {
		t.Fatalf("limit clamp got %d want 200", lim2)
	}

	_, _, lim3, err := tool.validateInput(map[string]interface{}{"account": "t.bsky.social", "query": "x", "limit": -5})
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if lim3 != 1 {
		t.Fatalf("limit clamp got %d want 1", lim3)
	}
}
