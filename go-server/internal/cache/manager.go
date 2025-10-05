// Package cache provides cache management for CAR files and metadata
package cache

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/oyin-bo/autoreply/go-server/pkg/errors"
	"golang.org/x/sync/singleflight"
)

// Manager handles caching of CAR files and their metadata
type Manager struct {
	cacheDir string
	locks    sync.Map
	sf       singleflight.Group
}

// Metadata represents cache metadata stored alongside CAR files
type Metadata struct {
	DID           string  `json:"did"`
	ETag          *string `json:"etag,omitempty"`
	LastModified  *string `json:"lastModified,omitempty"`
	ContentLength *int64  `json:"contentLength,omitempty"`
	CachedAt      int64   `json:"cachedAt"`
	TTLHours      int64   `json:"ttlHours"`
}

// NewManager creates a new cache manager with platform-specific cache directory
func NewManager() (*Manager, error) {
	cacheDir, err := getCacheDir()
	if err != nil {
		return nil, errors.Wrap(err, errors.CacheError, "Failed to determine cache directory")
	}
	// Ensure cache directory exists
	if err := os.MkdirAll(cacheDir, 0755); err != nil {
		return nil, errors.Wrap(err, errors.CacheError, "Failed to create cache directory")
	}

	return &Manager{
		cacheDir: cacheDir,
	}, nil
}

// getCacheDir determines the platform-specific cache directory
func getCacheDir() (string, error) {
	userCacheDir, err := os.UserCacheDir()
	if err != nil {
		return "", fmt.Errorf("failed to get user cache directory: %w", err)
	}
	// New root: <OS cache>/autoreply/did
	return filepath.Join(userCacheDir, "autoreply", "did"), nil
}

// sanitizeDID returns a filesystem-friendly identifier by stripping method prefixes
// while keeping the rest of the DID intact (e.g.,
//
//	did:web:example.com -> example.com
//	did:plc:abc...      -> abc...)
//
// For unknown methods, returns the input unchanged.
func sanitizeDID(did string) string {
	// Strip known method prefixes
	var s string
	if strings.HasPrefix(did, "did:web:") {
		s = strings.TrimPrefix(did, "did:web:")
	} else if strings.HasPrefix(did, "did:plc:") {
		s = strings.TrimPrefix(did, "did:plc:")
	} else {
		s = did
	}

	// Replace colon separators (common in did:web path form) with double underscores
	s = strings.ReplaceAll(s, ":", "__")

	// Map any remaining characters outside [A-Za-z0-9._-] to underscore
	out := make([]rune, 0, len(s))
	for _, r := range s {
		switch {
		case r >= 'a' && r <= 'z':
			out = append(out, r)
		case r >= 'A' && r <= 'Z':
			out = append(out, r)
		case r >= '0' && r <= '9':
			out = append(out, r)
		case r == '.' || r == '-' || r == '_':
			out = append(out, r)
		default:
			out = append(out, '_')
		}
	}
	return string(out)
}

// GetCachePath returns the cache path for a DID using two-tier structure
func (m *Manager) GetCachePath(did string) (string, error) {
	if did == "" {
		return "", errors.NewMCPError(errors.InvalidInput, "DID cannot be empty")
	}

	// Use sanitized DID as directory name for readability
	sanitized := sanitizeDID(did)
	if sanitized == "" {
		return "", errors.NewMCPError(errors.InvalidInput, "Invalid DID format")
	}

	// Tier prefix: first two characters of sanitized DID (or entire string if shorter)
	prefix := sanitized
	if len(prefix) > 2 {
		prefix = sanitized[:2]
	}

	// Final directory uses sanitized DID for cross-platform safety
	return filepath.Join(m.cacheDir, prefix, sanitized), nil
}

// GetFilePaths returns paths for CAR file and metadata
func (m *Manager) GetFilePaths(did string) (carPath, metadataPath string, err error) {
	cachePath, err := m.GetCachePath(did)
	if err != nil {
		return "", "", err
	}
	carPath = filepath.Join(cachePath, "repo.car")
	metadataPath = filepath.Join(cachePath, "metadata.json")
	return carPath, metadataPath, nil
}

