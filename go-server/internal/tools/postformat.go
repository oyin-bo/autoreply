// Package tools provides post formatting utilities for consistent Markdown output
package tools

import (
	"encoding/json"
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

		// Basic validation for the facet range
		if startByte < 0 || endByte < 0 || startByte > endByte || endByte > len(textBytes) {
			continue // Skip invalid facets entirely
		}

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

// FormatEmbed formats a single embed into a Markdown string.
// `did` is required to construct full image URLs.
func FormatEmbed(embed *bluesky.Embed, did string) string {
	if embed == nil {
		return ""
	}

	switch embed.Type {
	case bluesky.EmbedImages:
		var parts []string
		for _, img := range embed.Images {
			// URL format: https://cdn.bsky.app/img/feed_fullsize/plain/{did}/{cid}@jpeg
			url := fmt.Sprintf("https://cdn.bsky.app/img/feed_fullsize/plain/%s/%s@jpeg", did, img.Image.Ref)
			parts = append(parts, fmt.Sprintf("![%s](%s)", img.Alt, url))
		}
		return strings.Join(parts, "\n")

	case bluesky.EmbedExternal:
		var parts []string
		if embed.External != nil {
			parts = append(parts, fmt.Sprintf("[%s](%s)", embed.External.Title, embed.External.URI))
			if embed.External.Description != "" {
				parts = append(parts, BlockquoteContent(embed.External.Description))
			}
			if embed.External.Thumb != nil {
				url := fmt.Sprintf("https://cdn.bsky.app/img/feed_thumbnail/plain/%s/%s@jpeg", did, embed.External.Thumb.Ref)
				parts = append(parts, fmt.Sprintf("![thumb](%s)", url))
			}
		}
		return strings.Join(parts, "\n")

	case bluesky.EmbedRecord:
		if embed.Record != nil {
			return BlockquoteContent(fmt.Sprintf("Quoted post: %s", embed.Record.URI))
		}

	case bluesky.EmbedRecordWithMedia:
		var recordMd, mediaMd string
		if embed.Record != nil {
			recordMd = FormatEmbed(&bluesky.Embed{Type: bluesky.EmbedRecord, Record: embed.Record}, did)
		}

		if embed.Media != nil {
			var mediaEmbed bluesky.Embed
			if err := json.Unmarshal(*embed.Media, &mediaEmbed); err == nil {
				// The `did` for the media's blob should be the same as the post's author
				mediaMd = FormatEmbed(&mediaEmbed, did)
			}
		}
		return fmt.Sprintf("%s\n%s", recordMd, mediaMd)
	}

	return ""
}

// formatFacetFeature formats a facet feature (mention, link, or tag) as Markdown
func formatFacetFeature(text string, features []bluesky.FacetFeature) string {
	if len(features) == 0 {
		return text
	}

	// Use the first feature if multiple are present
	feature := features[0]

	switch feature.Type {
	case "app.bsky.richtext.facet#mention":
		// The text already contains the @ symbol and handle
		// Extract handle without the @ prefix for the URL
		handle := strings.TrimPrefix(text, "@")
		return fmt.Sprintf("[%s](https://bsky.app/profile/%s)", text, handle)

	case "app.bsky.richtext.facet#link":
		// Create a markdown link
		return fmt.Sprintf("[%s](%s)", text, feature.URI)

	case "app.bsky.richtext.facet#tag":
		// Link to hashtag search
		return fmt.Sprintf("[#%s](https://bsky.app/hashtag/%s)", feature.Tag, feature.Tag)
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
