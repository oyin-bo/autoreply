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
	logoutTool, err := tools.NewLogoutTool()
	if err != nil {
		log.Fatalf("Failed to create logout tool: %v", err)
	}
	accountsTool, err := tools.NewAccountsTool()
	if err != nil {
		log.Fatalf("Failed to create accounts tool: %v", err)
	}
	oauthLoginTool, err := tools.NewOAuthLoginTool()
	if err != nil {
		log.Fatalf("Failed to create OAuth login tool: %v", err)
	}
	deviceLoginTool, err := tools.NewDeviceLoginTool()
	if err != nil {
		log.Fatalf("Failed to create device login tool: %v", err)
	}

	// Detect mode: CLI if args present, MCP server otherwise
	if len(os.Args) > 1 {
		// CLI Mode
		runCLIMode(profileTool, searchTool, loginTool, logoutTool, accountsTool, oauthLoginTool, deviceLoginTool)
	} else {
		// MCP Server Mode
		runMCPMode(cfg, profileTool, searchTool, loginTool, logoutTool, accountsTool, oauthLoginTool, deviceLoginTool)
	}
}

// runCLIMode executes the tool in CLI mode
func runCLIMode(profileTool *tools.ProfileTool, searchTool *tools.SearchTool, loginTool *tools.LoginTool, logoutTool *tools.LogoutTool, accountsTool *tools.AccountsTool, oauthLoginTool *tools.OAuthLoginTool, deviceLoginTool *tools.DeviceLoginTool) {
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

	// Register login tool with interactive prompting
	loginInteractiveAdapter := cli.NewInteractiveLoginAdapter(loginTool)
	loginDef := &cli.ToolDefinition{
		Name:        "login",
		Description: "Authenticate with Bluesky using handle and app password",
		ArgsType:    &cli.LoginArgs{},
		Execute:     loginInteractiveAdapter.Execute,
	}
	registry.RegisterTool(loginDef)

	// TODO: OAuth and Device login temporarily disabled - need proper AT Protocol OAuth discovery
	// These methods require actual BlueSky OAuth endpoints, not placeholder URLs
	// See docs/12-auth-plan.md for implementation requirements
	
	// Register OAuth login tool
	// oauthLoginAdapter := cli.NewMCPToolAdapter(oauthLoginTool)
	// oauthLoginDef := &cli.ToolDefinition{
	// 	Name:        "oauth-login",
	// 	Description: "Authenticate with Bluesky using OAuth 2.0 with PKCE and DPoP",
	// 	ArgsType:    &cli.OAuthLoginArgs{},
	// 	Execute:     oauthLoginAdapter.Execute,
	// }
	// registry.RegisterTool(oauthLoginDef)

	// Register device login tool
	// deviceLoginAdapter := cli.NewMCPToolAdapter(deviceLoginTool)
	// deviceLoginDef := &cli.ToolDefinition{
	// 	Name:        "device-login",
	// 	Description: "Authenticate with Bluesky using Device Authorization Grant",
	// 	ArgsType:    &cli.DeviceLoginArgs{},
	// 	Execute:     deviceLoginAdapter.Execute,
	// }
	// registry.RegisterTool(deviceLoginDef)

	// Register logout tool
	logoutAdapter := cli.NewMCPToolAdapter(logoutTool)
	logoutDef := &cli.ToolDefinition{
		Name:        "logout",
		Description: "Remove stored credentials for a Bluesky account",
		ArgsType:    &cli.LogoutArgs{},
		Execute:     logoutAdapter.Execute,
	}
	registry.RegisterTool(logoutDef)

	// Register accounts tool
	accountsAdapter := cli.NewMCPToolAdapter(accountsTool)
	accountsDef := &cli.ToolDefinition{
		Name:        "accounts",
		Description: "List authenticated accounts and manage default account",
		ArgsType:    &cli.AccountsArgs{},
		Execute:     accountsAdapter.Execute,
	}
	registry.RegisterTool(accountsDef)

	// Create and run CLI runner
	runner := cli.NewRunner(registry)
	runner.RegisterToolCommand(profileDef)
	runner.RegisterToolCommand(searchDef)
	runner.RegisterToolCommand(loginDef)
	// runner.RegisterToolCommand(oauthLoginDef) // Disabled - needs proper OAuth endpoints
	// runner.RegisterToolCommand(deviceLoginDef) // Disabled - needs proper OAuth endpoints
	runner.RegisterToolCommand(logoutDef)
	runner.RegisterToolCommand(accountsDef)

	ctx := context.Background()
	if err := runner.Run(ctx, os.Args[1:]); err != nil {
		os.Exit(1)
	}
}

// runMCPMode starts the MCP server
func runMCPMode(cfg *config.Config, profileTool *tools.ProfileTool, searchTool *tools.SearchTool, loginTool *tools.LoginTool, logoutTool *tools.LogoutTool, accountsTool *tools.AccountsTool, oauthLoginTool *tools.OAuthLoginTool, deviceLoginTool *tools.DeviceLoginTool) {
	// Create MCP server
	server, err := mcp.NewServer()
	if err != nil {
		log.Fatalf("Failed to create MCP server: %v", err)
	}

	// Register tools
	server.RegisterTool("profile", profileTool)
	server.RegisterTool("search", searchTool)
	server.RegisterTool("login", loginTool)
	// server.RegisterTool("oauth-login", oauthLoginTool) // Disabled - needs proper OAuth endpoints
	// server.RegisterTool("device-login", deviceLoginTool) // Disabled - needs proper OAuth endpoints
	server.RegisterTool("logout", logoutTool)
	server.RegisterTool("accounts", accountsTool)

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
