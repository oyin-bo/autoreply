// Package bluesky provides AT Protocol record type definitions
package bluesky

import "time"

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
	Embeds    []Embed `json:"embeds,omitempty"`
	Facets    []Facet `json:"facets,omitempty"`
	Reply     *Reply  `json:"reply,omitempty"`
}

// Embed represents embedded content in a post
type Embed struct {
	Type     string    `json:"$type"`
	External *External `json:"external,omitempty"`
	Images   []Image   `json:"images,omitempty"`
	Record   *Record   `json:"record,omitempty"`
}

// External represents an external link embed
type External struct {
	Title       string `json:"title"`
	Description string `json:"description"`
	URI         string `json:"uri"`
}

// Image represents an image embed
type Image struct {
	Alt   string `json:"alt"`
	Image struct {
		Ref string `json:"$link"`
	} `json:"image"`
}

// Record represents a record embed (quote post)
type Record struct {
	URI    string `json:"uri"`
	CID    string `json:"cid"`
	Record struct {
		Text      string `json:"text,omitempty"`
		CreatedAt string `json:"createdAt,omitempty"`
	} `json:"record,omitempty"`
}

// Facet represents text formatting/linking information
type Facet struct {
	Index    IndexRange    `json:"index"`
	Features []interface{} `json:"features"`
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
	ProfileCollection = "app.bsky.actor.profile"
	PostCollection    = "app.bsky.feed.post"
)
