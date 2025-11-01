// Package tools provides post formatting utilities for consistent Markdown output
package tools

import (
	"fmt"
	"strings"
	"time"
)

// PostFormatter provides utilities for formatting BlueSky posts as Markdown
// according to the spec in docs/16-mcp-schemas.md

// CompactPostID generates a compact post ID for display
// First mention: @handle/rkey
// Subsequent mentions: @firstletter/…last4
func CompactPostID(handle, rkey string, seenPosts map[string]bool) string {
	fullID := fmt.Sprintf("%s/%s", handle, rkey)

	if seenPosts[fullID] {
		return UltraCompactID(handle, rkey)
	}

	return fmt.Sprintf("@%s/%s", handle, rkey)
}

// UltraCompactID generates ultra-compact format for reply-to references
// @firstletter/…last4
func UltraCompactID(handle, rkey string) string {
	firstLetter := "?"
	if len(handle) > 0 {
		firstLetter = string(handle[0])
	}

	lastFour := rkey
	if len(rkey) > 4 {
		lastFour = rkey[len(rkey)-4:]
	}

	return fmt.Sprintf("@%s/…%s", firstLetter, lastFour)
}

// BlockquoteContent prefixes every line with "> " for Markdown blockquote
func BlockquoteContent(text string) string {
	if text == "" {
		return "> \n"
	}

	lines := strings.Split(text, "\n")
	for i, line := range lines {
		lines[i] = fmt.Sprintf("> %s", line)
	}

	return strings.Join(lines, "\n")
}

// FormatStats formats engagement stats with emojis
// ♻️ combines reposts + quotes
// Only shows non-zero stats
func FormatStats(likes, reposts, quotes, replies int) string {
	var parts []string

	if likes > 0 {
		parts = append(parts, fmt.Sprintf("👍 %d", likes))
	}

	// Combine reposts and quotes into ♻️
	reshares := reposts + quotes
	if reshares > 0 {
		parts = append(parts, fmt.Sprintf("♻️ %d", reshares))
	}

	if replies > 0 {
		parts = append(parts, fmt.Sprintf("💬 %d", replies))
	}

	return strings.Join(parts, "  ")
}

// FormatTimestamp formats timestamp as ISO 8601 without milliseconds, with Z suffix
func FormatTimestamp(timestamp string) string {
	// Remove milliseconds if present and ensure Z suffix
	if idx := strings.Index(timestamp, "."); idx != -1 {
		beforeDot := timestamp[:idx]
		return fmt.Sprintf("%sZ", beforeDot)
	}

	if strings.HasSuffix(timestamp, "Z") {
		return timestamp
	}

	// Remove timezone offset and add Z
	timestamp = strings.Split(timestamp, "+")[0]
	return fmt.Sprintf("%sZ", timestamp)
}

// ExtractRkey extracts the rkey from an at:// URI
// at://did:plc:abc123/app.bsky.feed.post/3m4jnj3efp22t -> 3m4jnj3efp22t
func ExtractRkey(uri string) string {
	if uri == "" {
		return "unknown"
	}

	parts := strings.Split(uri, "/")
	if len(parts) > 0 {
		return parts[len(parts)-1]
	}

	return "unknown"
}

// ThreadingIndicator builds threading indicator with indentation
// depth=0: no prefix (root post)
// depth=1: "└─"
// depth=2: "  └─"
// depth=3: "    └─"
func ThreadingIndicator(depth int, replyToCompact, authorID string) string {
	if depth == 0 {
		// Root post - no indicator, just the author ID
		return authorID
	}

	indent := strings.Repeat("  ", depth-1)
	return fmt.Sprintf("%s└─%s → %s", indent, replyToCompact, authorID)
}

// GetIntField safely extracts an integer field from a map
func GetIntField(m map[string]interface{}, key string) int {
	if val, ok := m[key]; ok {
		switch v := val.(type) {
		case int:
			return v
		case int32:
			return int(v)
		case int64:
			return int(v)
		case float64:
			return int(v)
		case float32:
			return int(v)
		}
	}
	return 0
}

// HighlightQuery highlights query matches in text with **bold** markdown
func HighlightQuery(text, query string) string {
	if query == "" {
		return text
	}

	lowerText := strings.ToLower(text)
	lowerQuery := strings.ToLower(query)

	if !strings.Contains(lowerText, lowerQuery) {
		return text
	}

	var result strings.Builder
	remaining := text
	lowerRemaining := lowerText

	for {
		idx := strings.Index(lowerRemaining, lowerQuery)
		if idx == -1 {
			result.WriteString(remaining)
			break
		}

		// Add text before match
		result.WriteString(remaining[:idx])

		// Add highlighted match
		result.WriteString("**")
		result.WriteString(remaining[idx : idx+len(query)])
		result.WriteString("**")

		// Move to after the match
		remaining = remaining[idx+len(query):]
		lowerRemaining = lowerRemaining[idx+len(query):]
	}

	return result.String()
}

// ParseTimestamp attempts to parse various timestamp formats
func ParseTimestamp(ts string) (time.Time, error) {
	// Try common formats
	formats := []string{
		time.RFC3339,
		time.RFC3339Nano,
		"2006-01-02T15:04:05Z",
		"2006-01-02T15:04:05",
	}

	for _, format := range formats {
		if t, err := time.Parse(format, ts); err == nil {
			return t, nil
		}
	}

	return time.Time{}, fmt.Errorf("unable to parse timestamp: %s", ts)
}
