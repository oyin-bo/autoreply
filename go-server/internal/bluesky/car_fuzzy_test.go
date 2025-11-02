package bluesky

import "testing"

func TestFuzzyMatch_Basic(t *testing.T) {
	cases := []struct {
		pattern string
		text    string
		want    bool
		name    string
	}{
		{"hello", "hello world", true, "substring true"},
		{"hlo", "hello", true, "subsequence true"},
		{"hw", "hello world", true, "subsequence across space"},
		{"wrld", "world", true, "missing vowel still subsequence"},
		{"xyz", "hello world", false, "no match"},
		{"", "anything", true, "empty pattern"},
		{"世界", "hello 世界! 🌍", true, "unicode subsequence"},
		{"🌍", "hello 世界! 🌍", true, "emoji match"},
	}

	for _, c := range cases {
		got := fuzzyMatch(c.pattern, c.text)
		if got != c.want {
			t.Fatalf("%s: fuzzyMatch(%q,%q)=%v want %v", c.name, c.pattern, c.text, got, c.want)
		}
	}
}

func TestFuzzyMatch_NonSubstring(t *testing.T) {
	// Ensure a pattern that is not a contiguous substring still matches as subsequence
	text := "compact denoised format"
	// pattern picks every second character from a cleaned fragment
	pattern := "cmatdnoe" // c m a t d n o e (non-contiguous in text)
	if contains := (len(text) >= len(pattern)) && (findSubstring(text, pattern) >= 0); contains {
		t.Skip("constructed pattern unexpectedly appears as substring; skip to avoid flake")
	}
	if !fuzzyMatch(pattern, text) {
		t.Fatalf("expected fuzzy subsequence match for %q in %q", pattern, text)
	}
}

// findSubstring returns index or -1; simple wrapper to avoid importing strings in this file
func findSubstring(haystack, needle string) int {
	for i := 0; i+len(needle) <= len(haystack); i++ {
		if haystack[i:i+len(needle)] == needle {
			return i
		}
	}
	return -1
}
