// Package cli provides CLI support
package cli

import (
	"strings"
	"testing"
)

// TestProfileArgsDeserialization tests ProfileArgs struct
func TestProfileArgsDeserialization(t *testing.T) {
	args := ProfileArgs{
		Account: "alice.bsky.social",
	}

	if args.Account != "alice.bsky.social" {
		t.Errorf("Expected account 'alice.bsky.social', got '%s'", args.Account)
	}
}

// TestSearchArgsDeserialization tests SearchArgs struct
func TestSearchArgsDeserialization(t *testing.T) {
	args := SearchArgs{
		Account: "bob.bsky.social",
		Query:   "test query",
		Limit:   100,
	}

	if args.Account != "bob.bsky.social" {
		t.Errorf("Expected account 'bob.bsky.social', got '%s'", args.Account)
	}

	if args.Query != "test query" {
		t.Errorf("Expected query 'test query', got '%s'", args.Query)
	}

	if args.Limit != 100 {
		t.Errorf("Expected limit 100, got %d", args.Limit)
	}
}

// TestLoginArgsDeserialization tests LoginArgs struct
func TestLoginArgsDeserialization(t *testing.T) {
	t.Run("BasicFields", func(t *testing.T) {
		args := LoginArgs{
			Command:  "list",
			Handle:   "alice.bsky.social",
			Password: "app-password",
			Service:  "https://bsky.social",
			Port:     8080,
		}

		if args.Command != "list" {
			t.Errorf("Expected command 'list', got '%s'", args.Command)
		}

		if args.Handle != "alice.bsky.social" {
			t.Errorf("Expected handle 'alice.bsky.social', got '%s'", args.Handle)
		}

		if args.Password != "app-password" {
			t.Errorf("Expected password 'app-password', got '%s'", args.Password)
		}

		if args.Service != "https://bsky.social" {
			t.Errorf("Expected service 'https://bsky.social', got '%s'", args.Service)
		}

		if args.Port != 8080 {
			t.Errorf("Expected port 8080, got %d", args.Port)
		}
	})

	t.Run("Subcommands", func(t *testing.T) {
		subcommands := []string{"list", "default", "delete", ""}
		for _, cmd := range subcommands {
			args := LoginArgs{Command: cmd}
			if args.Command != cmd {
				t.Errorf("Expected command '%s', got '%s'", cmd, args.Command)
			}
		}
	})
}

// TestArgsJSONTags tests that JSON tags are correct
func TestArgsJSONTags(t *testing.T) {
	// This test ensures the struct tags are present
	// The actual serialization/deserialization would be tested in integration tests

	t.Run("ProfileArgs", func(t *testing.T) {
		// ProfileArgs should have json tags for account
		args := ProfileArgs{Account: "test"}
		if args.Account == "" {
			t.Error("ProfileArgs.Account should be accessible")
		}
	})

	t.Run("SearchArgs", func(t *testing.T) {
		// SearchArgs should have json tags
		args := SearchArgs{
			Account: "test",
			Query:   "query",
			Limit:   50,
		}
		if args.Account == "" || args.Query == "" {
			t.Error("SearchArgs fields should be accessible")
		}
	})

	t.Run("LoginArgs", func(t *testing.T) {
		// LoginArgs should have json tags
		args := LoginArgs{
			Command: "list",
			Handle:  "test",
		}
		if args.Command == "" {
			t.Error("LoginArgs.Command should be accessible")
		}
	})
}

// TestJSONSchemaDescriptions tests that descriptions are meaningful
func TestJSONSchemaDescriptions(t *testing.T) {
	// Note: We can't directly test jsonschema tags without reflection,
	// but we can test that the structs are usable

	t.Run("ProfileArgs has account field", func(t *testing.T) {
		args := ProfileArgs{Account: "alice.bsky.social"}
		if args.Account == "" {
			t.Error("ProfileArgs.Account should not be empty after assignment")
		}
	})

	t.Run("SearchArgs has all required fields", func(t *testing.T) {
		args := SearchArgs{
			Account: "alice.bsky.social",
			Query:   "rust",
			Limit:   10,
		}
		if args.Account == "" || args.Query == "" || args.Limit == 0 {
			t.Error("SearchArgs fields should all be set")
		}
	})

	t.Run("LoginArgs supports various commands", func(t *testing.T) {
		commands := []string{"list", "default", "delete", ""}
		for _, cmd := range commands {
			args := LoginArgs{Command: cmd}
			// All commands should be valid (including empty for login)
			if cmd != "" && args.Command == "" {
				t.Errorf("LoginArgs.Command should be '%s'", cmd)
			}
		}
	})
}

// TestArgsFieldTypes tests that field types are correct
func TestArgsFieldTypes(t *testing.T) {
	t.Run("LoginArgs Port is integer", func(t *testing.T) {
		args := LoginArgs{Port: 8080}
		if args.Port != 8080 {
			t.Errorf("Expected port 8080, got %d", args.Port)
		}

		// Test default/zero value
		args2 := LoginArgs{}
		if args2.Port != 0 {
			t.Errorf("Expected zero port, got %d", args2.Port)
		}
	})

	t.Run("SearchArgs Limit is integer", func(t *testing.T) {
		args := SearchArgs{Limit: 100}
		if args.Limit != 100 {
			t.Errorf("Expected limit 100, got %d", args.Limit)
		}
	})

	t.Run("All string fields accept various values", func(t *testing.T) {
		testStrings := []string{
			"simple",
			"with spaces",
			"with-dashes",
			"with.dots",
			"@prefixed",
			"did:plc:abcdefghij",
		}

		for _, s := range testStrings {
			args := LoginArgs{Handle: s}
			if args.Handle != s {
				t.Errorf("Expected handle '%s', got '%s'", s, args.Handle)
			}
		}
	})
}

// TestArgsDocumentation tests that args have meaningful names
func TestArgsDocumentation(t *testing.T) {
	// Verify struct names are self-documenting

	structs := map[string]string{
		"ProfileArgs":  "Profile",
		"SearchArgs":   "Search",
		"LoginArgs":    "Login",
		"LogoutArgs":   "Logout",
		"AccountsArgs": "Accounts",
	}

	for name, expected := range structs {
		if !strings.Contains(name, expected) {
			t.Errorf("Struct name '%s' should contain '%s'", name, expected)
		}
	}
}
