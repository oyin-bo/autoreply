package bluesky

import (
	"testing"
)

func TestParsePostURI(t *testing.T) {
	tests := []struct {
		name        string
		uri         string
		wantDID     string
		wantRKey    string
		wantErr     bool
	}{
		{
			name:     "at URI with DID",
			uri:      "at://did:plc:abc123/app.bsky.feed.post/xyz789",
			wantDID:  "did:plc:abc123",
			wantRKey: "xyz789",
			wantErr:  false,
		},
		{
			name:     "bsky.app URL with handle",
			uri:      "https://bsky.app/profile/alice.bsky.social/post/abc123",
			wantDID:  "alice.bsky.social",
			wantRKey: "abc123",
			wantErr:  false,
		},
		{
			name:     "bsky.app URL with DID",
			uri:      "https://bsky.app/profile/did:plc:xyz/post/abc123",
			wantDID:  "did:plc:xyz",
			wantRKey: "abc123",
			wantErr:  false,
		},
		{
			name:     "gist.ing URL",
			uri:      "https://gist.ing/profile/alice.bsky.social/post/xyz789",
			wantDID:  "alice.bsky.social",
			wantRKey: "xyz789",
			wantErr:  false,
		},
		{
			name:    "empty URI",
			uri:     "",
			wantErr: true,
		},
		{
			name:    "invalid URI",
			uri:     "not-a-valid-uri",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParsePostURI(tt.uri)
			if (err != nil) != tt.wantErr {
				t.Errorf("ParsePostURI() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if !tt.wantErr {
				if got.DID != tt.wantDID {
					t.Errorf("ParsePostURI() DID = %v, want %v", got.DID, tt.wantDID)
				}
				if got.RKey != tt.wantRKey {
					t.Errorf("ParsePostURI() RKey = %v, want %v", got.RKey, tt.wantRKey)
				}
				if got.Collection != "app.bsky.feed.post" {
					t.Errorf("ParsePostURI() Collection = %v, want app.bsky.feed.post", got.Collection)
				}
			}
		})
	}
}

func TestMakePostURI(t *testing.T) {
	tests := []struct {
		name string
		did  string
		rkey string
		want string
	}{
		{
			name: "basic post URI",
			did:  "did:plc:abc123",
			rkey: "xyz789",
			want: "at://did:plc:abc123/app.bsky.feed.post/xyz789",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := MakePostURI(tt.did, tt.rkey); got != tt.want {
				t.Errorf("MakePostURI() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestIsLikelyDID(t *testing.T) {
	tests := []struct {
		name string
		s    string
		want bool
	}{
		{"valid DID", "did:plc:abc123", true},
		{"valid DID with whitespace", "  did:web:example.com  ", true},
		{"handle", "alice.bsky.social", false},
		{"empty", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsLikelyDID(tt.s); got != tt.want {
				t.Errorf("IsLikelyDID() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestNormalizeHandle(t *testing.T) {
	tests := []struct {
		name   string
		handle string
		want   string
	}{
		{"with @", "@alice.bsky.social", "alice.bsky.social"},
		{"without @", "alice.bsky.social", "alice.bsky.social"},
		{"with whitespace", "  @alice.bsky.social  ", "alice.bsky.social"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := NormalizeHandle(tt.handle); got != tt.want {
				t.Errorf("NormalizeHandle() = %v, want %v", got, tt.want)
			}
		})
	}
}
