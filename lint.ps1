cd server

Write-Host "--- 1. Formatting Code ---" -ForegroundColor Cyan
cargo fmt
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- 2. Applying Automatic Fixes ---" -ForegroundColor Cyan
cargo fix --allow-dirty --allow-staged --examples
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- 3. Running Clippy (Linting) ---" -ForegroundColor Cyan
# -D warnings makes warnings fail the build
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- 4. Checking Compilation ---" -ForegroundColor Cyan
cargo check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- 5. Running Tests ---" -ForegroundColor Cyan
cargo test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "--- All Checks Passed! ---" -ForegroundColor Green