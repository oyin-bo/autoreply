// Package bluesky provides CAR file processing functionality
package bluesky

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	carv2 "github.com/ipld/go-car/v2"

	"github.com/oyin-bo/autoreply/go-server/internal/cache"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// CARProcessor handles downloading and processing CAR files
type CARProcessor struct {
	client      *http.Client
	cacheManager *cache.Manager
}

// NewCARProcessor creates a new CAR processor
func NewCARProcessor(cacheManager *cache.Manager) *CARProcessor {
	return &CARProcessor{
		client: &http.Client{
			Timeout: 60 * time.Second,
		},
		cacheManager: cacheManager,
	}
}

// FetchRepository downloads and caches a repository CAR file
func (p *CARProcessor) FetchRepository(ctx context.Context, did string) error {
	// Check if already cached and valid
	if p.cacheManager.IsCacheValid(did, 24) {
		return nil
	}

	// Build repository URL
	repoURL := fmt.Sprintf("https://bsky.social/xrpc/com.atproto.sync.getRepo?did=%s", did)

	// Create request with context
	req, err := http.NewRequestWithContext(ctx, "GET", repoURL, nil)
	if err != nil {
		return errors.Wrap(err, errors.InternalError, "Failed to create repository request")
	}

	req.Header.Set("User-Agent", "bluesky-mcp-server/1.0")

	// Make the request
	resp, err := p.client.Do(req)
	if err != nil {
		return errors.Wrap(err, errors.RepoFetchFailed, "Failed to fetch repository")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return errors.NewMCPError(errors.RepoFetchFailed,
			fmt.Sprintf("Repository fetch failed with status %d", resp.StatusCode))
	}

	// Read response body
	carData, err := io.ReadAll(resp.Body)
	if err != nil {
		return errors.Wrap(err, errors.RepoFetchFailed, "Failed to read repository data")
	}

	// Create metadata
	metadata := cache.Metadata{
		DID:           did,
		ETag:          getHeader(resp, "ETag"),
		LastModified:  getHeader(resp, "Last-Modified"),
		ContentLength: getContentLength(resp),
		CachedAt:      time.Now().Unix(),
		TTLHours:      24,
	}

	// Store in cache
	if err := p.cacheManager.StoreCar(did, carData, metadata); err != nil {
		return errors.Wrap(err, errors.CacheError, "Failed to cache repository")
	}

	return nil
}

// GetProfile extracts profile information from cached CAR data
func (p *CARProcessor) GetProfile(did string) (*ParsedProfile, error) {
	carData, err := p.cacheManager.ReadCar(did)
	if err != nil {
		return nil, err
	}

	// Parse CAR file
	reader := bytes.NewReader(carData)
	carReader, err := carv2.NewBlockReader(reader)
	if err != nil {
		return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed to parse CAR file")
	}

	// Find profile record
	profileRecord, err := p.findProfileRecord(carReader, did)
	if err != nil {
		return nil, err
	}

	if profileRecord == nil {
		return nil, errors.NewMCPError(errors.NotFound, "Profile record not found")
	}

	return &ParsedProfile{
		ProfileRecord: profileRecord,
		DID:          did,
		ParsedTime:   time.Now(),
	}, nil
}

// SearchPosts searches for posts containing the given query
func (p *CARProcessor) SearchPosts(did, query string) ([]*ParsedPost, error) {
	carData, err := p.cacheManager.ReadCar(did)
	if err != nil {
		return nil, err
	}

	// Parse CAR file
	reader := bytes.NewReader(carData)
	carReader, err := carv2.NewBlockReader(reader)
	if err != nil {
		return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed to parse CAR file")
	}

	// Find matching posts
	posts, err := p.findMatchingPosts(carReader, did, query)
	if err != nil {
		return nil, err
	}

	return posts, nil
}

// findProfileRecord finds and parses the profile record from the CAR reader
func (p *CARProcessor) findProfileRecord(carReader *carv2.BlockReader, did string) (*ProfileRecord, error) {
	// This is a simplified implementation. In a full implementation,
	// you would traverse the CAR file structure to find the profile record.
	// For now, we'll return a placeholder implementation.
	
	// TODO: Implement proper CAR traversal and CBOR parsing
	// This would involve:
	// 1. Finding the repository root
	// 2. Traversing to the profile collection
	// 3. Reading and parsing the CBOR data
	
	return &ProfileRecord{
		CreatedAt: time.Now().Format(time.RFC3339),
	}, nil
}

// findMatchingPosts finds posts that match the search query
func (p *CARProcessor) findMatchingPosts(carReader *carv2.BlockReader, did, query string) ([]*ParsedPost, error) {
	// This is a simplified implementation. In a full implementation,
	// you would traverse the CAR file structure to find matching posts.
	
	// TODO: Implement proper CAR traversal, CBOR parsing, and text search
	// This would involve:
	// 1. Finding the repository root
	// 2. Traversing to the posts collection
	// 3. Reading and parsing each post's CBOR data
	// 4. Performing text search with highlighting
	
	var posts []*ParsedPost
	return posts, nil
}

// getHeader safely gets a header value from the HTTP response
func getHeader(resp *http.Response, name string) *string {
	if value := resp.Header.Get(name); value != "" {
		return &value
	}
	return nil
}

// getContentLength safely gets the content length from the HTTP response
func getContentLength(resp *http.Response) *int64 {
	if resp.ContentLength > 0 {
		return &resp.ContentLength
	}
	return nil
}