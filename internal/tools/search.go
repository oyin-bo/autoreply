// search.go - Search tool implementation
package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"golang.org/x/text/unicode/norm"

	"github.com/oyin-bo/autoreply/internal/bluesky"
	"github.com/oyin-bo/autoreply/pkg/errors"
)

// SearchTool implements the search tool
type SearchTool struct {
	processor *bluesky.CarProcessor
	resolver  *bluesky.DidResolver
}

// SearchArgs represents arguments for the search tool
type SearchArgs struct {
	Account string `json:"account"`
	Query   string `json:"query"`
}

// NewSearchTool creates a new search tool
func NewSearchTool() *SearchTool {
	processor, err := bluesky.NewCarProcessor()
	if err != nil {
		panic(fmt.Sprintf("Failed to create CAR processor: %v", err))
	}

	return &SearchTool{
		processor: processor,
		resolver:  bluesky.NewDidResolver(),
	}
}

// Execute executes the search tool
func (s *SearchTool) Execute(ctx context.Context, args json.RawMessage) (*ToolResult, error) {
	// Set total timeout to 120 seconds
	ctx, cancel := context.WithTimeout(ctx, 120*time.Second)
	defer cancel()

	// Parse arguments
	var searchArgs SearchArgs
	if err := json.Unmarshal(args, &searchArgs); err != nil {
		return nil, errors.NewMcpError(errors.InvalidInput, fmt.Sprintf("Invalid arguments: %v", err))
	}

	// Validate parameters
	if err := errors.ValidateAccount(searchArgs.Account); err != nil {
		return nil, err
	}
	if err := errors.ValidateQuery(searchArgs.Query); err != nil {
		return nil, err
	}

	return s.executeImpl(ctx, searchArgs)
}

func (s *SearchTool) executeImpl(ctx context.Context, args SearchArgs) (*ToolResult, error) {
	// Resolve handle to DID if necessary
	did, err := s.resolver.ResolveHandle(ctx, args.Account)
	if err != nil {
		return nil, err
	}

	// Determine display handle
	displayHandle := args.Account
	if strings.HasPrefix(args.Account, "did:plc:") {
		displayHandle = did
	}

	// Fetch repository CAR file
	carData, err := s.processor.FetchRepo(ctx, did)
	if err != nil {
		return nil, err
	}

	// Extract post records
	posts, err := s.processor.ExtractPostRecords(carData, did)
	if err != nil {
		return nil, err
	}

	// Normalize search query
	normalizedQuery := s.normalizeText(args.Query)
	queryTerms := strings.Fields(strings.ToLower(normalizedQuery))

	if len(queryTerms) == 0 {
		return nil, errors.NewMcpError(errors.InvalidInput, "Query contains no searchable terms")
	}

	// Search and score posts
	matchingPosts := s.searchPosts(posts, queryTerms)

	// Format results as markdown
	markdown := s.formatSearchResults(matchingPosts, args.Query, displayHandle, len(posts))

	return &ToolResult{
		Content: []ContentItem{
			{
				Type: "text",
				Text: markdown,
			},
		},
	}, nil
}

// PostMatch represents a matched post with score
type PostMatch struct {
	Post        *bluesky.PostRecord
	Score       int
	MatchedText string
}

// normalizeText normalizes text using Unicode NFKC normalization
func (s *SearchTool) normalizeText(text string) string {
	return norm.NFKC.String(strings.TrimSpace(text))
}

// searchPosts searches posts for query terms and returns matches
func (s *SearchTool) searchPosts(posts []*bluesky.PostRecord, queryTerms []string) []PostMatch {
	var matches []PostMatch

	for _, post := range posts {
		score := 0
		matchedTexts := []string{}

		// Search in post text
		if postScore, matchedText := s.scoreText(post.Text, queryTerms); postScore > 0 {
			score += postScore
			matchedTexts = append(matchedTexts, matchedText)
		}

		// Search in external embed content
		if post.HasExternal() {
			if titleScore, matchedText := s.scoreText(post.GetExternalTitle(), queryTerms); titleScore > 0 {
				score += titleScore
				matchedTexts = append(matchedTexts, "Title: "+matchedText)
			}

			if descScore, matchedText := s.scoreText(post.GetExternalDescription(), queryTerms); descScore > 0 {
				score += descScore
				matchedTexts = append(matchedTexts, "Description: "+matchedText)
			}
		}

		// Search in image alt text
		if post.HasImages() && post.Embed != nil {
			for _, image := range post.Embed.Images {
				if altScore, matchedText := s.scoreText(image.Alt, queryTerms); altScore > 0 {
					score += altScore
					matchedTexts = append(matchedTexts, "Alt: "+matchedText)
				}
			}
		}

		if score > 0 {
			matches = append(matches, PostMatch{
				Post:        post,
				Score:       score,
				MatchedText: strings.Join(matchedTexts, " | "),
			})
		}
	}

	// Sort by score (descending)
	for i := 0; i < len(matches)-1; i++ {
		for j := i + 1; j < len(matches); j++ {
			if matches[i].Score < matches[j].Score {
				matches[i], matches[j] = matches[j], matches[i]
			}
		}
	}

	return matches
}

