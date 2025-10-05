// Package bluesky provides CAR file processing functionality
package bluesky

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"strings"
	"time"

	carv2 "github.com/ipld/go-car/v2"
	"github.com/ipld/go-ipld-prime/codec/dagcbor"
	ipld "github.com/ipld/go-ipld-prime/datamodel"
	"github.com/ipld/go-ipld-prime/node/basicnode"

	"github.com/oyin-bo/autoreply/go-server/internal/cache"
	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
)

// CARProcessor handles downloading and processing CAR files
type CARProcessor struct {
	client       *http.Client
	cacheManager *cache.Manager
	didResolver  *DIDResolver
}

// ResolveURIsForCIDs resolves at:// URIs for a set of post CIDs by calling listRecords on the user's repo
func (p *CARProcessor) ResolveURIsForCIDs(ctx context.Context, did string, needed map[string]struct{}) (map[string]string, error) {
	result := make(map[string]string)
	if len(needed) == 0 {
		return result, nil
	}

	// Resolve PDS endpoint
	pdsEndpoint, err := p.didResolver.ResolvePDSEndpoint(ctx, did)
	if err != nil {
		return nil, errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve PDS endpoint for listRecords")
	}

	type recordItem struct {
		URI string `json:"uri"`
		CID string `json:"cid"`
	}
	type listResp struct {
		Cursor  *string      `json:"cursor"`
		Records []recordItem `json:"records"`
	}

	cursor := ""
	pages := 0
	for len(needed) > 0 && pages < 25 {
		url := fmt.Sprintf("%s/xrpc/com.atproto.repo.listRecords?repo=%s&collection=app.bsky.feed.post&limit=100", pdsEndpoint, did)
		if cursor != "" {
			url = url + "&cursor=" + cursor
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		if err != nil {
			return nil, errors.Wrap(err, errors.InternalError, "Failed to create listRecords request")
		}
		req.Header.Set("User-Agent", "autoreply/1.0")
		resp, err := p.client.Do(req)
		if err != nil {
			return nil, errors.Wrap(err, errors.RepoFetchFailed, "listRecords request failed")
		}
		func() {
			defer resp.Body.Close()
			if resp.StatusCode != http.StatusOK {
				// Stop paging on error
				return
			}
			var body listResp
			if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
				return
			}
			for _, rec := range body.Records {
				if _, ok := needed[rec.CID]; ok {
					result[rec.CID] = rec.URI
					delete(needed, rec.CID)
				}
			}
			if body.Cursor != nil {
				cursor = *body.Cursor
			} else {
				cursor = ""
			}
		}()
		if cursor == "" {
			break
		}
		pages++
	}

	return result, nil
}

// GetProfile extracts profile information from cached CAR data
func (p *CARProcessor) GetProfile(did string) (*ParsedProfile, error) {
	carData, err := p.cacheManager.ReadCar(did)
	if err != nil {
		return nil, err
	}

	reader := bytes.NewReader(carData)
	carReader, err := carv2.NewBlockReader(reader)
	if err != nil {
		return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed to parse CAR file")
	}

	profileRecord, err := p.findProfileRecord(carReader, did)
	if err != nil {
		return nil, err
	}

	return &ParsedProfile{
		ProfileRecord: profileRecord,
		DID:           did,
		ParsedTime:    time.Now(),
	}, nil
}

// SearchPosts searches for posts containing the given query
func (p *CARProcessor) SearchPosts(did, query string) ([]*ParsedPost, error) {
	carData, err := p.cacheManager.ReadCar(did)
	if err != nil {
		return nil, err
	}

	// Extract CID to rkey mapping from CAR file's MST structure using indigo
	cidToRKey, err := ExtractCIDToRKeyMapping(carData, "app.bsky.feed.post")
	if err != nil {
		log.Printf("Warning: failed to extract rkey mappings: %v", err)
		cidToRKey = make(map[string]string) // Continue with empty map
	}

	reader := bytes.NewReader(carData)
	carReader, err := carv2.NewBlockReader(reader)
	if err != nil {
		return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed to parse CAR file")
	}

	posts, err := p.findMatchingPosts(carReader, did, query, cidToRKey)
	if err != nil {
		return nil, err
	}
	return posts, nil
}

// NewCARProcessor creates a new CAR processor
func NewCARProcessor(cacheManager *cache.Manager) *CARProcessor {
	return &CARProcessor{
		client: &http.Client{
			Timeout: 60 * time.Second,
			Transport: &http.Transport{
				Proxy:               http.ProxyFromEnvironment,
				MaxIdleConns:        10,
				IdleConnTimeout:     30 * time.Second,
				DisableCompression:  false,
				MaxIdleConnsPerHost: 5,
			},
		},
		cacheManager: cacheManager,
		didResolver:  NewDIDResolver(),
	}
}

