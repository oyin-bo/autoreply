# Integration Test Strategy for Rust Server

## Overview

This document outlines a comprehensive integration testing strategy for the autoreply Rust server, focusing on end-to-end testing without over-complicating the current implementation.

## Current Status

The codebase now has comprehensive unit test coverage for all major modules:

- **CAR file processing** (bluesky/car.rs): Critical path tests including varint parsing, CID handling, and CBOR processing
- **Cache management** (cache.rs): File operations, expiration, atomic writes, and platform-specific directory handling  
- **Error handling** (error.rs): All error types, validation functions, and conversion logic
- **HTTP client** (http.rs): Proxy configuration, environment variable handling, and client creation
- **Record parsing** (bluesky/records.rs): Markdown formatting, searchable text extraction, and serialization
- **DID resolution** (bluesky/did.rs): URL construction, validation logic, and caching behavior

## Integration Testing Approach

### 1. Mock-based Integration Tests

**Recommended for current implementation:**

```rust
// Example: tests/integration/
#[tokio::test]
async fn test_end_to_end_search_flow() {
    // Mock HTTP responses for known test data
    let mock_server = MockServer::start().await;
    
    // Setup mock responses for:
    // - DID resolution
    // - CAR file download
    // - Well-known endpoints
    
    // Test full search flow
    let result = search_tool("test.handle", "rust").await;
    assert!(result.is_ok());
}
```

**Benefits:**
- No external dependencies
- Deterministic test results
- Fast execution
- Can test error scenarios

### 2. Docker-based Test Environment

**For more comprehensive testing:**

```dockerfile
# tests/docker/Dockerfile.test-env
FROM rust:1.70
RUN apt-get update && apt-get install -y mockserver
COPY mock-responses/ /app/mocks/
```

**Test scenarios:**
- Simulated Bluesky PDS responses
- Network timeout conditions
- Malformed data handling
- Cache behavior under load

### 3. Property-based Testing

**Using proptest for robust validation:**

```rust
proptest! {
    #[test]
    fn test_did_validation_properties(
        did in r"did:plc:[a-z0-9]{24}"
    ) {
        assert!(validate_account(&did).is_ok());
    }
    
    #[test] 
    fn test_cache_key_generation(
        did in r"did:(plc|web):[a-zA-Z0-9.:/-]+"
    ) {
        let path = cache_manager.get_cache_path(&did);
        assert!(path.is_ok());
        // Verify path properties
    }
}
```

### 4. Performance Integration Tests

**Memory and timing validation:**

```rust
#[tokio::test]
async fn test_concurrent_cache_access() {
    let cache_manager = CacheManager::new().unwrap();
    
    // Spawn multiple concurrent operations
    let handles: Vec<_> = (0..100).map(|i| {
        let cache = cache_manager.clone();
        tokio::spawn(async move {
            cache.store_car(&format!("did:plc:test{:020}", i), b"data", metadata).await
        })
    }).collect();
    
    // Verify all operations complete successfully
    for handle in handles {
        assert!(handle.await.is_ok());
    }
}
```

## Test Data Strategy

### 1. Synthetic Test Data

Create realistic but synthetic test data:

```rust
// tests/fixtures/
pub fn create_test_profile() -> ProfileRecord { /* ... */ }
pub fn create_test_posts(count: usize) -> Vec<PostRecord> { /* ... */ }
pub fn create_test_car_file(records: Vec<TestRecord>) -> Vec<u8> { /* ... */ }
```

### 2. Anonymized Real Data

- Use publicly available Bluesky data
- Strip personal information
- Focus on data structure validation

## Implementation Priority

### Phase 1: Essential Integration Tests (Immediate)
1. **End-to-end MCP flow testing**
   - Initialize → Tools List → Tool Call → Response
   - Error handling and validation
   
2. **Critical path integration**
   - Handle resolution → DID lookup → CAR fetch → Parse → Format
   - Cache hit/miss scenarios

### Phase 2: Robustness Testing (Short-term)
1. **Error scenario coverage**
   - Network failures
   - Malformed responses
   - Cache corruption
   
2. **Performance validation**
   - Memory usage under load
   - Cache cleanup effectiveness
   - Concurrent request handling

### Phase 3: Advanced Testing (Long-term)
1. **Chaos engineering**
   - Random failures injection
   - Resource exhaustion scenarios
   
2. **Real-world validation**
   - Integration with actual Bluesky infrastructure
   - Long-running stability tests

## Tools and Dependencies

### Recommended Crates
```toml
[dev-dependencies]
# Existing
hex = "0.4"
tempfile = "3.0"

# For integration tests
mockito = "1.0"           # HTTP mocking
proptest = "1.0"          # Property-based testing
criterion = "0.5"         # Benchmarking
tokio-test = "0.4"        # Async testing utilities
wiremock = "0.5"          # Advanced HTTP mocking
```

### CI/CD Integration

```yaml
# .github/workflows/test.yml
- name: Run integration tests
  run: |
    cargo test --test integration -- --nocapture
    cargo test --release --bench performance
```

## Monitoring and Metrics

### Test Coverage Tracking
- Use `cargo-tarpaulin` for coverage reports
- Aim for >90% line coverage on critical paths
- Track test execution time trends

### Performance Baselines
- Establish baseline metrics for:
  - DID resolution time
  - CAR parsing speed
  - Cache operation latency
  - Memory usage patterns

## Maintenance Strategy

### Test Data Updates
- Quarterly review of test fixtures
- Update mock responses to match real API changes
- Validate against latest Bluesky protocol versions

### Test Reliability
- Regular review of flaky tests
- Timeout adjustments based on CI environment
- Mock service stability monitoring

## Conclusion

This integration testing strategy balances comprehensive coverage with implementation complexity. The immediate focus should be on mock-based integration tests for the critical MCP flow, followed by property-based testing for robustness validation.

The modular approach allows for incremental implementation while maintaining the current unit test coverage that already provides strong confidence in individual component behavior.

Key success metrics:
- Zero integration test failures in CI
- Sub-100ms average response time for cached operations  
- Memory usage stability under load
- Graceful degradation during network issues

This strategy leverages the comprehensive unit test foundation already established and extends it with targeted integration scenarios that validate the system's behavior as a cohesive whole.
