// main.go - Application entry point for Go BlueSky MCP Server
//
// Model Context Protocol server for Bluesky profile and post search functionality.
// Implements two MCP tools:
// - `profile(account)` - Retrieve user profile information
// - `search(account, query)` - Search posts within a user's repository

package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/oyin-bo/autoreply/internal/mcp"
)

func main() {
	// Set up context for graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Set up signal handling for graceful shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		log.Println("Received shutdown signal, gracefully shutting down...")
		cancel()
	}()

	log.Println("Starting BlueSky MCP Server")

	// Create and run the MCP server
	server := mcp.NewServer()

	if err := server.RunStdio(ctx); err != nil {
		log.Fatalf("Server failed: %v", err)
	}

	log.Println("BlueSky MCP Server stopped")
}