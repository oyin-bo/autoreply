// records.go - AT Protocol record types
package bluesky

import (
	"time"
)

// ProfileRecord represents app.bsky.actor.profile record
type ProfileRecord struct {
	Type        string  `json:"$type,omitempty" cbor:"$type,omitempty"`
	DisplayName *string `json:"displayName,omitempty" cbor:"displayName,omitempty"`
	Description *string `json:"description,omitempty" cbor:"description,omitempty"`
	Avatar      *string `json:"avatar,omitempty" cbor:"avatar,omitempty"`
	Banner      *string `json:"banner,omitempty" cbor:"banner,omitempty"`
	CreatedAt   string  `json:"createdAt" cbor:"createdAt"`
}

// PostRecord represents app.bsky.feed.post record
type PostRecord struct {
	Type      string    `json:"$type,omitempty" cbor:"$type,omitempty"`
	URI       string    `json:"uri,omitempty" cbor:"uri,omitempty"`
	CID       string    `json:"cid,omitempty" cbor:"cid,omitempty"`
	Text      string    `json:"text" cbor:"text"`
	CreatedAt string    `json:"createdAt" cbor:"createdAt"`
	Embed     *Embed    `json:"embed,omitempty" cbor:"embed,omitempty"`
	Facets    []Facet   `json:"facets,omitempty" cbor:"facets,omitempty"`
	Reply     *Reply    `json:"reply,omitempty" cbor:"reply,omitempty"`
	Langs     []string  `json:"langs,omitempty" cbor:"langs,omitempty"`
}

// Embed represents various embed types
type Embed struct {
	Type     string      `json:"$type,omitempty" cbor:"$type,omitempty"`
	External *External   `json:"external,omitempty" cbor:"external,omitempty"`
	Images   []Image     `json:"images,omitempty" cbor:"images,omitempty"`
	Record   *EmbedRecord `json:"record,omitempty" cbor:"record,omitempty"`
}

// External represents external link embeds
type External struct {
	URI         string `json:"uri" cbor:"uri"`
	Title       string `json:"title" cbor:"title"`
	Description string `json:"description" cbor:"description"`
	Thumb       *Blob  `json:"thumb,omitempty" cbor:"thumb,omitempty"`
}

// Image represents image embeds
type Image struct {
	Alt   string `json:"alt" cbor:"alt"`
	Image Blob   `json:"image" cbor:"image"`
}

// EmbedRecord represents record embeds (quotes, etc.)
type EmbedRecord struct {
	Record *StrongRef `json:"record,omitempty" cbor:"record,omitempty"`
}

// Blob represents a blob reference
type Blob struct {
	Type     string `json:"$type,omitempty" cbor:"$type,omitempty"`
	Ref      string `json:"$link,omitempty" cbor:"$link,omitempty"`
	MimeType string `json:"mimeType,omitempty" cbor:"mimeType,omitempty"`
	Size     int64  `json:"size,omitempty" cbor:"size,omitempty"`
}

// Facet represents text annotations
type Facet struct {
	Index    ByteSlice `json:"index" cbor:"index"`
	Features []Feature `json:"features" cbor:"features"`
}

// ByteSlice represents byte positions
type ByteSlice struct {
	ByteStart int `json:"byteStart" cbor:"byteStart"`
	ByteEnd   int `json:"byteEnd" cbor:"byteEnd"`
}

// Feature represents facet features (links, mentions, etc.)
type Feature struct {
	Type string `json:"$type" cbor:"$type"`
	URI  string `json:"uri,omitempty" cbor:"uri,omitempty"`
	DID  string `json:"did,omitempty" cbor:"did,omitempty"`
}

// Reply represents reply metadata
type Reply struct {
	Root   StrongRef `json:"root" cbor:"root"`
	Parent StrongRef `json:"parent" cbor:"parent"`
}

// StrongRef represents a strong reference to another record
type StrongRef struct {
	URI string `json:"uri" cbor:"uri"`
	CID string `json:"cid" cbor:"cid"`
}

// ParsedDateTime parses an AT Protocol datetime string
type ParsedDateTime struct {
	time.Time
}

// Helper functions

// GetDisplayName returns the display name or handle as fallback
func (p *ProfileRecord) GetDisplayName(handle string) string {
	if p.DisplayName != nil && *p.DisplayName != "" {
		return *p.DisplayName
	}
	return handle
}

// GetDescription returns the description or empty string
func (p *ProfileRecord) GetDescription() string {
	if p.Description != nil {
		return *p.Description
	}
	return ""
}

// GetCreatedAt parses and returns the created at time
func (p *ProfileRecord) GetCreatedAt() (time.Time, error) {
	return time.Parse(time.RFC3339, p.CreatedAt)
}

// GetCreatedAt parses and returns the post created at time
func (p *PostRecord) GetCreatedAt() (time.Time, error) {
	return time.Parse(time.RFC3339, p.CreatedAt)
}

// HasImages checks if the post has image embeds
func (p *PostRecord) HasImages() bool {
	return p.Embed != nil && len(p.Embed.Images) > 0
}

// HasExternal checks if the post has external link embeds
func (p *PostRecord) HasExternal() bool {
	return p.Embed != nil && p.Embed.External != nil
}

// GetExternalTitle returns the external link title
func (p *PostRecord) GetExternalTitle() string {
	if p.HasExternal() {
		return p.Embed.External.Title
	}
	return ""
}

// GetExternalDescription returns the external link description
func (p *PostRecord) GetExternalDescription() string {
	if p.HasExternal() {
		return p.Embed.External.Description
	}
	return ""
}

// GetExternalURL returns the external link URL
func (p *PostRecord) GetExternalURL() string {
	if p.HasExternal() {
		return p.Embed.External.URI
	}
	return ""
}