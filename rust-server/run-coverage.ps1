# PowerShell script to run code coverage for Rust server
# Requires: cargo-llvm-cov (install with: cargo install cargo-llvm-cov)

Write-Host "Running code coverage analysis..." -ForegroundColor Green

# Clean previous coverage data
if (Test-Path "target/coverage") {
    Remove-Item -Recurse -Force "target/coverage"
}

# Run tests with coverage
cargo llvm-cov --workspace --html --output-dir target/coverage

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nCoverage report generated successfully!" -ForegroundColor Green
    Write-Host "Open: target/coverage/html/index.html" -ForegroundColor Cyan
    
    # Optional: Open the report in default browser
    # Start-Process "target/coverage/html/index.html"
} else {
    Write-Host "`nCoverage generation failed!" -ForegroundColor Red
    exit 1
}