// IsCacheValid checks if cached data is valid and not expired
func (m *Manager) IsCacheValid(did string, ttlHours int64) bool {
	_, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return false
	}

	// Check if files exist
	if _, err := os.Stat(metadataPath); os.IsNotExist(err) {
		return false
	}

	// Read metadata
	metadata, err := m.GetMetadata(did)
	if err != nil {
		return false
	}

	// Check if expired
	cachedAt := time.Unix(metadata.CachedAt, 0)
	expiry := cachedAt.Add(time.Duration(ttlHours) * time.Hour)
	return time.Now().Before(expiry)
}

// GetMetadata reads cached metadata for a DID
func (m *Manager) GetMetadata(did string) (*Metadata, error) {
	_, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return nil, err
	}

	data, err := os.ReadFile(metadataPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, errors.NewMCPError(errors.NotFound, "Metadata not found")
		}
		return nil, errors.Wrap(err, errors.CacheError, "Failed to read metadata")
	}

	var metadata Metadata
	if err := json.Unmarshal(data, &metadata); err != nil {
		return nil, errors.Wrap(err, errors.CacheError, "Failed to parse metadata")
	}

	return &metadata, nil
}

// StoreCar stores CAR file and metadata atomically
func (m *Manager) StoreCar(did string, carData []byte, metadata Metadata) error {
	carPath, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return err
	}

	// Ensure directory exists
	if err := os.MkdirAll(filepath.Dir(carPath), 0755); err != nil {
		return errors.Wrap(err, errors.CacheError, "Failed to create cache directory")
	}

	// Use temporary files for atomic writes
	carTmpPath := carPath + ".tmp"
	metadataTmpPath := metadataPath + ".tmp"

	// Get or create file lock for this DID
	lockKey := fmt.Sprintf("lock_%s", did)
	lockValue, _ := m.locks.LoadOrStore(lockKey, &sync.Mutex{})
	lock := lockValue.(*sync.Mutex)

	lock.Lock()
	defer lock.Unlock()

	// Write CAR file
	if err := os.WriteFile(carTmpPath, carData, 0644); err != nil {
		return errors.Wrap(err, errors.CacheError, "Failed to write CAR file")
	}

	// Write metadata
	metadataJSON, err := json.MarshalIndent(metadata, "", "  ")
	if err != nil {
		os.Remove(carTmpPath) // Cleanup on failure
		return errors.Wrap(err, errors.CacheError, "Failed to marshal metadata")
	}

	if err := os.WriteFile(metadataTmpPath, metadataJSON, 0644); err != nil {
		os.Remove(carTmpPath) // Cleanup on failure
		return errors.Wrap(err, errors.CacheError, "Failed to write metadata")
	}

	// Atomic rename
	if err := os.Rename(carTmpPath, carPath); err != nil {
		os.Remove(carTmpPath)
		os.Remove(metadataTmpPath)
		return errors.Wrap(err, errors.CacheError, "Failed to move CAR file")
	}

	if err := os.Rename(metadataTmpPath, metadataPath); err != nil {
		os.Remove(metadataTmpPath)
		return errors.Wrap(err, errors.CacheError, "Failed to move metadata")
	}

	return nil
}

// ReadCar reads cached CAR file data
func (m *Manager) ReadCar(did string) ([]byte, error) {
	carPath, _, err := m.GetFilePaths(did)
	if err != nil {
		return nil, err
	}

	data, err := os.ReadFile(carPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, errors.NewMCPError(errors.NotFound, "CAR file not found")
		}
		return nil, errors.Wrap(err, errors.CacheError, "Failed to read CAR file")
	}

	return data, nil
}

// CleanupExpired removes expired cache entries
func (m *Manager) CleanupExpired() error {
	return filepath.Walk(m.cacheDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil // Continue walking even if there are errors
		}

		// Skip if not a metadata file
		if filepath.Base(path) != "metadata.json" {
			return nil
		}

		// Try to read metadata
		data, err := os.ReadFile(path)
		if err != nil {
			return nil // Continue if can't read
		}

		var metadata Metadata
		if err := json.Unmarshal(data, &metadata); err != nil {
			return nil // Continue if can't parse
		}

		// Check if expired
		cachedAt := time.Unix(metadata.CachedAt, 0)
		expiry := cachedAt.Add(time.Duration(metadata.TTLHours) * time.Hour)

		if time.Now().After(expiry) {
			// Remove the entire DID directory
			didDir := filepath.Dir(path)
			os.RemoveAll(didDir)
		}

		return nil
	})
}
