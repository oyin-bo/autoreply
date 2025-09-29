// autoreply MCP Server - Main entry point
package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/oyin-bo/autoreply/go-server/internal/config"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/oyin-bo/autoreply/go-server/internal/tools"
)

func main() {
	// Load configuration
	cfg := config.LoadConfig()
	
	// Create MCP server
	server, err := mcp.NewServer()
	if err != nil {
		log.Fatalf("Failed to create MCP server: %v", err)
	}

	// Register tools
	profileTool := tools.NewProfileTool()
	server.RegisterTool("profile", profileTool)

	searchTool := tools.NewSearchTool()
	server.RegisterTool("search", searchTool)

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
