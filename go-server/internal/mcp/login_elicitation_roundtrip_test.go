package mcp_test

import (
    "bytes"
    "context"
    "encoding/json"
    "strings"
    "sync"
    "testing"
    "time"

    mcp "github.com/oyin-bo/autoreply/go-server/internal/mcp"
    "github.com/oyin-bo/autoreply/go-server/internal/tools"
)

// threadSafeWriter provides a concurrency-safe writer that accumulates
// JSON-RPC requests in an internal buffer for test inspection.
type threadSafeWriter struct {
    buffer *bytes.Buffer
    mu     *sync.Mutex
}

func (w *threadSafeWriter) Write(p []byte) (n int, err error) {
    w.mu.Lock()
    defer w.mu.Unlock()
    return w.buffer.Write(p)
}

// getLastLine returns the last non-empty line from the writer buffer.
func (w *threadSafeWriter) getLastLine() string {
    w.mu.Lock()
    defer w.mu.Unlock()
    lines := strings.Split(w.buffer.String(), "\n")
    // Walk backwards to find last non-empty line
    for i := len(lines) - 1; i >= 0; i-- {
        if strings.TrimSpace(lines[i]) != "" {
            return lines[i]
        }
    }
    return ""
}

// countOccurrences counts substring occurrences in a string
func countOccurrences(s, substr string) int {
    if substr == "" {
        return 0
    }
    count := 0
    idx := 0
    for {
        i := strings.Index(s[idx:], substr)
        if i == -1 {
            break
        }
        count++
        idx += i + len(substr)
    }
    return count
}

// waitForNextElicitationRequest waits until a new elicitation/create request is written
// beyond the provided seenCount and returns the parsed request, its ID, and the new count.
func waitForNextElicitationRequest(t *testing.T, w *threadSafeWriter, seenCount int, timeout time.Duration) (mcp.JSONRPCRequest, interface{}, int) {
    deadline := time.Now().Add(timeout)
    for time.Now().Before(deadline) {
        w.mu.Lock()
        bufStr := w.buffer.String()
        w.mu.Unlock()

        total := countOccurrences(bufStr, "\"elicitation/create\"")
        if total > seenCount {
            // find last line containing method
            lines := strings.Split(bufStr, "\n")
            for i := len(lines) - 1; i >= 0; i-- {
                line := strings.TrimSpace(lines[i])
                if line == "" {
                    continue
                }
                if strings.Contains(line, "\"elicitation/create\"") {
                    var req mcp.JSONRPCRequest
                    if err := json.Unmarshal([]byte(line), &req); err == nil && req.Method == "elicitation/create" {
                        if req.ID == nil {
                            t.Fatalf("elicitation/create request missing id: %s", line)
                        }
                        return req, req.ID, total
                    }
                }
            }
        }
        time.Sleep(10 * time.Millisecond)
    }
    t.Fatalf("Timed out waiting for elicitation/create request")
    return mcp.JSONRPCRequest{}, nil, seenCount
}

// respondElicitation sends a synthetic JSON-RPC response to the server for a given ID
func respondElicitation(t *testing.T, server *mcp.Server, id interface{}, action string, content map[string]interface{}) {
    // Build result matching ElicitationResponse JSON shape
    result := map[string]interface{}{
        "action":  action,
        "content": content,
    }
    // Coerce ID to int64 to match server's pendingResponses key type
    var respID interface{} = id
    if f, ok := id.(float64); ok {
        respID = int64(f)
    }
    resp := &mcp.JSONRPCResponse{JSONRPC: "2.0", ID: respID, Result: result}
    server.InjectClientResponseForTest(resp)
}

