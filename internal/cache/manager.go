// manager.go - Cache management implementation
package cache

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/oyin-bo/autoreply/pkg/errors"
)

// Manager handles cache operations
type Manager struct {
	cacheDir string
}

// Metadata represents cache metadata stored alongside CAR files
type Metadata struct {
	DID           string    `json:"did"`
	ETag          string    `json:"etag,omitempty"`
	LastModified  string    `json:"lastModified,omitempty"`
	ContentLength int64     `json:"contentLength,omitempty"`
	CachedAt      time.Time `json:"cachedAt"`
	TTLHours      int       `json:"ttlHours"`
}

// NewManager creates a new cache manager
func NewManager() (*Manager, error) {
	cacheDir, err := getCacheDir()
	if err != nil {
		return nil, fmt.Errorf("failed to determine cache directory: %w", err)
	}

	// Ensure cache directory exists
	if err := os.MkdirAll(cacheDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create cache directory: %w", err)
	}

	return &Manager{
		cacheDir: cacheDir,
	}, nil
}

// getCacheDir determines the platform-specific cache directory
func getCacheDir() (string, error) {
	// Try XDG_CACHE_HOME first
	if xdgCache := os.Getenv("XDG_CACHE_HOME"); xdgCache != "" {
		return filepath.Join(xdgCache, "bluesky-mcp"), nil
	}

	// Get user cache directory (cross-platform)
	userCacheDir, err := os.UserCacheDir()
	if err != nil {
		// Fallback to home directory
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", fmt.Errorf("unable to determine cache directory: %w", err)
		}
		return filepath.Join(homeDir, ".cache", "bluesky-mcp"), nil
	}

	return filepath.Join(userCacheDir, "bluesky-mcp"), nil
}

// GetCachePath returns the cache path for a DID using two-tier structure
func (m *Manager) GetCachePath(did string) (string, error) {
	if !strings.HasPrefix(did, "did:plc:") {
		return "", errors.NewMcpError(errors.InvalidInput, "Invalid DID format")
	}

	// Calculate hash prefix from DID for two-tier structure
	hash := sha256.Sum256([]byte(did))
	prefix := fmt.Sprintf("%02x", hash[:1])

	cachePath := filepath.Join(m.cacheDir, prefix, did)
	
	// Ensure directory exists
	if err := os.MkdirAll(cachePath, 0755); err != nil {
		return "", errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to create cache directory: %v", err))
	}

	return cachePath, nil
}

// GetFilePaths returns the paths for CAR file and metadata
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
func (m *Manager) IsCacheValid(did string, ttlHours int) bool {
	_, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return false
	}

	// Check if metadata file exists
	metadataBytes, err := os.ReadFile(metadataPath)
	if err != nil {
		return false
	}

	var metadata Metadata
	if err := json.Unmarshal(metadataBytes, &metadata); err != nil {
		return false
	}

	// Check if cache has expired
	expiryTime := metadata.CachedAt.Add(time.Duration(ttlHours) * time.Hour)
	if time.Now().After(expiryTime) {
		return false
	}

	// Check if CAR file exists
	carPath, _, err := m.GetFilePaths(did)
	if err != nil {
		return false
	}

	if _, err := os.Stat(carPath); os.IsNotExist(err) {
		return false
	}

	return true
}

// GetMetadata retrieves cached metadata
func (m *Manager) GetMetadata(did string) (*Metadata, error) {
	_, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return nil, err
	}

	metadataBytes, err := os.ReadFile(metadataPath)
	if err != nil {
		return nil, errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to read metadata: %v", err))
	}

	var metadata Metadata
	if err := json.Unmarshal(metadataBytes, &metadata); err != nil {
		return nil, errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to parse metadata: %v", err))
	}

	return &metadata, nil
}

// StoreData stores CAR file and metadata atomically
func (m *Manager) StoreData(did string, carData []byte, metadata *Metadata) error {
	carPath, metadataPath, err := m.GetFilePaths(did)
	if err != nil {
		return err
	}

	// Write CAR file atomically using temporary file
	carTempPath := carPath + ".tmp"
	if err := os.WriteFile(carTempPath, carData, 0644); err != nil {
		return errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to write CAR temp file: %v", err))
	}

	// Write metadata atomically
	metadataBytes, err := json.MarshalIndent(metadata, "", "  ")
	if err != nil {
		os.Remove(carTempPath)
		return errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to marshal metadata: %v", err))
	}

	metadataTempPath := metadataPath + ".tmp"
	if err := os.WriteFile(metadataTempPath, metadataBytes, 0644); err != nil {
		os.Remove(carTempPath)
		return errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to write metadata temp file: %v", err))
	}

	// Atomically move files into place
	if err := os.Rename(carTempPath, carPath); err != nil {
		os.Remove(carTempPath)
		os.Remove(metadataTempPath)
		return errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to move CAR file: %v", err))
	}

	if err := os.Rename(metadataTempPath, metadataPath); err != nil {
		os.Remove(metadataTempPath)
		return errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to move metadata file: %v", err))
	}

	return nil
}

// GetCARData retrieves cached CAR data
func (m *Manager) GetCARData(did string) ([]byte, error) {
	carPath, _, err := m.GetFilePaths(did)
	if err != nil {
		return nil, err
	}

	data, err := os.ReadFile(carPath)
	if err != nil {
		return nil, errors.NewMcpError(errors.CacheError, fmt.Sprintf("Failed to read CAR file: %v", err))
	}

	return data, nil
}

// CleanupExpired removes expired cache entries
func (m *Manager) CleanupExpired() error {
	// Walk through cache directory structure
	return filepath.Walk(m.cacheDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		// Look for metadata files
		if info.Name() == "metadata.json" {
			metadataBytes, err := os.ReadFile(path)
			if err != nil {
				return nil // Skip problematic files
			}

			var metadata Metadata
			if err := json.Unmarshal(metadataBytes, &metadata); err != nil {
				return nil // Skip invalid metadata
			}

			// Check if expired
			expiryTime := metadata.CachedAt.Add(time.Duration(metadata.TTLHours) * time.Hour)
			if time.Now().After(expiryTime) {
				// Remove entire DID directory
				didDir := filepath.Dir(path)
				os.RemoveAll(didDir)
			}
		}

		return nil
	})
}