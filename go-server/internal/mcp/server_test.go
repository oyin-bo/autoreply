package mcp

import (
	"bytes"
	"context"
	"encoding/json"
	"strings"
	"sync"
	"testing"
	"time"
)

// TestServerBidirectionalCommunication tests that server can send requests and receive responses
func TestServerBidirectionalCommunication(t *testing.T) {
	// Create a pipe to simulate stdin/stdout
	var outputBuffer bytes.Buffer

	server := &Server{
		tools:            make(map[string]Tool),
		encoder:          json.NewEncoder(&outputBuffer),
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
		clientCapability: &ClientCapabilities{
			Elicitation: &ElicitationCapability{},
		},
	}

	// Simulate sending an elicitation request
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
		defer cancel()

		schema := map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"test": map[string]interface{}{
					"type": "string",
				},
			},
		}

		// This should send a request and wait for response
		_, err := server.RequestElicitation(ctx, "Test message", schema)

		// We expect a timeout since we're not sending a response
		if err == nil {
			t.Error("Expected timeout error")
		}
	}()

	// Give goroutine time to send request
	time.Sleep(100 * time.Millisecond)

	// Verify request was written to output
	output := outputBuffer.String()
	if !strings.Contains(output, "elicitation/create") {
		t.Errorf("Expected elicitation/create in output, got: %s", output)
	}

	if !strings.Contains(output, "Test message") {
		t.Errorf("Expected message in output, got: %s", output)
	}
}

// TestSupportsElicitation tests capability checking
func TestSupportsElicitation(t *testing.T) {
	tests := []struct {
		name       string
		capability *ClientCapabilities
		expected   bool
	}{
		{
			name: "with elicitation support",
			capability: &ClientCapabilities{
				Elicitation: &ElicitationCapability{},
			},
			expected: true,
		},
		{
			name: "without elicitation support",
			capability: &ClientCapabilities{
				Elicitation: nil,
			},
			expected: false,
		},
		{
			name:       "nil capabilities",
			capability: nil,
			expected:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			server := &Server{
				clientCapability: tt.capability,
			}

			result := server.SupportsElicitation()
			if result != tt.expected {
				t.Errorf("Expected %v, got %v", tt.expected, result)
			}
		})
	}
}

// TestGetClientName tests client name retrieval
func TestGetClientName(t *testing.T) {
	tests := []struct {
		name       string
		clientInfo *ClientInfo
		expected   string
	}{
		{
			name: "with client name",
			clientInfo: &ClientInfo{
				Name: "Test Client",
			},
			expected: "Test Client",
		},
		{
			name:       "without client info",
			clientInfo: nil,
			expected:   "Unknown Client",
		},
		{
			name: "empty client name",
			clientInfo: &ClientInfo{
				Name: "",
			},
			expected: "Unknown Client",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			server := &Server{
				clientInfo: tt.clientInfo,
			}

			result := server.GetClientName()
			if result != tt.expected {
				t.Errorf("Expected %v, got %v", tt.expected, result)
			}
		})
	}
}

// TestRequestElicitation_NoCapability tests error when client doesn't support elicitation
func TestRequestElicitation_NoCapability(t *testing.T) {
	var outputBuffer bytes.Buffer

	server := &Server{
		tools:            make(map[string]Tool),
		encoder:          json.NewEncoder(&outputBuffer),
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
		clientCapability: nil, // No capability
	}

	ctx := context.Background()
	_, err := server.RequestElicitation(ctx, "Enter handle", map[string]interface{}{})

	if err == nil {
		t.Fatal("Expected error when client doesn't support elicitation")
	}

	expectedMsg := "client does not support elicitation"
	if err.Error() != expectedMsg {
		t.Errorf("Expected error '%s', got '%s'", expectedMsg, err.Error())
	}

	// Verify no request was sent
	output := outputBuffer.String()
	if output != "" {
		t.Errorf("Expected no output when client doesn't support elicitation, got: %s", output)
	}
}

