package auth

import (
	"context"
	"fmt"
	"net/http"
	"time"
)

// OAuthCallbackResult represents the OAuth callback data
type OAuthCallbackResult struct {
	Code  string
	State string
}

// OAuthCallbackServer handles OAuth callbacks
type OAuthCallbackServer struct {
	port       int
	resultChan chan OAuthCallbackResult
	server     *http.Server
}

// NewOAuthCallbackServer creates a new OAuth callback server
func NewOAuthCallbackServer(port int) *OAuthCallbackServer {
	return &OAuthCallbackServer{
		port:       port,
		resultChan: make(chan OAuthCallbackResult, 1),
	}
}

// WaitForCallback starts the server and waits for the OAuth callback
func (s *OAuthCallbackServer) WaitForCallback(ctx context.Context) (*OAuthCallbackResult, error) {
	mux := http.NewServeMux()
	
	// Handle callback
	mux.HandleFunc("/callback", func(w http.ResponseWriter, r *http.Request) {
		s.handleCallback(w, r)
	})
	
	// Handle root as well (some OAuth providers redirect to root)
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" && r.URL.Query().Get("code") != "" {
			s.handleCallback(w, r)
		} else {
			http.NotFound(w, r)
		}
	})
	
	s.server = &http.Server{
		Addr:    fmt.Sprintf("127.0.0.1:%d", s.port),
		Handler: mux,
	}
	
	// Start server in background
	go func() {
		fmt.Printf("✓ OAuth callback server listening on http://127.0.0.1:%d\n", s.port)
		if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			fmt.Printf("⚠  Callback server error: %v\n", err)
		}
	}()
	
	// Wait for callback or context cancellation
	select {
	case result := <-s.resultChan:
		// Shutdown server
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		s.server.Shutdown(shutdownCtx)
		return &result, nil
	case <-ctx.Done():
		// Shutdown server
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		s.server.Shutdown(shutdownCtx)
		return nil, ctx.Err()
	}
}

// handleCallback handles the OAuth callback request
func (s *OAuthCallbackServer) handleCallback(w http.ResponseWriter, r *http.Request) {
	query := r.URL.Query()
	
	code := query.Get("code")
	state := query.Get("state")
	
	if code == "" || state == "" {
		http.Error(w, "Missing code or state parameter", http.StatusBadRequest)
		w.Write([]byte(`
<html><body>
<h1>❌ Authentication Failed</h1>
<p>Missing code or state parameter.</p>
</body></html>
`))
		return
	}
	
	// Send success response
	w.Header().Set("Content-Type", "text/html")
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(`
<html><body>
<h1>✓ Authentication Successful</h1>
<p>You can close this window and return to the terminal.</p>
</body></html>
`))
	
	// Send result to channel
	select {
	case s.resultChan <- OAuthCallbackResult{Code: code, State: state}:
	default:
		// Channel already has a result
	}
}