// Test login elicitation when client supports it: handle accept, then password cancel
func TestLoginElicitation_HandleAccept_PasswordCancel(t *testing.T) {
    // Wire up a server with elicitation support and a custom encoder
    var buf bytes.Buffer
    var mu sync.Mutex
    writer := &threadSafeWriter{buffer: &buf, mu: &mu}

    server, _ := mcp.NewServer()
    server.SetWriterForTest(writer)
    server.SetClientCapabilitiesForTest(&mcp.ClientCapabilities{Elicitation: &mcp.ElicitationCapability{}})

    tool, err := tools.NewLoginTool()
    if err != nil {
        t.Fatalf("NewLoginTool error: %v", err)
    }

    // Force app-password flow but with missing handle and password -> triggers 2 elicitations
    args := map[string]interface{}{"password": ""}

    // Run the call concurrently; it will block awaiting elicitation responses
    done := make(chan *mcp.ToolResult, 1)
    go func() {
        res, callErr := tool.Call(context.Background(), args, server)
        if callErr != nil {
            t.Fatalf("login.Call returned error: %v", callErr)
        }
        done <- res
    }()

    // 1) Handle elicitation
    _, id1, seen := waitForNextElicitationRequest(t, writer, 0, 2*time.Second)
    respondElicitation(t, server, id1, "accept", map[string]interface{}{"handle": "alice.bsky.social"})

    // 2) Password elicitation -> cancel
    _, id2, _ := waitForNextElicitationRequest(t, writer, seen, 2*time.Second)
    respondElicitation(t, server, id2, "cancel", nil)

    // Final result
    select {
    case result := <-done:
        if result == nil || len(result.Content) == 0 {
            t.Fatalf("Expected non-empty ToolResult")
        }
        txt := result.Content[0].Text
        if !strings.Contains(txt, "Login cancelled.") {
            t.Fatalf("Expected cancellation guidance, got: %s", txt)
        }
        if result.IsError { // cancel path is non-error informational
            t.Fatalf("Expected IsError=false on cancel path")
        }
    case <-time.After(2 * time.Second):
        t.Fatal("Timed out waiting for login result")
    }
}

// Test login elicitation: handle accept, then password decline
func TestLoginElicitation_HandleAccept_PasswordDecline(t *testing.T) {
    var buf bytes.Buffer
    var mu sync.Mutex
    writer := &threadSafeWriter{buffer: &buf, mu: &mu}

    server, _ := mcp.NewServer()
    server.SetWriterForTest(writer)
    server.SetClientCapabilitiesForTest(&mcp.ClientCapabilities{Elicitation: &mcp.ElicitationCapability{}})

    tool, err := tools.NewLoginTool()
    if err != nil {
        t.Fatalf("NewLoginTool error: %v", err)
    }

    args := map[string]interface{}{"password": ""}

    done := make(chan *mcp.ToolResult, 1)
    go func() {
        res, callErr := tool.Call(context.Background(), args, server)
        if callErr != nil {
            t.Fatalf("login.Call returned error: %v", callErr)
        }
        done <- res
    }()

    // 1) Handle elicitation -> accept
    _, id1, seen := waitForNextElicitationRequest(t, writer, 0, 2*time.Second)
    respondElicitation(t, server, id1, "accept", map[string]interface{}{"handle": "alice.bsky.social"})

    // 2) Password elicitation -> decline
    _, id2, _ := waitForNextElicitationRequest(t, writer, seen, 2*time.Second)
    respondElicitation(t, server, id2, "decline", nil)

    // Final result should be "Login declined"
    select {
    case result := <-done:
        if result == nil || len(result.Content) == 0 {
            t.Fatalf("Expected non-empty ToolResult")
        }
        txt := result.Content[0].Text
        if !strings.Contains(txt, "Login declined") {
            t.Fatalf("Expected 'Login declined', got: %s", txt)
        }
        if result.IsError {
            t.Fatalf("Expected IsError=false on decline path")
        }
    case <-time.After(2 * time.Second):
        t.Fatal("Timed out waiting for login result")
    }
}

// Test handle elicitation declined immediately
func TestLoginElicitation_HandleDecline(t *testing.T) {
    var buf bytes.Buffer
    var mu sync.Mutex
    writer := &threadSafeWriter{buffer: &buf, mu: &mu}

    server, _ := mcp.NewServer()
    server.SetWriterForTest(writer)
    server.SetClientCapabilitiesForTest(&mcp.ClientCapabilities{Elicitation: &mcp.ElicitationCapability{}})

    tool, err := tools.NewLoginTool()
    if err != nil {
        t.Fatalf("NewLoginTool error: %v", err)
    }

    args := map[string]interface{}{"password": ""}

    done := make(chan *mcp.ToolResult, 1)
    go func() {
        res, callErr := tool.Call(context.Background(), args, server)
        if callErr != nil {
            t.Fatalf("login.Call returned error: %v", callErr)
        }
        done <- res
    }()

    // 1) Handle elicitation -> decline immediately
    _, id1, _ := waitForNextElicitationRequest(t, writer, 0, 2*time.Second)
    respondElicitation(t, server, id1, "decline", nil)

    select {
    case result := <-done:
        if result == nil || len(result.Content) == 0 {
            t.Fatalf("Expected non-empty ToolResult")
        }
        txt := result.Content[0].Text
        if !strings.Contains(txt, "Login cancelled") {
            t.Fatalf("Expected 'Login cancelled', got: %s", txt)
        }
        if result.IsError {
            t.Fatalf("Expected IsError=false on cancel/decline path")
        }
    case <-time.After(2 * time.Second):
        t.Fatal("Timed out waiting for login result")
    }
}
