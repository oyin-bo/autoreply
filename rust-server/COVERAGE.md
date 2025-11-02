# Code Coverage Setup

## Prerequisites

### Install cargo-llvm-cov

```powershell
cargo install cargo-llvm-cov
```

### Verify Installation

```powershell
cargo llvm-cov --version
```

## Running Coverage

### Quick Start

From the `rust-server` directory:

```powershell
# PowerShell
.\run-coverage.ps1

# Or manually:
cargo llvm-cov --workspace --html --output-dir target/coverage
```

### View Report

Open `target/coverage/html/index.html` in your browser.

## Coverage Options

### Generate HTML Report
```powershell
cargo llvm-cov --workspace --html
```

### Generate Text Summary
```powershell
cargo llvm-cov --workspace
```

### Generate LCOV format (for CI/CD)
```powershell
cargo llvm-cov --workspace --lcov --output-path target/coverage/lcov.info
```

### Coverage for Specific Package
```powershell
cargo llvm-cov --package autoreply --html
```

### Exclude Tests from Coverage
```powershell
cargo llvm-cov --workspace --html --ignore-filename-regex tests
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Coverage

on: [push, pull_request]

jobs:
  coverage:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install cargo-llvm-cov
        run: cargo install cargo-llvm-cov
      
      - name: Generate coverage
        run: |
          cd rust-server
          cargo llvm-cov --workspace --lcov --output-path lcov.info
      
      - name: Upload to Codecov
        uses: codecov/codecov-action@v4
        with:
          files: rust-server/lcov.info
```

## Troubleshooting

### ARM Windows Compatibility

If you encounter issues on ARM Windows:

1. **Ensure LLVM is installed**: cargo-llvm-cov requires LLVM tools
2. **Use stable toolchain**: `rustup default stable`
3. **Clean build**: `cargo clean` before running coverage

### Common Issues

#### "could not find Cargo.toml"
Make sure you're in the `rust-server` directory:
```powershell
cd rust-server
cargo llvm-cov --workspace --html
```

#### Slow coverage generation
Coverage builds are slower than regular builds. For faster iteration:
- Use `--no-cfg-coverage` flag
- Run coverage only on specific modules
- Use `--ignore-run-fail` to continue on test failures

## Coverage Goals

### Current Coverage Status

Run coverage to see current statistics. Goals:

- **Overall**: >80% line coverage
- **Core modules** (search, auth, mcp): >90%
- **Utilities**: >70%
- **Tests**: 100% (excluded from coverage)

### Focus Areas

Priority modules for testing:

1. `src/tools/` - Tool implementations
2. `src/auth/` - Authentication flows
3. `src/mcp/` - MCP protocol handling
4. `src/bluesky/` - AT Protocol integration
5. `src/car/` - CAR file parsing

## Alternative: grcov (Nightly-only)

If cargo-llvm-cov doesn't work:

```powershell
# Install nightly Rust
rustup install nightly
rustup default nightly

# Install grcov
cargo install grcov

# Set environment variables
$env:RUSTFLAGS="-Zinstrument-coverage"
$env:LLVM_PROFILE_FILE="autoreply-%p-%m.profraw"

# Run tests
cargo test

# Generate report
grcov . -s . -t html --llvm --branch --ignore-not-existing -o target/coverage/

# Clean up
Remove-Item *.profraw
```

## References

- [cargo-llvm-cov documentation](https://github.com/taiki-e/cargo-llvm-cov)
- [Rust coverage documentation](https://doc.rust-lang.org/rustc/instrument-coverage.html)
- [grcov repository](https://github.com/mozilla/grcov)
