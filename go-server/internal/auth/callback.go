// Package auth provides authentication and credential management
package auth

import (
	"context"
	"fmt"
	"net/http"
	"net/url"
	"sync"
)

// CallbackServer handles OAuth callback requests
type CallbackServer struct {
	server      *http.Server
	resultChan  chan *CallbackResult
	mu          sync.Mutex
	isListening bool
}

// CallbackResult contains the result of the OAuth callback
type CallbackResult struct {
	Code  string
	State string
	Error string
}

// NewCallbackServer creates a new callback server
func NewCallbackServer(port int) *CallbackServer {
	return &CallbackServer{
		resultChan: make(chan *CallbackResult, 1),
		server: &http.Server{
			Addr: fmt.Sprintf(":%d", port),
		},
	}
}

// Start starts the callback server
func (s *CallbackServer) Start() error {
	s.mu.Lock()
	if s.isListening {
		s.mu.Unlock()
		return fmt.Errorf("server already listening")
	}
	s.isListening = true
	s.mu.Unlock()

	mux := http.NewServeMux()
	mux.HandleFunc("/callback", s.handleCallback)
	s.server.Handler = mux

	go func() {
		if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			// Send error to result channel
			s.resultChan <- &CallbackResult{Error: err.Error()}
		}
	}()

	return nil
}

// Stop stops the callback server
func (s *CallbackServer) Stop(ctx context.Context) error {
	s.mu.Lock()
	if !s.isListening {
		s.mu.Unlock()
		return nil
	}
	s.isListening = false
	s.mu.Unlock()

	return s.server.Shutdown(ctx)
}

// WaitForCallback waits for the OAuth callback
func (s *CallbackServer) WaitForCallback(ctx context.Context) (*CallbackResult, error) {
	select {
	case result := <-s.resultChan:
		return result, nil
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

// handleCallback handles the OAuth callback request
func (s *CallbackServer) handleCallback(w http.ResponseWriter, r *http.Request) {
	// Parse query parameters
	query := r.URL.Query()
	code := query.Get("code")
	state := query.Get("state")
	errorParam := query.Get("error")
	errorDesc := query.Get("error_description")

	result := &CallbackResult{
		Code:  code,
		State: state,
	}

	if errorParam != "" {
		if errorDesc != "" {
			result.Error = fmt.Sprintf("%s: %s", errorParam, errorDesc)
		} else {
			result.Error = errorParam
		}
	}

	// Send result
	select {
	case s.resultChan <- result:
	default:
		// Channel already has a result
	}

	// Send response to browser
	if result.Error != "" {
		w.WriteHeader(http.StatusBadRequest)
		fmt.Fprintf(w, `<!DOCTYPE html>
<html>
<head>
    <title>Authorization Failed</title>
    <style>
        body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
        .error { color: #d32f2f; }
    </style>
</head>
<body>
    <h1 class="error">Authorization Failed</h1>
    <p>%s</p>
    <p>You can close this window and return to the terminal.</p>
</body>
</html>`, result.Error)
	} else {
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, `<!DOCTYPE html>
<html>
<head>
    <title>Authorization Successful</title>
    <style>
        body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
        .success { color: #388e3c; }
    </style>
</head>
<body>
    <h1 class="success">✓ Authorization Successful</h1>
    <p>You have successfully authorized the application.</p>
    <p>You can close this window and return to the terminal.</p>
</body>
</html>`)
	}
}

// GetRedirectURI returns the redirect URI for this callback server
func (s *CallbackServer) GetRedirectURI() string {
	return fmt.Sprintf("http://localhost%s/callback", s.server.Addr)
}

// GetRedirectURIWithPort returns the redirect URI with a specific port
func GetRedirectURIWithPort(port int) string {
	return fmt.Sprintf("http://localhost:%d/callback", port)
}

// ParseCallbackURL parses a callback URL to extract code and state
func ParseCallbackURL(callbackURL string) (*CallbackResult, error) {
	u, err := url.Parse(callbackURL)
	if err != nil {
		return nil, fmt.Errorf("failed to parse callback URL: %w", err)
	}

	query := u.Query()
	result := &CallbackResult{
		Code:  query.Get("code"),
		State: query.Get("state"),
		Error: query.Get("error"),
	}

	if errorDesc := query.Get("error_description"); errorDesc != "" && result.Error != "" {
		result.Error = fmt.Sprintf("%s: %s", result.Error, errorDesc)
	}

	return result, nil
}
