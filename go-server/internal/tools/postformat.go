// Package tools provides post formatting utilities for consistent Markdown output
package tools

import (
	"fmt"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
)

// PostFormatter provides utilities for formatting BlueSky posts as Markdown
// according to the spec in docs/16-mcp-schemas.md

// ApplyFacetsToText applies facets to text, converting mentions/links/tags to Markdown format
// Facets use byte indices, so we need to handle UTF-8 properly
func ApplyFacetsToText(text string, facets []bluesky.Facet) string {
	if len(facets) == 0 {
		return text
	}

	// Sort facets by ByteStart, and for overlapping facets, prioritize the larger one
	sortedFacets := make([]bluesky.Facet, len(facets))
	copy(sortedFacets, facets)
	// Simple bubble sort for small arrays
	for i := 0; i < len(sortedFacets); i++ {
		for j := i + 1; j < len(sortedFacets); j++ {
			if sortedFacets[j].Index.ByteStart < sortedFacets[i].Index.ByteStart ||
				(sortedFacets[j].Index.ByteStart == sortedFacets[i].Index.ByteStart && sortedFacets[j].Index.ByteEnd > sortedFacets[i].Index.ByteEnd) {
				sortedFacets[i], sortedFacets[j] = sortedFacets[j], sortedFacets[i]
			}
		}
	}

	var result strings.Builder
	lastByteIdx := 0
	textBytes := []byte(text)

	for _, facet := range sortedFacets {
		startByte := facet.Index.ByteStart
		endByte := facet.Index.ByteEnd

		// Skip facets that are completely contained within the last processed facet
		if startByte < lastByteIdx {
			continue
		}

		// Add text before this facet
		if lastByteIdx < startByte {
			result.Write(textBytes[lastByteIdx:startByte])
		}

		// Get the text covered by this facet
		facetText := string(textBytes[startByte:endByte])

		// Apply the facet formatting based on feature type
		formatted := formatFacetFeature(facetText, facet.Features)
		result.WriteString(formatted)

		lastByteIdx = endByte
	}

	// Add remaining text after last facet
	if lastByteIdx < len(textBytes) {
		result.Write(textBytes[lastByteIdx:])
	}

	return result.String()
}

// formatFacetFeature formats a facet feature (mention, link, or tag) as Markdown
func formatFacetFeature(text string, features []interface{}) string {
	if len(features) == 0 {
		return text
	}

	// Use the first feature if multiple are present
	feature := features[0]
	if featureMap, ok := feature.(map[string]interface{}); ok {
		facetType, _ := featureMap["$type"].(string)

		switch facetType {
		case "app.bsky.richtext.facet#mention":
			// The text already contains the @ symbol and handle
			// Extract handle without the @ prefix for the URL
			handle := strings.TrimPrefix(text, "@")
			return fmt.Sprintf("[%s](https://bsky.app/profile/%s)", text, handle)

		case "app.bsky.richtext.facet#link":
			// Create a markdown link
			if uri, ok := featureMap["uri"].(string); ok {
				return fmt.Sprintf("[%s](%s)", text, uri)
			}

		case "app.bsky.richtext.facet#tag":
			// Link to hashtag search
			if tag, ok := featureMap["tag"].(string); ok {
				return fmt.Sprintf("[#%s](https://bsky.app/hashtag/%s)", tag, tag)
			}
		}
	}

	// No recognized feature, return text as-is
	return text
}

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

// BlockquoteContentWithFacets blockquotes user content with facets applied
// This is the preferred method when you have facet data available
func BlockquoteContentWithFacets(text string, facets []bluesky.Facet) string {
	formattedText := ApplyFacetsToText(text, facets)
	return BlockquoteContent(formattedText)
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
// For short queries or acronyms, only highlights whole-word matches to avoid false positives.
func HighlightQuery(text, query string) string {
	if query == "" {
		return text
	}

	lowerText := strings.ToLower(text)
	lowerQuery := strings.ToLower(query)

	// Determine if we should use strict word-boundary matching
	// Apply for: short queries (≤3 chars) or all-uppercase acronyms
	strictMode := len(query) <= 3 || (len(query) > 0 && query == strings.ToUpper(query) && strings.IndexFunc(query, func(r rune) bool {
		return r >= 'A' && r <= 'Z'
	}) >= 0)

	// Try substring matching first
	if strings.Contains(lowerText, lowerQuery) {
		var result strings.Builder
		remaining := text
		lowerRemaining := lowerText
		lowerRunes := []rune(lowerText)
		queryLen := len([]rune(query))

		pos := 0
		for {
			idx := strings.Index(lowerRemaining, lowerQuery)
			if idx == -1 {
				result.WriteString(remaining)
				break
			}

			absoluteIdx := pos + idx

			// In strict mode, check word boundaries
			if strictMode {
				// Check if match is at word boundary
				atStart := absoluteIdx == 0 || !isWordChar(lowerRunes[absoluteIdx-1])
				atEnd := (absoluteIdx+queryLen >= len(lowerRunes)) || !isWordChar(lowerRunes[absoluteIdx+queryLen])

				if !atStart || !atEnd {
					// Not a whole-word match, skip this occurrence
					result.WriteString(remaining[:idx+len([]rune(lowerQuery))])
					remaining = remaining[idx+len([]rune(lowerQuery)):]
					lowerRemaining = lowerRemaining[idx+len(lowerQuery):]
					pos += idx + queryLen
					continue
				}
			}

			// Valid match - highlight it
			result.WriteString(remaining[:idx])
			result.WriteString("**")
			result.WriteString(remaining[idx : idx+len(query)])
			result.WriteString("**")

			remaining = remaining[idx+len(query):]
			lowerRemaining = lowerRemaining[idx+len(query):]
			pos += idx + queryLen
		}

		return result.String()
	}

	// No substring match - don't show scattered fuzzy highlighting as it's confusing
	return text
}

// isWordChar returns true if the rune is alphanumeric or underscore
func isWordChar(r rune) bool {
	return (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '_'
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
