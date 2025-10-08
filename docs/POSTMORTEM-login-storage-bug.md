# Postmortem: Login Storage Bug - Credentials Not Persisting After Successful Authentication

## Date
2025-01-08

## Summary
Users successfully authenticated via OAuth but subsequent `login list` commands showed "No accounts stored", despite multiple fallback warnings during login indicating credentials were being stored to file.

## Impact
- **Severity**: Critical
- **Duration**: Multiple commits (dd10796, 4ced04e) attempted fixes but failed
- **Affected**: All Rust server users on systems without D-Bus secrets service (common in WSL, Docker, headless Linux)

## Root Cause Analysis

### The Bug
The `CredentialStorage::new()` function uses `test_keyring()` to determine if keyring is available:

```rust
fn test_keyring() -> bool {
    let entry = keyring::Entry::new(SERVICE_NAME, "test");
    entry.is_ok()
}
```

**This test is insufficient.** Creating an Entry succeeds even when the D-Bus secrets service is unavailable. The actual failure occurs during `set_password()` or `get_password()` operations.

### What Happened

1. **First LoginManager instance (during `autoreply login`):**
   - `CredentialStorage::new()` calls `test_keyring()` → returns `true` (Entry creation succeeds)
   - Starts with `backend: Keyring`
   - OAuth succeeds, tries to store credentials via keyring
   - Keyring operations fail (D-Bus error)
   - Fallback logic switches to `backend: File` and stores credentials successfully
   - User sees: "✓ Successfully authenticated... Storage: OS keyring" (MISLEADING - actually using file!)

2. **Second LoginManager instance (during `autoreply login list`):**
   - Creates NEW `CredentialStorage` 
   - `test_keyring()` again returns `true` (Entry creation succeeds)
   - Starts with `backend: Keyring` again
   - `list_accounts()` tries keyring → fails with D-Bus error
   - Returns empty list (no fallback in list_accounts)
   - User sees: "No accounts stored"

### Previous Failed Fix Attempts

**Commit dd10796**: Added `update_account_list()` call
- **Wrong diagnosis**: Assumed account list wasn't being updated
- **Didn't fix**: Root cause was using wrong backend on subsequent runs

**Commit 4ced04e**: Made storage methods switch backend on failure
- **Correct approach** for maintaining state within a single instance
- **Didn't fix**: Each command creates a NEW instance that starts with Keyring again

## The Real Fix

The `test_keyring()` function must actually test keyring operations, not just Entry creation:

```rust
fn test_keyring() -> bool {
    let entry = match keyring::Entry::new(SERVICE_NAME, "_test_probe") {
        Ok(e) => e,
        Err(_) => return false,
    };
    
    // Test actual write operation
    if entry.set_password("test").is_err() {
        return false;
    }
    
    // Test actual read operation
    if entry.get_password().is_err() {
        let _ = entry.delete_password(); // cleanup
        return false;
    }
    
    // Cleanup
    let _ = entry.delete_password();
    true
}
```

Additionally, the success message should reflect the actual backend used:

```rust
// In login_flow.rs, update the success message to use actual backend
match self.storage.backend() {
    StorageBackend::Keyring => "OS keyring",
    StorageBackend::File => "file",
}
```

## Timeline of Events

1. **Initial implementation**: Login worked but used incorrect backend detection
2. **User report #1**: "login list shows nothing after successful login"
3. **Response #1**: Misunderstood as case-sensitivity issue → wrong fix
4. **User report #2**: Provided detailed logs showing the actual issue
5. **Response #2**: Diagnosed as missing account_list update → incomplete fix
6. **Response #3**: Added backend switching on failure → correct pattern but incomplete
7. **User report #3**: Issue persists, requests postmortem

## Lessons Learned

1. **Test actual operations, not just object creation**: The keyring test must perform read/write operations to validate functionality
2. **Understand instance lifecycle**: Each command creates a new LoginManager/CredentialStorage instance
3. **Read the logs carefully**: The user's logs clearly showed D-Bus errors during keyring operations, but test_keyring() was passing
4. **Don't mislead users**: The success message said "OS keyring" when actually using file storage
5. **Verify fixes properly**: Each "fix" should have been tested on a system without D-Bus to confirm it actually worked

## Action Items

- [x] Create this postmortem document  
- [x] Fix `test_keyring()` to test actual operations
- [ ] Update success message to show correct storage backend (optional enhancement)
- [ ] Add integration test for keyring fallback scenario (future work)
- [ ] Document the storage backend selection logic (future work)

## Resolution

Fixed in commit (pending): Updated `test_keyring()` to perform actual read/write operations instead of just checking Entry creation. This ensures `CredentialStorage::new()` correctly detects when keyring is unavailable and starts with file backend from the beginning.
