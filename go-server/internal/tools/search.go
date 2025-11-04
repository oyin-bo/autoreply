// Package tools provides MCP tool implementations
package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"time"
	"unicode"

	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
	"github.com/oyin-bo/autoreply/go-server/internal/cache"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
	"golang.org/x/text/unicode/norm"
)

// SearchTool implements the search tool
type SearchTool struct {
	didResolver  *bluesky.DIDResolver
	carProcessor *bluesky.CARProcessor
}

// NewSearchTool creates a new search tool
func NewSearchTool() *SearchTool {
	cacheManager, _ := cache.NewManager()
	return &SearchTool{
		didResolver:  bluesky.NewDIDResolver(),
		carProcessor: bluesky.NewCARProcessor(cacheManager),
	}
}

// Name returns the tool name
func (t *SearchTool) Name() string {
	return "search"
}

// Description returns the tool description
func (t *SearchTool) Description() string {
	return "Search posts within a user's repository"
}

// InputSchema returns the JSON schema for tool input
func (t *SearchTool) InputSchema() mcp.InputSchema {
	return mcp.InputSchema{
		Type: "object",
		Properties: map[string]mcp.PropertySchema{
			"from": {
				Type:        "string",
				Description: "Account whose posts to search: handle (alice.bsky.social), @handle, DID, Bsky.app profile URL, or partial DID suffix",
			},
			"query": {
				Type:        "string",
				Description: "Search terms (case-insensitive)",
			},
			"limit": {
				Type:        "integer",
				Description: "Defaults to 50",
			},
		},
		Required: []string{"from", "query"},
	}
} // Call executes the search tool
func (t *SearchTool) Call(ctx context.Context, args map[string]interface{}, _ *mcp.Server) (*mcp.ToolResult, error) {
	// Extract and validate parameters
	from, query, limit, err := t.validateInput(args)
	if err != nil {
		return nil, err
	}

	// Resolve handle to DID
	did, err := t.didResolver.ResolveHandle(ctx, from)
	if err != nil {
		return nil, err
	}

	// Fetch repository if needed
	if err := t.carProcessor.FetchRepository(ctx, did); err != nil {
		return nil, err
	}

	// Search posts
	posts, err := t.carProcessor.SearchPosts(did, query)
	if err != nil {
		return nil, err
	}

	// Sort by CreatedAt descending (ISO8601 strings; parse for robustness)
	sort.Slice(posts, func(i, j int) bool {
		ti, ei := time.Parse(time.RFC3339, posts[i].CreatedAt)
		tj, ej := time.Parse(time.RFC3339, posts[j].CreatedAt)
		if ei == nil && ej == nil {
			return tj.Before(ti)
		}
		// Fallback to string compare
		return posts[i].CreatedAt > posts[j].CreatedAt
	})

	// Apply limit (default 50)
	if limit <= 0 {
		limit = 50
	}
	if len(posts) > limit {
		posts = posts[:limit]
	}

	// URIs are now constructed directly from MST during search
	// No HTTP requests needed!

	// Format results as markdown
	markdown := t.formatSearchResults(from, query, posts)

	return &mcp.ToolResult{
		Content: []mcp.ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// validateInput validates the input parameters
func (t *SearchTool) validateInput(args map[string]interface{}) (from, query string, limit int, err error) {
	// Validate from
	fromRaw, ok := args["from"]
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "from parameter is required")
	}

	from, ok = fromRaw.(string)
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "from must be a string")
	}

	if strings.TrimSpace(from) == "" {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "from cannot be empty")
	}

	// Validate query
	queryRaw, ok := args["query"]
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query parameter is required")
	}

	query, ok = queryRaw.(string)
	if !ok {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query must be a string")
	}

	query = strings.TrimSpace(query)
	if query == "" {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot be empty")
	}

	if len(query) > 500 {
		return "", "", 0, errors.NewMCPError(errors.InvalidInput, "query cannot exceed 500 characters")
	}

	// Normalize query for consistent search
	query = normalizeText(query)

	// Optional limit
	limit = 50
	if v, ok := args["limit"]; ok {
		switch vv := v.(type) {
		case float64:
			limit = int(vv)
		case int:
			limit = vv
		case int32:
			limit = int(vv)
		case int64:
			limit = int(vv)
		case string:
			// ignore strings silently
		}
	}
	if limit < 1 {
		limit = 1
	}

	return from, query, limit, nil
}

