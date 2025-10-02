package auth

import "time"

// Credentials represents stored authentication credentials for a BlueSky account
type Credentials struct {
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	DPoPKey      string    `json:"dpop_key"`
	ExpiresAt    time.Time `json:"expires_at"`
}

// Account represents metadata for an authenticated account
type Account struct {
	Handle     string            `json:"handle"`
	DID        string            `json:"did"`
	PDS        string            `json:"pds"`
	StorageRef string            `json:"storage_ref"` // "keyring", "encrypted", or "plaintext"
	CreatedAt  time.Time         `json:"created_at"`
	LastUsed   time.Time         `json:"last_used"`
	Metadata   map[string]string `json:"metadata,omitempty"`
}

// Config represents the authentication configuration
type Config struct {
	Version        string    `json:"version"`
	Accounts       []Account `json:"accounts"`
	DefaultAccount *string   `json:"default_account,omitempty"`
	Settings       Settings  `json:"settings"`
}

// Settings for authentication behavior
type Settings struct {
	AutoRefresh            bool `json:"auto_refresh"`
	RefreshThresholdMinutes int  `json:"refresh_threshold_minutes"`
	TokenRotationDays      int  `json:"token_rotation_days"`
}

// DefaultConfig returns a new configuration with default values
func DefaultConfig() *Config {
	return &Config{
		Version:  "2.0",
		Accounts: []Account{},
		Settings: Settings{
			AutoRefresh:            true,
			RefreshThresholdMinutes: 5,
			TokenRotationDays:      30,
		},
	}
}

// GetAccount finds an account by handle
func (c *Config) GetAccount(handle string) *Account {
	for i := range c.Accounts {
		if c.Accounts[i].Handle == handle {
			return &c.Accounts[i]
		}
	}
	return nil
}

// AddAccount adds or updates an account in the configuration
func (c *Config) AddAccount(account Account) {
	for i := range c.Accounts {
		if c.Accounts[i].Handle == account.Handle {
			c.Accounts[i] = account
			return
		}
	}
	c.Accounts = append(c.Accounts, account)
}

// RemoveAccount removes an account from the configuration
func (c *Config) RemoveAccount(handle string) bool {
	for i := range c.Accounts {
		if c.Accounts[i].Handle == handle {
			c.Accounts = append(c.Accounts[:i], c.Accounts[i+1:]...)
			return true
		}
	}
	return false
}

// UpdateLastUsed updates the last used timestamp for an account
func (c *Config) UpdateLastUsed(handle string) {
	if account := c.GetAccount(handle); account != nil {
		account.LastUsed = time.Now()
	}
}

// NeedsRefresh checks if credentials need to be refreshed
func (c *Credentials) NeedsRefresh(thresholdMinutes int) bool {
	threshold := time.Duration(thresholdMinutes) * time.Minute
	return time.Until(c.ExpiresAt) < threshold
}
