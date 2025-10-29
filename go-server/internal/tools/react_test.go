// Package tools provides MCP tool implementations
package tools

import (
	"strings"
	"testing"
)

// TestReactToolBasics tests basic tool properties
func TestReactToolBasics(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	t.Run("Name", func(t *testing.T) {
		if tool.Name() != "react" {
			t.Errorf("Expected name 'react', got '%s'", tool.Name())
		}
	})

	t.Run("Description", func(t *testing.T) {
		desc := tool.Description()
		if desc == "" {
			t.Error("Description should not be empty")
		}
		descLower := strings.ToLower(desc)
		if !strings.Contains(descLower, "react") && !strings.Contains(descLower, "like") && !strings.Contains(descLower, "repost") {
			t.Errorf("Description should mention reactions, got: %s", desc)
		}
	})

	t.Run("InputSchema", func(t *testing.T) {
		schema := tool.InputSchema()

		if schema.Type != "object" {
			t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
		}

		// Check for reactAs parameter (optional)
		if reactAsProp, ok := schema.Properties["reactAs"]; ok {
			if reactAsProp.Type != "string" {
				t.Errorf("ReactAs property should be string, got %s", reactAsProp.Type)
			}
		} else {
			t.Error("Schema missing 'reactAs' property")
		}

		// Check for like parameter (array, optional)
		if likeProp, ok := schema.Properties["like"]; ok {
			if likeProp.Type != "array" {
				t.Errorf("Like property should be array, got %s", likeProp.Type)
			}
		} else {
			t.Error("Schema missing 'like' property")
		}

		// Check for unlike parameter (array, optional)
		if unlikeProp, ok := schema.Properties["unlike"]; ok {
			if unlikeProp.Type != "array" {
				t.Errorf("Unlike property should be array, got %s", unlikeProp.Type)
			}
		} else {
			t.Error("Schema missing 'unlike' property")
		}

		// Check for repost parameter (array, optional)
		if repostProp, ok := schema.Properties["repost"]; ok {
			if repostProp.Type != "array" {
				t.Errorf("Repost property should be array, got %s", repostProp.Type)
			}
		} else {
			t.Error("Schema missing 'repost' property")
		}

		// Check for delete parameter (array, optional)
		if deleteProp, ok := schema.Properties["delete"]; ok {
			if deleteProp.Type != "array" {
				t.Errorf("Delete property should be array, got %s", deleteProp.Type)
			}
		} else {
			t.Error("Schema missing 'delete' property")
		}

		// None should be required
		if len(schema.Required) > 0 {
			t.Errorf("No parameters should be required, but found: %v", schema.Required)
		}
	})
}

// TestFormatResults tests the result formatting
func TestFormatResults(t *testing.T) {
	tool, err := NewReactTool()
	if err != nil {
		t.Fatalf("Failed to create react tool: %v", err)
	}

	t.Run("All successes", func(t *testing.T) {
		results := &OperationResults{
			Successes: []string{
				"Liked: at://did:plc:abc/app.bsky.feed.post/123",
				"Reposted: at://did:plc:abc/app.bsky.feed.post/456",
			},
			Failures: []OperationFailure{},
		}

		output := tool.formatResults(results)

		if !strings.Contains(output, "✓ Successful Operations (2)") {
			t.Error("Output should show successful operations count")
		}
		if !strings.Contains(output, "2 successful, 0 failed") {
			t.Error("Output should show summary")
		}
	})

	t.Run("All failures", func(t *testing.T) {
		results := &OperationResults{
			Successes: []string{},
			Failures: []OperationFailure{
				{
					Operation: "like",
					URI:       "at://did:plc:abc/app.bsky.feed.post/123",
					Error:     "not found",
				},
			},
		}

		output := tool.formatResults(results)

		if !strings.Contains(output, "✗ Failed Operations (1)") {
			t.Error("Output should show failed operations count")
		}
		if !strings.Contains(output, "0 successful, 1 failed") {
			t.Error("Output should show summary")
		}
	})

	t.Run("Mixed results", func(t *testing.T) {
		results := &OperationResults{
			Successes: []string{
				"Liked: at://did:plc:abc/app.bsky.feed.post/123",
			},
			Failures: []OperationFailure{
				{
					Operation: "delete",
					URI:       "at://did:plc:abc/app.bsky.feed.post/456",
					Error:     "permission denied",
				},
			},
		}

		output := tool.formatResults(results)

		if !strings.Contains(output, "✓ Successful Operations (1)") {
			t.Error("Output should show successful operations count")
		}
		if !strings.Contains(output, "✗ Failed Operations (1)") {
			t.Error("Output should show failed operations count")
		}
		if !strings.Contains(output, "1 successful, 1 failed") {
			t.Error("Output should show summary")
		}
	})
}