// TestRequestElicitation_NoEncoder tests error when encoder not initialized
func TestRequestElicitation_NoEncoder(t *testing.T) {
	server := &Server{
		tools:            make(map[string]Tool),
		encoder:          nil, // No encoder
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
		clientCapability: &ClientCapabilities{
			Elicitation: &ElicitationCapability{},
		},
	}

	ctx := context.Background()
	_, err := server.RequestElicitation(ctx, "Enter handle", map[string]interface{}{})

	if err == nil {
		t.Fatal("Expected error when encoder not initialized")
	}

	expectedMsg := "server not initialized with stdio transport"
	if err.Error() != expectedMsg {
		t.Errorf("Expected error '%s', got '%s'", expectedMsg, err.Error())
	}
}

// TestHandleClientResponse tests response correlation
func TestHandleClientResponse(t *testing.T) {
	server := &Server{
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
	}

	// Create a pending response channel
	requestID := int64(42)
	responseChan := make(chan *JSONRPCResponse, 1)
	server.pendingResponses[requestID] = responseChan

	// Send a response
	response := &JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      requestID,
		Result:  "test result",
	}

	server.handleClientResponse(response)

	// Verify response was delivered
	select {
	case received := <-responseChan:
		if received.ID != requestID {
			t.Errorf("Expected ID %v, got %v", requestID, received.ID)
		}
		if received.Result.(string) != "test result" {
			t.Errorf("Expected result 'test result', got %v", received.Result)
		}
	case <-time.After(100 * time.Millisecond):
		t.Error("Response not delivered to channel")
	}
}

// TestHandleClientResponse_UnknownID tests handling response for unknown request
func TestHandleClientResponse_UnknownID(t *testing.T) {
	server := &Server{
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
	}

	// Send response for non-existent request (should not panic)
	response := &JSONRPCResponse{
		JSONRPC: "2.0",
		ID:      int64(999),
		Result:  "test result",
	}

	// Should not panic or block
	server.handleClientResponse(response)
}

// TestConcurrentRequestIDGeneration tests that request IDs are unique
func TestConcurrentRequestIDGeneration(t *testing.T) {
	var outputBuffer bytes.Buffer
	var bufferMu sync.Mutex

	// Thread-safe writer
	safeWriter := &threadSafeWriter{
		buffer: &outputBuffer,
		mu:     &bufferMu,
	}

	server := &Server{
		tools:            make(map[string]Tool),
		encoder:          json.NewEncoder(safeWriter),
		pendingResponses: make(map[interface{}]chan *JSONRPCResponse),
		clientCapability: &ClientCapabilities{
			Elicitation: &ElicitationCapability{},
		},
	}

	// Send multiple concurrent requests
	const numRequests = 20
	var wg sync.WaitGroup

	for i := 0; i < numRequests; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
			defer cancel()

			// We expect timeout since we're not responding
			_, _ = server.RequestElicitation(ctx, "Test", map[string]interface{}{})
		}()
	}

	wg.Wait()

	// Parse all requests from output and verify unique IDs
	bufferMu.Lock()
	output := outputBuffer.String()
	bufferMu.Unlock()

	requests := strings.Split(output, "\n")
	seenIDs := make(map[interface{}]bool)

	for _, reqStr := range requests {
		if reqStr == "" {
			continue
		}

		var req JSONRPCRequest
		if err := json.Unmarshal([]byte(reqStr), &req); err == nil {
			if req.ID != nil {
				if seenIDs[req.ID] {
					t.Errorf("Duplicate request ID found: %v", req.ID)
				}
				seenIDs[req.ID] = true
			}
		}
	}

	if len(seenIDs) != numRequests {
		t.Errorf("Expected %d unique request IDs, got %d", numRequests, len(seenIDs))
	}
}

// Thread-safe writer for concurrent tests
type threadSafeWriter struct {
	buffer *bytes.Buffer
	mu     *sync.Mutex
}

func (w *threadSafeWriter) Write(p []byte) (n int, err error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.buffer.Write(p)
}