// FetchRepository downloads and caches a repository CAR file
func (p *CARProcessor) FetchRepository(ctx context.Context, did string) error {
	// Check if already cached and valid
	if p.cacheManager.IsCacheValid(did, 24) {
		log.Printf("Using cached repository for DID: %s", did)
		return nil
	}

	// Resolve PDS endpoint for this DID
	pdsEndpoint, err := p.didResolver.ResolvePDSEndpoint(ctx, did)
	if err != nil {
		return errors.Wrap(err, errors.DIDResolveFailed, "Failed to resolve PDS endpoint")
	}

	// Build repository URL using the PDS endpoint
	repoURL := fmt.Sprintf("%s/xrpc/com.atproto.sync.getRepo?did=%s", pdsEndpoint, did)
	log.Printf("Fetching repository from PDS endpoint: %s", repoURL)

	// Create request with context
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, repoURL, nil)
	if err != nil {
		return errors.Wrap(err, errors.InternalError, "Failed to create repository request")
	}
	req.Header.Set("User-Agent", "autoreply/1.0")

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

// findProfileRecord finds and parses the profile record from the CAR reader
func (p *CARProcessor) findProfileRecord(carReader *carv2.BlockReader, did string) (*ProfileRecord, error) {
	for {
		blk, err := carReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed reading CAR block")
		}

		nb := basicnode.Prototype.Any.NewBuilder()
		if err := dagcbor.Decode(nb, bytes.NewReader(blk.RawData())); err != nil {
			continue
		}
		n := nb.Build()

		if t := getStringNode(n, "$type"); t != "app.bsky.actor.profile" {
			continue
		}

		rec := &ProfileRecord{CreatedAt: getStringNode(n, "createdAt")}
		if s := getStringNode(n, "displayName"); s != "" {
			rec.DisplayName = &s
		}
		if s := getStringNode(n, "description"); s != "" {
			rec.Description = &s
		}
		if s := getStringNode(n, "avatar"); s != "" {
			rec.Avatar = &s
		}
		if s := getStringNode(n, "banner"); s != "" {
			rec.Banner = &s
		}
		return rec, nil
	}
	return nil, errors.NewMCPError(errors.NotFound, "Profile record not found")
}

// findMatchingPosts finds posts that match the search query
func (p *CARProcessor) findMatchingPosts(carReader *carv2.BlockReader, did, query string, cidToRKey map[string]string) ([]*ParsedPost, error) {
	var results []*ParsedPost
	normalizedQuery := strings.ToLower(query)
	for {
		blk, err := carReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, errors.Wrap(err, errors.RepoParseFailed, "Failed reading CAR block")
		}

		nb := basicnode.Prototype.Any.NewBuilder()
		if err := dagcbor.Decode(nb, bytes.NewReader(blk.RawData())); err != nil {
			continue
		}
		n := nb.Build()
		if t := getStringNode(n, "$type"); t != "app.bsky.feed.post" {
			continue
		}

		text := getStringNode(n, "text")
		createdAt := getStringNode(n, "createdAt")

		searchable := strings.ToLower(text)
		if em, err := n.LookupByString("embed"); err == nil && em != nil && em.Kind() == ipld.Kind_Map {
			if ext, err := em.LookupByString("external"); err == nil && ext != nil && ext.Kind() == ipld.Kind_Map {
				searchable += "\n" + strings.ToLower(getStringNode(ext, "title"))
				searchable += "\n" + strings.ToLower(getStringNode(ext, "description"))
			}
			if imgs, err := em.LookupByString("images"); err == nil && imgs != nil && imgs.Kind() == ipld.Kind_List {
				itr := imgs.ListIterator()
				for !itr.Done() {
					_, v, err := itr.Next()
					if err != nil {
						break
					}
					if v.Kind() == ipld.Kind_Map {
						searchable += "\n" + strings.ToLower(getStringNode(v, "alt"))
					}
				}
			}
			if rec, err := em.LookupByString("record"); err == nil && rec != nil && rec.Kind() == ipld.Kind_Map {
				if inner, err := rec.LookupByString("record"); err == nil && inner != nil && inner.Kind() == ipld.Kind_Map {
					searchable += "\n" + strings.ToLower(getStringNode(inner, "text"))
				}
			}
		}

		if normalizedQuery != "" && !strings.Contains(searchable, normalizedQuery) {
			continue
		}

		cidStr := blk.Cid().String()

		// Construct URI from CID to rkey mapping
		uri := ""
		if rkey, ok := cidToRKey[cidStr]; ok {
			// rkey from ForEach already includes collection prefix
			uri = fmt.Sprintf("at://%s/%s", did, rkey)
		}

		pr := &PostRecord{
			URI:       uri,
			CID:       cidStr,
			Text:      text,
			CreatedAt: createdAt,
		}
		results = append(results, &ParsedPost{
			PostRecord:     pr,
			DID:            did,
			SearchableText: searchable,
			ParsedTime:     time.Now(),
		})
	}
	return results, nil
}

// strFromMap safely extracts a string from a generic map
func strFromMap(m map[string]interface{}, key string) string {
	if v, ok := m[key]; ok {
		if s, ok := v.(string); ok {
			return s
		}
	}
	return ""
}

// getStringNode fetches a string field from an ipld.Node map
func getStringNode(n ipld.Node, key string) string {
	if n == nil {
		return ""
	}
	v, err := n.LookupByString(key)
	if err != nil || v == nil {
		return ""
	}
	if v.Kind() != ipld.Kind_String {
		return ""
	}
	s, err := v.AsString()
	if err != nil {
		return ""
	}
	return s
}

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
