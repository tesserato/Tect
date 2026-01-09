# Stop the script execution if any command fails
$ErrorActionPreference = 'Stop'

Write-Host "--- 1. Building Rust server for release ---"
cargo build --release

Write-Host "--- 2. Navigating to extension directory ---"
Set-Location "./extensions/vscode"

Write-Host "--- 3. Cleaning previous build artifacts ---"
if (Test-Path "./bin") { Remove-Item -Path "./bin" -Recurse -Force }
if (Test-Path "./dist") { Remove-Item -Path "./dist" -Recurse -Force }
if (Test-Path "./out") { Remove-Item -Path "./out" -Recurse -Force }
Get-ChildItem -Path . -Filter "*.vsix" | Remove-Item -Force

Write-Host "--- 4. Creating 'bin' directory ---"
New-Item -ItemType Directory -Force -Path "./bin" | Out-Null

Write-Host "--- 5. Copying binary and LICENSE ---"
# Binary: Rename to match extension.ts expectations
Copy-Item -Path "../../target/release/tect.exe" -Destination "./bin/tect-x86_64-pc-windows-msvc.exe"

# License: Smart copy to prevent vsce warning
# 1. Look for LICENSE, LICENSE.txt, LICENSE.md in root
$rootLicense = Get-ChildItem -Path "../.." -Filter "LICENSE*" | Select-Object -First 1

if ($rootLicense) {
    Write-Host "Found '$($rootLicense.Name)' in root. Copying..."
    Copy-Item -Path $rootLicense.FullName -Destination "./LICENSE"
}
else {
    Write-Warning "No LICENSE file found in project root. Creating placeholder to bypass VSCE warning."
    Set-Content -Path "./LICENSE" -Value "See project repository for license information."
}

Write-Host "--- 6. Installing npm dependencies ---"
# -u overwrites package.json with the latest versions found on the internet
# -y answers "yes" to prompts
npx.cmd npm-check-updates -u --deep

npm.cmd install

Write-Host "--- 7. Bundling Extension ---"
npm.cmd run package

Write-Host "--- 8. Packaging extension ---"
# The LICENSE file now definitely exists, so vsce won't ask for confirmation.
npx.cmd @vscode/vsce package

Write-Host "--- 9. Installing extension into VS Code ---"
$vsixFile = Get-ChildItem -Path . -Filter "*.vsix" | Select-Object -First 1

if ($vsixFile) {
    Write-Host "Found package: $($vsixFile.Name)"
    Write-Host "Installing..."
    code --install-extension $vsixFile.FullName --force
    Write-Host "Done! Reload VS Code to apply changes."
}
else {
    Write-Error "Could not find a .vsix file to install."
}