// scoreText scores text against query terms and returns highlighted version
func (s *SearchTool) scoreText(text string, queryTerms []string) (int, string) {
	if text == "" {
		return 0, ""
	}

	normalizedText := s.normalizeText(text)
	lowerText := strings.ToLower(normalizedText)
	score := 0
	highlightedText := text

	for _, term := range queryTerms {
		if strings.Contains(lowerText, term) {
			score += strings.Count(lowerText, term)
			// Simple highlighting with **bold**
			highlightedText = strings.ReplaceAll(highlightedText, term, "**"+term+"**")
			// Also handle case variations
			titleTerm := strings.Title(term)
			upperTerm := strings.ToUpper(term)
			highlightedText = strings.ReplaceAll(highlightedText, titleTerm, "**"+titleTerm+"**")
			highlightedText = strings.ReplaceAll(highlightedText, upperTerm, "**"+upperTerm+"**")
		}
	}

	return score, highlightedText
}

// formatSearchResults formats search results as markdown
func (s *SearchTool) formatSearchResults(matches []PostMatch, query, handle string, totalPosts int) string {
	var markdown strings.Builder

	// Header
	markdown.WriteString(fmt.Sprintf("# Search Results for \"%s\" in @%s\n\n", query, handle))
	markdown.WriteString(fmt.Sprintf("Found %d matching posts out of %d total posts.\n\n", len(matches), totalPosts))

	if len(matches) == 0 {
		markdown.WriteString("No posts found matching the search criteria.\n")
		return markdown.String()
	}

	// Limit results to prevent overwhelming output
	maxResults := 10
	displayCount := len(matches)
	if displayCount > maxResults {
		displayCount = maxResults
	}

	// Format each matching post
	for i := 0; i < displayCount; i++ {
		match := matches[i]
		post := match.Post

		markdown.WriteString(fmt.Sprintf("## Post %d\n", i+1))
		
		// Post metadata
		if createdAt, err := post.GetCreatedAt(); err == nil {
			markdown.WriteString(fmt.Sprintf("**Created:** %s\n", createdAt.Format("January 2, 2006 at 3:04 PM")))
		}
		markdown.WriteString(fmt.Sprintf("**Score:** %d matches\n\n", match.Score))

		// Post text with highlighting
		if post.Text != "" {
			markdown.WriteString(fmt.Sprintf("%s\n\n", match.MatchedText))
		}

		// Handle embeds
		if post.HasExternal() {
			markdown.WriteString("**External Link:**\n")
			markdown.WriteString(fmt.Sprintf("- [%s](%s)\n", post.GetExternalTitle(), post.GetExternalURL()))
			if desc := post.GetExternalDescription(); desc != "" {
				markdown.WriteString(fmt.Sprintf("  %s\n", desc))
			}
			markdown.WriteString("\n")
		}

		if post.HasImages() {
			markdown.WriteString("**Images:**\n")
			for _, image := range post.Embed.Images {
				if image.Alt != "" {
					markdown.WriteString(fmt.Sprintf("- ![%s](image)\n", image.Alt))
				} else {
					markdown.WriteString("- ![Image](image)\n")
				}
			}
			markdown.WriteString("\n")
		}

		markdown.WriteString("---\n\n")
	}

	// Summary
	if len(matches) > displayCount {
		markdown.WriteString(fmt.Sprintf("**Results:** Showing %d of %d total matches.\n", displayCount, len(matches)))
	} else {
		markdown.WriteString(fmt.Sprintf("**Results:** Showing all %d matches.\n", len(matches)))
	}

	return markdown.String()
}