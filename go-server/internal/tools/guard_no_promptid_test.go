package tools

import (
	"strings"
	"testing"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
)

// TestNoPromptIDInSchemas ensures no tool InputSchema includes a prompt_id field or mentions it
func TestNoPromptIDInSchemas(t *testing.T) {
	// Instantiate tools
	loginTool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	profileTool := NewProfileTool()
	searchTool := NewSearchTool()

	// Local helper to check a schema
	checkSchema := func(toolName string, schema mcp.InputSchema) {
		for key, prop := range schema.Properties {
			if strings.Contains(strings.ToLower(key), "prompt_id") {
				t.Fatalf("%s InputSchema contains forbidden key 'prompt_id' in property: %s", toolName, key)
			}
			if strings.Contains(strings.ToLower(prop.Description), "prompt_id") {
				t.Fatalf("%s InputSchema description mentions 'prompt_id' in property: %s -> %s", toolName, key, prop.Description)
			}
		}
		for _, req := range schema.Required {
			if strings.EqualFold(req, "prompt_id") {
				t.Fatalf("%s InputSchema marks forbidden field 'prompt_id' as required", toolName)
			}
		}
	}

	// Run checks
	checkSchema("login", loginTool.InputSchema())
	checkSchema("profile", profileTool.InputSchema())
	checkSchema("search", searchTool.InputSchema())
}
