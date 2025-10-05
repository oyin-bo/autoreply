//go:build ignore
// +build ignore

// Test tool list alignment with Rust implementation
// Run with: go run test_tools_list.go

package main

import (
	"encoding/json"
	"fmt"
	"log"

	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/internal/tools"
)

func main() {
	// Create tools
	profileTool := tools.NewProfileTool()
	searchTool := tools.NewSearchTool()
	loginTool, err := tools.NewLoginTool()
	if err != nil {
		log.Fatalf("Failed to create login tool: %v", err)
	}

	// Create MCP server
	server, err := mcp.NewServer()
	if err != nil {
		log.Fatalf("Failed to create MCP server: %v", err)
	}

	// Register tools
	server.RegisterTool("profile", profileTool)
	server.RegisterTool("search", searchTool)
	server.RegisterTool("login", loginTool)

	// Get tools list
	toolsList := []map[string]interface{}{}
	for name, tool := range map[string]mcp.Tool{
		"profile": profileTool,
		"search":  searchTool,
		"login":   loginTool,
	} {
		toolsList = append(toolsList, map[string]interface{}{
			"name":        name,
			"description": tool.Description(),
			"inputSchema": tool.InputSchema(),
		})
	}

	// Print as JSON
	output, err := json.MarshalIndent(map[string]interface{}{
		"tools": toolsList,
	}, "", "  ")
	if err != nil {
		log.Fatalf("Failed to marshal tools: %v", err)
	}

	fmt.Println(string(output))

	fmt.Println("\n=== Expected tool names (matching Rust) ===")
	fmt.Println("- profile")
	fmt.Println("- search")
	fmt.Println("- login")

	fmt.Println("\n=== Login tool should have 'command' parameter ===")
	fmt.Println("Commands: list, default, delete, or omit for login")
}