// normalizeText normalizes text for consistent searching
func normalizeText(text string) string {
	// Apply Unicode NFKC normalization
	normalized := norm.NFKC.String(text)

	return strings.ToLower(normalized)
}

// formatSearchResults formats search results as markdown per docs/16-mcp-schemas.md spec
func (t *SearchTool) formatSearchResults(handle, query string, posts []*bluesky.ParsedPost) string {
	var sb strings.Builder

	// Header
	sb.WriteString(fmt.Sprintf("# Search Results · %d posts\n\n", len(posts)))

	if len(posts) == 0 {
		return sb.String()
	}

	// Track seen posts for ID compaction
	seenPosts := make(map[string]bool)

	// Format each post
	for _, post := range posts {
		rkey := ExtractRkey(post.URI)

		// Author ID line
		fullID := fmt.Sprintf("%s/%s", handle, rkey)
		authorID := CompactPostID(handle, rkey, seenPosts)
		sb.WriteString(fmt.Sprintf("%s\n", authorID))
		seenPosts[fullID] = true

		// 1. Highlight the raw text content BEFORE applying facets or formatting.
		highlightedText := HighlightQuery(post.Text, query)
		textWithFacets := ApplyFacetsToText(highlightedText, post.Facets)

		// 2. Highlight the content within embeds before formatting them.
		// We need to create a temporary, modified embed structure for formatting.
		var embedMarkdown string
		if post.Embed != nil {
			// Create a deep copy of the embed to modify it safely.
			tempEmbedBytes, _ := json.Marshal(post.Embed)
			var tempEmbed bluesky.Embed
			json.Unmarshal(tempEmbedBytes, &tempEmbed)

			// Highlight text fields within the copied embed structure.
			if tempEmbed.External != nil {
				tempEmbed.External.Title = HighlightQuery(tempEmbed.External.Title, query)
				tempEmbed.External.Description = HighlightQuery(tempEmbed.External.Description, query)
			}
			for _, img := range tempEmbed.Images {
				img.Alt = HighlightQuery(img.Alt, query)
			}
			if tempEmbed.Media != nil {
				var mediaEmbed bluesky.Embed
				if err := json.Unmarshal(*tempEmbed.Media, &mediaEmbed); err == nil {
					if mediaEmbed.External != nil {
						mediaEmbed.External.Title = HighlightQuery(mediaEmbed.External.Title, query)
						mediaEmbed.External.Description = HighlightQuery(mediaEmbed.External.Description, query)
					}
					for _, img := range mediaEmbed.Images {
						img.Alt = HighlightQuery(img.Alt, query)
					}
					// Marshal the highlighted media embed back to raw JSON.
					highlightedMediaBytes, _ := json.Marshal(mediaEmbed)
					raw := json.RawMessage(highlightedMediaBytes)
					tempEmbed.Media = &raw
				}
			}

			// Now format the embed with the highlighted content.
			embedMarkdown = FormatEmbed(&tempEmbed, post.DID)
		}

		// 3. Combine and blockquote the final content.
		var combinedContent string
		if strings.TrimSpace(textWithFacets) != "" {
			combinedContent = textWithFacets
		}
		if embedMarkdown != "" {
			if combinedContent != "" {
				combinedContent += "\n\n" + embedMarkdown
			} else {
				combinedContent = embedMarkdown
			}
		}

		sb.WriteString(BlockquoteContent(combinedContent))
		sb.WriteString("\n")

		// Stats and timestamp
		timestamp := FormatTimestamp(post.CreatedAt)
		sb.WriteString(fmt.Sprintf("%s\n", timestamp))
		sb.WriteString("\n")
	}

	return sb.String()
}

