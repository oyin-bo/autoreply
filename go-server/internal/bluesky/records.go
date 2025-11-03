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

// GetSearchableText extracts all text content from an embed for indexing.
func (e *Embed) GetSearchableText() []string {
	var texts []string
	if e == nil {
		return texts
	}

	switch e.Type {
	case EmbedImages:
		for _, img := range e.Images {
			if img.Alt != "" {
				texts = append(texts, img.Alt)
			}
		}
	case EmbedExternal:
		if e.External != nil {
			texts = append(texts, e.External.Title)
			texts = append(texts, e.External.Description)
		}
	case EmbedRecordWithMedia:
		if e.Media != nil {
			// The 'media' field contains another embed. We need to unmarshal it
			// to determine its type and extract its text.
			var nestedEmbed Embed
			if err := json.Unmarshal(*e.Media, &nestedEmbed); err == nil {
				texts = append(texts, nestedEmbed.GetSearchableText()...)
			}
		}
	case EmbedRecord:
		// Simple record embeds (quote posts) do not contain the text of the
		// quoted post directly in the embed, so there's no text to add.
	}
	return texts
}

// GetSearchableText extracts all text from a post, including its embeds.
func (p *PostRecord) GetSearchableText() []string {
	texts := []string{p.Text}

	if p.Embed != nil {
		texts = append(texts, p.Embed.GetSearchableText()...)
	}

	for _, facet := range p.Facets {
		for _, feature := range facet.Features {
			if feature.Type == "app.bsky.richtext.facet#link" {
				texts = append(texts, feature.URI)
			}
		}
	}

	return texts
}
