// Package bluesky provides AT Protocol record type definitions
package bluesky

import (
	"encoding/json"
	"time"
)

// ProfileRecord represents an app.bsky.actor.profile record
type ProfileRecord struct {
	DisplayName *string `json:"displayName,omitempty"`
	Description *string `json:"description,omitempty"`
	Avatar      *string `json:"avatar,omitempty"`
	Banner      *string `json:"banner,omitempty"`
	CreatedAt   string  `json:"createdAt"`
}

// PostRecord represents an app.bsky.feed.post record
type PostRecord struct {
	URI       string  `json:"uri"`
	CID       string  `json:"cid"`
	Text      string  `json:"text"`
	CreatedAt string  `json:"createdAt"`
	Embed     *Embed  `json:"embed,omitempty"`
	Facets    []Facet `json:"facets,omitempty"`
	Reply     *Reply  `json:"reply,omitempty"`
}

// Embed represents the top-level embed structure in a post.
// It uses json.RawMessage to delay parsing of media and record fields,
// allowing us to handle different embed types like images, external links,
// records, and records with media.
type Embed struct {
	Type     string           `json:"$type"`
	External *ExternalEmbed   `json:"external,omitempty"`
	Images   []*ImageEmbed    `json:"images,omitempty"`
	Record   *RecordEmbed     `json:"record,omitempty"`
	Media    *json.RawMessage `json:"media,omitempty"` // For recordWithMedia
}

// ExternalEmbed represents an external link card.
type ExternalEmbed struct {
	URI         string   `json:"uri"`
	Title       string   `json:"title"`
	Description string   `json:"description"`
	Thumb       *BlobRef `json:"thumb,omitempty"`
}

// ImageEmbed represents a single image within an embed.
type ImageEmbed struct {
	Alt   string   `json:"alt"`
	Image *BlobRef `json:"image"`
}

// BlobRef represents a reference to a blob (like an image).
type BlobRef struct {
	Ref string `json:"$link"`
}

// RecordEmbed represents a quote post embed.
type RecordEmbed struct {
	URI string `json:"uri"`
	CID string `json:"cid"`
}

// Facet represents text formatting/linking information
type Facet struct {
	Index    IndexRange     `json:"index"`
	Features []FacetFeature `json:"features"`
}

// FacetFeature is a union type for different kinds of features.
type FacetFeature struct {
	Type string `json:"$type"`
	DID  string `json:"did,omitempty"`
	URI  string `json:"uri,omitempty"`
	Tag  string `json:"tag,omitempty"`
}

// IndexRange represents character indices for facets
type IndexRange struct {
	ByteStart int `json:"byteStart"`
	ByteEnd   int `json:"byteEnd"`
}

// Reply represents reply information
type Reply struct {
	Root   RecordRef `json:"root"`
	Parent RecordRef `json:"parent"`
}

// RecordRef represents a reference to another record
type RecordRef struct {
	URI string `json:"uri"`
	CID string `json:"cid"`
}

// ParsedProfile represents a parsed profile with computed fields
type ParsedProfile struct {
	*ProfileRecord
	Handle     string
	DID        string
	PDS        string
	ParsedTime time.Time
}

// ParsedPost represents a parsed post with computed fields
type ParsedPost struct {
	*PostRecord
	Handle         string
	DID            string
	RKey           string
	ParsedTime     time.Time
	SearchableText string // Combined text for searching
}

// Collection type constants
const (
	ProfileCollection    = "app.bsky.actor.profile"
	PostCollection       = "app.bsky.feed.post"
	EmbedImages          = "app.bsky.embed.images"
	EmbedExternal        = "app.bsky.embed.external"
	EmbedRecord          = "app.bsky.embed.record"
	EmbedRecordWithMedia = "app.bsky.embed.recordWithMedia"
)