// HighlightQuery highlights the query in the text with markdown bold.
// It splits the query into words and highlights each word.
func HighlightQuery(text, query string) string {
	if query == "" {
		return text
	}

	// Normalize and split the query into words
	words := strings.Fields(normalizeText(query))
	if len(words) == 0 {
		return text
	}

	// Create a map of words for efficient lookup
	wordMap := make(map[string]bool)
	for _, word := range words {
		wordMap[word] = true
	}

	var result strings.Builder
	var currentWord strings.Builder
	lastCharWasLetter := false
	startPos := 0

	// Iterate through the text rune by rune to identify words
	for i, r := range text {
		isLetter := unicode.IsLetter(r) || unicode.IsNumber(r)
		if isLetter {
			if !lastCharWasLetter {
				startPos = i
			}
			currentWord.WriteRune(r)
		}

		if (!isLetter || i == len(text)-1) && lastCharWasLetter {
			// End of a word
			wordStr := currentWord.String()
			if wordMap[strings.ToLower(wordStr)] {
				// This word is in our query, so highlight it
				result.WriteString("**")
				result.WriteString(text[startPos : i+1])
				if !isLetter {
					result.WriteString("**")
				} else if i == len(text)-1 {
					result.WriteString("**")
				}
			} else {
				// Not a query word, append as is
				result.WriteString(text[startPos : i+1])
			}
			currentWord.Reset()
		}

		if !isLetter {
			result.WriteRune(r)
		}
		lastCharWasLetter = isLetter
	}

	// This is a simplified approach. A more robust solution would handle
	// cases where highlighting might break existing markdown.
	// For now, we will try to fix the fuzzy highlighting as a fallback.
	// The primary strategy of splitting words should fix the main test failure.

	// Let's try a simpler approach first.
	highlightedText := text
	for _, word := range words {
		highlightedText = highlightWord(highlightedText, word)
	}

	// If no substring matches were found, fall back to fuzzy.
	if highlightedText == text {
		return FuzzyHighlightQuery(text, query)
	}

	return highlightedText
}

// highlightWord performs a case-insensitive search for a whole word and wraps it in asterisks.
// It avoids adding highlights inside existing markdown.
func highlightWord(text, word string) string {
	if word == "" {
		return text
	}
	lowerText := strings.ToLower(text)
	lowerWord := strings.ToLower(word)

	var result strings.Builder
	lastIndex := 0
	for {
		index := strings.Index(lowerText[lastIndex:], lowerWord)
		if index == -1 {
			break
		}
		absoluteIndex := lastIndex + index

		// Check if we are inside existing markdown bold/italic or links
		if !isInsideMarkdown(text, absoluteIndex) {
			result.WriteString(text[lastIndex:absoluteIndex])
			result.WriteString("**")
			result.WriteString(text[absoluteIndex : absoluteIndex+len(word)])
			result.WriteString("**")
		} else {
			result.WriteString(text[lastIndex : absoluteIndex+len(word)])
		}
		lastIndex = absoluteIndex + len(word)
	}
	result.WriteString(text[lastIndex:])
	return result.String()
}

// isInsideMarkdown is a helper to prevent nested highlighting.
func isInsideMarkdown(text string, index int) bool {
	// Count asterisks before the index. An odd number suggests we're inside a bold/italic block.
	if strings.Count(text[:index], "**")%2 != 0 {
		return true
	}
	// Check for being inside link text `[]` or URL `()`
	if strings.LastIndex(text[:index], "[") > strings.LastIndex(text[:index], "]") {
		return true
	}
	if strings.LastIndex(text[:index], "(") > strings.LastIndex(text[:index], ")") {
		return true
	}
	return false
}

// FuzzyHighlightQuery performs a character-by-character subsequence match,
// highlighting each matched character. It groups consecutive matches.
func FuzzyHighlightQuery(text, query string) string {
	if query == "" {
		return text
	}
	lowerQuery := strings.ToLower(query)

	var result strings.Builder
	textRunes := []rune(text)
	queryRunes := []rune(lowerQuery)
	queryIdx := 0
	inHighlight := false

	for _, textRune := range textRunes {
		if queryIdx < len(queryRunes) && unicode.ToLower(textRune) == queryRunes[queryIdx] {
			if !inHighlight {
				result.WriteString("**")
				inHighlight = true
			}
			result.WriteRune(textRune)
			queryIdx++
		} else {
			if inHighlight {
				result.WriteString("**")
				inHighlight = false
			}
			result.WriteRune(textRune)
		}
	}

	if inHighlight {
		result.WriteString("**")
	}

	return result.String()
}
