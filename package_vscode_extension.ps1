# Stop the script execution if any command fails
$ErrorActionPreference = 'Stop'

Write-Host "--- 1. Building Rust server for release ---"
cargo build --release

Write-Host "--- 2. Navigating to extension directory ---"
Set-Location "./extensions/vscode"

Write-Host "--- 3. Cleaning previous build artifacts ---"
# Check if exists before removing to avoid errors, -Recurse is needed for folders
if (Test-Path "./bin") { Remove-Item -Path "./bin" -Recurse -Force }
if (Test-Path "./dist") { Remove-Item -Path "./dist" -Recurse -Force }
if (Test-Path "./out") { Remove-Item -Path "./out" -Recurse -Force }
# Remove any existing .vsix files
Get-ChildItem -Path . -Filter "*.vsix" | Remove-Item -Force

Write-Host "--- 4. Creating 'bin' directory ---"
# -Force ensures it creates parent dirs and doesn't fail if it exists
New-Item -ItemType Directory -Force -Path "./bin" | Out-Null

Write-Host "--- 5. Copying binary (Renaming for Windows) and LICENSE ---"
# Rename tect.exe -> tect-x86_64-pc-windows-msvc.exe
# This matches the filename expected by extension.ts line 42
Copy-Item -Path "../../target/release/tect.exe" -Destination "./bin/tect-x86_64-pc-windows-msvc.exe"
Copy-Item -Path "../../LICENSE" -Destination "./LICENSE"

Write-Host "--- 6. Installing npm dependencies ---"
# using .cmd ensures it runs correctly in PowerShell on Windows
npm.cmd install

Write-Host "--- 7. Bundling Extension ---"
npm.cmd run package

Write-Host "--- 8. Packaging extension ---"
# npx usually handles the executable resolution, but npx.cmd is safer on strictly Windows envs
npx.cmd @vscode/vsce package