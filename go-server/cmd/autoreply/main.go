// autoreply MCP Server - Main entry point
package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/oyin-bo/autoreply/go-server/internal/cli"
	"github.com/oyin-bo/autoreply/go-server/internal/config"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/internal/tools"
)

func main() {
	// Load configuration
	cfg := config.LoadConfig()

	// Create tools
	profileTool := tools.NewProfileTool()
	searchTool := tools.NewSearchTool()
	loginTool, err := tools.NewLoginTool()
	if err != nil {
		log.Fatalf("Failed to create login tool: %v", err)
	}

	// Detect mode: CLI if args present, MCP server otherwise
	if len(os.Args) > 1 {
		// CLI Mode
		runCLIMode(profileTool, searchTool, loginTool)
	} else {
		// MCP Server Mode
		runMCPMode(cfg, profileTool, searchTool, loginTool)
	}
}

// runCLIMode executes the tool in CLI mode
func runCLIMode(profileTool *tools.ProfileTool, searchTool *tools.SearchTool, loginTool *tools.LoginTool) {
	// Create registry
	registry := cli.NewRegistry()

	// Register profile tool
	profileAdapter := cli.NewMCPToolAdapter(profileTool)
	profileDef := &cli.ToolDefinition{
		Name:        "profile",
		Description: "Retrieve user profile information from Bluesky",
		ArgsType:    &cli.ProfileArgs{},
		Execute:     profileAdapter.Execute,
	}
	registry.RegisterTool(profileDef)

	// Register search tool
	searchAdapter := cli.NewMCPToolAdapter(searchTool)
	searchDef := &cli.ToolDefinition{
		Name:        "search",
		Description: "Search posts within a user's repository",
		ArgsType:    &cli.SearchArgs{},
		Execute:     searchAdapter.Execute,
	}
	registry.RegisterTool(searchDef)

	// Register unified login tool with interactive prompting and subcommands
	loginInteractiveAdapter := cli.NewInteractiveLoginAdapter(loginTool)
	loginDef := &cli.ToolDefinition{
		Name:        "login",
		Description: "Authenticate accounts and manage stored credentials (supports subcommands: list, default, delete)",
		ArgsType:    &cli.LoginArgs{},
		Execute:     loginInteractiveAdapter.Execute,
	}
	registry.RegisterTool(loginDef)

	// Create and run CLI runner
	runner := cli.NewRunner(registry)
	runner.RegisterToolCommand(profileDef)
	runner.RegisterToolCommand(searchDef)
	runner.RegisterToolCommand(loginDef)

	ctx := context.Background()
	if err := runner.Run(ctx, os.Args[1:]); err != nil {
		os.Exit(1)
	}
}

// runMCPMode starts the MCP server
func runMCPMode(cfg *config.Config, profileTool *tools.ProfileTool, searchTool *tools.SearchTool, loginTool *tools.LoginTool) {
	// Create MCP server
	server, err := mcp.NewServer()
	if err != nil {
		log.Fatalf("Failed to create MCP server: %v", err)
	}

	// Register tools (logout and accounts functionality now in login subcommands)
	server.RegisterTool("profile", profileTool)
	server.RegisterTool("search", searchTool)
	server.RegisterTool("login", loginTool)

	// Set up graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Listen for interrupt signals
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)

	go func() {
		<-sigChan
		log.Println("Received shutdown signal, shutting down...")
		cancel()
	}()

	// Start the server in stdio mode
	log.Printf("Starting autoreply server with config: Cache TTL=%dh, Profile TTL=%dh",
		cfg.CacheTTLHours, cfg.ProfileTTLHours)

	if err := server.ServeStdio(ctx); err != nil {
		log.Fatalf("Server error: %v", err)
	}

	log.Println("Server shut down gracefully")
}
