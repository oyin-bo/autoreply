// Package config provides configuration management
package config

import (
	"os"
	"strconv"
	"time"
)

// Config holds application configuration
type Config struct {
	// Cache settings
	CacheTTLHours   int64
	ProfileTTLHours int64
	CleanupInterval time.Duration

	// HTTP client settings
	RequestTimeout  time.Duration
	DownloadTimeout time.Duration

	// Server settings
	MaxQueryLength         int
	MaxConcurrentDownloads int
}

// LoadConfig loads configuration from environment variables with defaults
func LoadConfig() *Config {
	return &Config{
		CacheTTLHours:          getEnvInt64("CACHE_TTL_HOURS", 24),
		ProfileTTLHours:        getEnvInt64("PROFILE_TTL_HOURS", 1),
		CleanupInterval:        getEnvDuration("CLEANUP_INTERVAL", "1h"),
		RequestTimeout:         getEnvDuration("REQUEST_TIMEOUT", "10s"),
		DownloadTimeout:        getEnvDuration("DOWNLOAD_TIMEOUT", "60s"),
		MaxQueryLength:         getEnvInt("MAX_QUERY_LENGTH", 500),
		MaxConcurrentDownloads: getEnvInt("MAX_CONCURRENT_DOWNLOADS", 4),
	}
}

// getEnvInt64 gets an int64 from environment or returns default
func getEnvInt64(key string, defaultValue int64) int64 {
	if value := os.Getenv(key); value != "" {
		if parsed, err := strconv.ParseInt(value, 10, 64); err == nil {
			return parsed
		}
	}
	return defaultValue
}

// getEnvInt gets an int from environment or returns default
func getEnvInt(key string, defaultValue int) int {
	if value := os.Getenv(key); value != "" {
		if parsed, err := strconv.Atoi(value); err == nil {
			return parsed
		}
	}
	return defaultValue
}

// getEnvDuration gets a duration from environment or returns default
func getEnvDuration(key string, defaultValue string) time.Duration {
	value := os.Getenv(key)
	if value == "" {
		value = defaultValue
	}

	if duration, err := time.ParseDuration(value); err == nil {
		return duration
	}

	// Parse default if parsing failed
	if duration, err := time.ParseDuration(defaultValue); err == nil {
		return duration
	}

	return time.Hour // Ultimate fallback
}
