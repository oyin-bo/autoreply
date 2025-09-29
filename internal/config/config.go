// config.go - Configuration management
package config

import (
	"os"
	"strconv"
	"time"
)

// Config holds application configuration
type Config struct {
	// Cache settings
	CacheTTLHours     int
	ProfileTTLHours   int
	
	// HTTP client settings
	HTTPTimeout       time.Duration
	DIDResolveTimeout time.Duration
	CARDownloadTimeout time.Duration
	
	// Request settings
	TotalRequestTimeout time.Duration
	MaxQueryLength      int
	
	// Cache cleanup
	EnableCacheCleanup bool
	CleanupInterval    time.Duration
}

// LoadConfig loads configuration from environment variables with defaults
func LoadConfig() *Config {
	return &Config{
		CacheTTLHours:       getEnvInt("BLUESKY_CACHE_TTL_HOURS", 24),
		ProfileTTLHours:     getEnvInt("BLUESKY_PROFILE_TTL_HOURS", 1),
		HTTPTimeout:         time.Duration(getEnvInt("BLUESKY_HTTP_TIMEOUT_SECONDS", 60)) * time.Second,
		DIDResolveTimeout:   time.Duration(getEnvInt("BLUESKY_DID_TIMEOUT_SECONDS", 10)) * time.Second,
		CARDownloadTimeout:  time.Duration(getEnvInt("BLUESKY_CAR_TIMEOUT_SECONDS", 60)) * time.Second,
		TotalRequestTimeout: time.Duration(getEnvInt("BLUESKY_TOTAL_TIMEOUT_SECONDS", 120)) * time.Second,
		MaxQueryLength:      getEnvInt("BLUESKY_MAX_QUERY_LENGTH", 500),
		EnableCacheCleanup:  getEnvBool("BLUESKY_ENABLE_CACHE_CLEANUP", true),
		CleanupInterval:     time.Duration(getEnvInt("BLUESKY_CLEANUP_INTERVAL_HOURS", 24)) * time.Hour,
	}
}

// getEnvInt gets an integer environment variable with a default value
func getEnvInt(key string, defaultValue int) int {
	if value := os.Getenv(key); value != "" {
		if intValue, err := strconv.Atoi(value); err == nil {
			return intValue
		}
	}
	return defaultValue
}

// getEnvBool gets a boolean environment variable with a default value
func getEnvBool(key string, defaultValue bool) bool {
	if value := os.Getenv(key); value != "" {
		if boolValue, err := strconv.ParseBool(value); err == nil {
			return boolValue
		}
	}
	return defaultValue
}