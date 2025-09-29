// car.go - CAR file operations and repository fetching
package bluesky

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/fxamacker/cbor/v2"
	"github.com/ipld/go-car/v2"

	"github.com/oyin-bo/autoreply/internal/cache"
	"github.com/oyin-bo/autoreply/pkg/errors"
)

// CarProcessor handles CAR file operations
type CarProcessor struct {
	client   *Client
	cache    *cache.Manager
	resolver *DidResolver
}

// NewCarProcessor creates a new CAR processor
func NewCarProcessor() (*CarProcessor, error) {
	cacheManager, err := cache.NewManager()
	if err != nil {
		return nil, fmt.Errorf("failed to create cache manager: %w", err)
	}

	return &CarProcessor{
		client:   NewClient(),
		cache:    cacheManager,
		resolver: NewDidResolver(),
	}, nil
}

// FetchRepo fetches a repository CAR file, using cache if valid
func (c *CarProcessor) FetchRepo(ctx context.Context, did string) ([]byte, error) {
	// Set total timeout to 60 seconds for CAR download
	ctx, cancel := context.WithTimeout(ctx, 60*time.Second)
	defer cancel()

	// Check cache validity (24 hours TTL for repositories)
	if c.cache.IsCacheValid(did, 24) {
		return c.cache.GetCARData(did)
	}

	// Discover PDS endpoint
	pdsEndpoint, err := c.resolver.DiscoverPDS(ctx, did)
	if err != nil {
		return nil, fmt.Errorf("failed to discover PDS: %w", err)
	}

	// Fetch repository from PDS
	url := fmt.Sprintf("%s/xrpc/com.atproto.sync.getRepo?did=%s", pdsEndpoint, did)
	
	resp, err := c.client.Get(ctx, url)
	if err != nil {
		return nil, errors.NewMcpError(errors.RepoFetchFailed, fmt.Sprintf("Failed to fetch repository: %v", err))
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, errors.NewMcpError(errors.RepoFetchFailed, fmt.Sprintf("HTTP %d: %s", resp.StatusCode, resp.Status))
	}

	// Read response data
	var buf bytes.Buffer
	written, err := c.client.StreamDownload(ctx, url, &buf)
	if err != nil {
		return nil, errors.NewMcpError(errors.RepoFetchFailed, fmt.Sprintf("Failed to download CAR: %v", err))
	}

	carData := buf.Bytes()

	// Extract HTTP headers for cache metadata
	etag := resp.Header.Get("ETag")
	lastModified := resp.Header.Get("Last-Modified")
	contentLengthStr := resp.Header.Get("Content-Length")
	
	var contentLength int64
	if contentLengthStr != "" {
		contentLength, _ = strconv.ParseInt(contentLengthStr, 10, 64)
	} else {
		contentLength = written
	}

	// Store in cache
	metadata := &cache.Metadata{
		DID:           did,
		ETag:          etag,
		LastModified:  lastModified,
		ContentLength: contentLength,
		CachedAt:      time.Now(),
		TTLHours:      24,
	}

	if err := c.cache.StoreData(did, carData, metadata); err != nil {
		// Log cache error but continue - we have the data
		fmt.Printf("Warning: Failed to cache data: %v\n", err)
	}

	return carData, nil
}

// ExtractProfileRecord extracts profile record from CAR data
func (c *CarProcessor) ExtractProfileRecord(carData []byte, did string) (*ProfileRecord, error) {
	reader := bytes.NewReader(carData)
	
	// Create a CAR reader
	carReader, err := car.NewBlockReader(reader)
	if err != nil {
		return nil, errors.NewMcpError(errors.RepoParseFailed, fmt.Sprintf("Failed to create CAR reader: %v", err))
	}

	// Iterate through blocks to find profile record
	for {
		block, err := carReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			continue // Skip problematic blocks
		}

		// Try to decode as CBOR
		var record map[string]interface{}
		if err := cbor.Unmarshal(block.RawData(), &record); err != nil {
			continue // Skip non-CBOR blocks or invalid records
		}

		// Check if this is a profile record
		if recordType, ok := record["$type"].(string); ok && recordType == "app.bsky.actor.profile" {
			// Convert to ProfileRecord
			profile := &ProfileRecord{
				Type: recordType,
			}

			if displayName, ok := record["displayName"].(string); ok {
				profile.DisplayName = &displayName
			}

			if description, ok := record["description"].(string); ok {
				profile.Description = &description
			}

			if createdAt, ok := record["createdAt"].(string); ok {
				profile.CreatedAt = createdAt
			} else {
				// Fallback created at
				profile.CreatedAt = time.Now().Format(time.RFC3339)
			}

			return profile, nil
		}
	}

	return nil, errors.NewMcpError(errors.NotFound, "Profile record not found in repository")
}

// ExtractPostRecords extracts post records from CAR data
func (c *CarProcessor) ExtractPostRecords(carData []byte, did string) ([]*PostRecord, error) {
	reader := bytes.NewReader(carData)
	
	carReader, err := car.NewBlockReader(reader)
	if err != nil {
		return nil, errors.NewMcpError(errors.RepoParseFailed, fmt.Sprintf("Failed to create CAR reader: %v", err))
	}

	var posts []*PostRecord

	// Iterate through blocks to find post records
	for {
		block, err := carReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			continue // Skip problematic blocks
		}

		// Try to decode as CBOR
		var record map[string]interface{}
		if err := cbor.Unmarshal(block.RawData(), &record); err != nil {
			continue
		}

		// Check if this is a post record
		if recordType, ok := record["$type"].(string); ok && recordType == "app.bsky.feed.post" {
			post := &PostRecord{
				Type: recordType,
			}

			if text, ok := record["text"].(string); ok {
				post.Text = text
			}

			if createdAt, ok := record["createdAt"].(string); ok {
				post.CreatedAt = createdAt
			}

			// Handle embeds (simplified for POC)
			if embed, ok := record["embed"].(map[string]interface{}); ok {
				post.Embed = &Embed{}
				
				if embedType, ok := embed["$type"].(string); ok {
					post.Embed.Type = embedType

					// Handle external links
					if embedType == "app.bsky.embed.external" {
						if external, ok := embed["external"].(map[string]interface{}); ok {
							post.Embed.External = &External{}
							if uri, ok := external["uri"].(string); ok {
								post.Embed.External.URI = uri
							}
							if title, ok := external["title"].(string); ok {
								post.Embed.External.Title = title
							}
							if description, ok := external["description"].(string); ok {
								post.Embed.External.Description = description
							}
						}
					}
				}
			}

			posts = append(posts, post)
		}
	}

	return posts, nil
}

// CleanupCache removes expired cache entries
func (c *CarProcessor) CleanupCache() error {
	return c.cache.CleanupExpired()
}