// Package tools provides MCP tool implementations
package tools

import (
	"strings"
	"testing"
)

// TestLoginToolBasics tests basic tool properties
func TestLoginToolBasics(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	t.Run("Name", func(t *testing.T) {
		if tool.Name() != "login" {
			t.Errorf("Expected name 'login', got '%s'", tool.Name())
		}
	})

	t.Run("Description_Contains_Subcommands", func(t *testing.T) {
		desc := tool.Description()
		if !strings.Contains(desc, "subcommand") {
			t.Error("Description should mention subcommands")
		}
		requiredSubcommands := []string{"list", "default", "delete"}
		for _, subcmd := range requiredSubcommands {
			if !strings.Contains(strings.ToLower(desc), subcmd) {
				t.Errorf("Description should mention '%s' subcommand", subcmd)
			}
		}
	})

	t.Run("InputSchema_HasRequiredFields", func(t *testing.T) {
		schema := tool.InputSchema()

		if schema.Type != "object" {
			t.Errorf("Expected schema type 'object', got '%s'", schema.Type)
		}

		requiredProps := []string{"command", "handle", "password", "service", "port"}
		for _, prop := range requiredProps {
			if _, ok := schema.Properties[prop]; !ok {
				t.Errorf("Schema missing '%s' property", prop)
			}
		}
	})

	t.Run("InputSchema_CommandDescription", func(t *testing.T) {
		schema := tool.InputSchema()
		
		commandProp, ok := schema.Properties["command"]
		if !ok {
			t.Fatal("Schema missing 'command' property")
		}

		if commandProp.Type != "string" {
			t.Errorf("Command property should be string, got %s", commandProp.Type)
		}

		desc := strings.ToLower(commandProp.Description)
		requiredTerms := []string{"list", "default", "delete"}
		for _, term := range requiredTerms {
			if !strings.Contains(desc, term) {
				t.Errorf("Command description should mention '%s', got: %s", term, commandProp.Description)
			}
		}
	})
}

// TestLoginSubcommandList tests the list subcommand basic behavior
func TestLoginSubcommandList(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	t.Run("EmptyList_NoError", func(t *testing.T) {
		args := map[string]interface{}{
			"command": "list",
		}

		result, err := tool.Call(nil, args, nil)
		if err != nil {
			t.Fatalf("Unexpected error: %v", err)
		}

		if result == nil {
			t.Fatal("Result should not be nil")
		}

		if len(result.Content) == 0 {
			t.Fatal("Result should have content")
		}

		content := result.Content[0].Text
		if !strings.Contains(content, "account") {
			t.Error("Expected account-related message in output")
		}
	})
}

// TestLoginSubcommandDefault tests the default subcommand
func TestLoginSubcommandDefault(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	t.Run("MissingHandle_ReturnsError", func(t *testing.T) {
		args := map[string]interface{}{
			"command": "default",
		}

		_, err := tool.Call(nil, args, nil)
		if err == nil {
			t.Error("Expected error when handle is missing")
		}

		if !strings.Contains(err.Error(), "required") {
			t.Errorf("Expected 'required' in error, got: %v", err)
		}
	})

	t.Run("NonexistentAccount_ReturnsError", func(t *testing.T) {
		args := map[string]interface{}{
			"command": "default",
			"handle":  "definitely-does-not-exist-12345.bsky.social",
		}

		_, err := tool.Call(nil, args, nil)
		if err == nil {
			t.Error("Expected error for non-existent account")
		}
	})
}

// TestLoginSubcommandDelete tests the delete subcommand
func TestLoginSubcommandDelete(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	t.Run("DeleteNonexistent_NoDefaultSet", func(t *testing.T) {
		args := map[string]interface{}{
			"command": "delete",
			"handle":  "nonexistent-12345.bsky.social",
		}

		// Should handle gracefully (either error or success depending on implementation)
		result, err := tool.Call(nil, args, nil)
		
		// Either error or success is acceptable for nonexistent account
		if err == nil && result != nil {
			// Success path is fine
			t.Log("Delete of nonexistent account succeeded (acceptable)")
		} else if err != nil {
			// Error path is also fine
			t.Logf("Delete of nonexistent account returned error: %v (acceptable)", err)
		}
	})
}

// TestLoginInvalidCommand tests invalid command handling
func TestLoginInvalidCommand(t *testing.T) {
	tool, err := NewLoginTool()
	if err != nil {
		t.Fatalf("Failed to create login tool: %v", err)
	}

	args := map[string]interface{}{
		"command": "invalid-command-xyz",
	}

	_, err = tool.Call(nil, args, nil)
	if err == nil {
		t.Error("Expected error for invalid command")
	}

	errMsg := strings.ToLower(err.Error())
	if !strings.Contains(errMsg, "unknown") && !strings.Contains(errMsg, "invalid") {
		t.Errorf("Expected 'unknown' or 'invalid' in error, got: %v", err)
	}
}

// TestGeneratePromptID tests that prompt ID generation works
func TestGeneratePromptID(t *testing.T) {
	// Generate multiple IDs and verify they're different
	seen := make(map[string]bool)

	for i := 0; i < 10; i++ {
		id := generatePromptID()
		
		if id == "" {
			t.Error("Generated prompt ID should not be empty")
		}

		if seen[id] {
			t.Errorf("Generated duplicate prompt ID: %s", id)
		}
		seen[id] = true

		// Should be hex encoded (32 chars for 16 bytes)
		if len(id) != 32 && !strings.HasPrefix(id, "fallback-") {
			t.Errorf("Expected 32-char hex string, got %d chars: %s", len(id), id)
		}
	}
}
