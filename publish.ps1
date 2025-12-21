# Script to generate documentation, publish the crate, and tag the release
cargo package --list --allow-dirty

# exit

# --- Configuration ---
$ErrorActionPreference = "Stop" # Exit script on any error

# Get the crate version from Cargo.toml
try {
    $cargoTomlContent = Get-Content -Path ".\Cargo.toml" -Raw
    $versionLine = $cargoTomlContent | Select-String -Pattern 'version\s*=\s*"([^"]+)"'
    if (-not $versionLine) {
        Write-Error "Could not find version in Cargo.toml"
        exit 1
    }
    $crateVersion = $versionLine.Matches[0].Groups[1].Value
}
catch {
    Write-Error "Error reading Cargo.toml: $_"
    exit 1
}

$tagName = "v$($crateVersion)"
$commitMessage = "Release version $($crateVersion)"

# --- Functions ---
function Check-GitClean {
    Write-Host "Checking if Git working directory is clean..."
    $gitStatus = git status --porcelain
    if ($gitStatus) {
        Write-Error "Git working directory is not clean. Please commit or stash changes before publishing."
        Write-Host "Git status output:"
        Write-Host $gitStatus
        exit 1
    }
    Write-Host "Git working directory is clean."
}

function Check-GitTagExists {
    param (
        [string]$tag
    )
    Write-Host "Checking if tag '$tag' already exists locally..."
    $existingTag = git tag --list $tag
    if ($existingTag) {
        Write-Error "Tag '$tag' already exists locally. Please remove it or choose a different version."
        exit 1
    }
    Write-Host "Tag '$tag' does not exist locally."

    Write-Host "Checking if tag '$tag' already exists remotely..."
    # Fetch tags from remote first to ensure we have the latest
    git fetch --tags origin
    $existingRemoteTag = git ls-remote --tags origin refs/tags/$tag
    if ($existingRemoteTag) {
        Write-Error "Tag '$tag' already exists on the remote 'origin'. Please remove it or choose a different version."
        exit 1
    }
    Write-Host "Tag '$tag' does not exist on remote 'origin'."
}

# --- Main Script ---

# 0. Initial Checks
Write-Host "--- Step 0: Initial Checks ---"
Check-GitClean
Check-GitTagExists -tag $tagName
Write-Host "Initial checks passed."
Write-Host ""

# 0.5. Push current branch to remote (NEW STEP)
Write-Host "--- Step 0.5: Pushing current branch to remote ---"
$currentBranch = ""
try {
    $currentBranch = (git rev-parse --abbrev-ref HEAD).Trim()
    if (-not $currentBranch) {
        Write-Error "Could not determine current git branch."
        exit 1
    }
    if ($currentBranch -eq "HEAD") {
        Write-Error "Cannot push from a detached HEAD state. Please checkout a named branch before publishing."
        exit 1
    }
}
catch {
    Write-Error "Failed to determine current git branch: $_"
    exit 1
}

Write-Host "Current branch is '$currentBranch'."
Write-Host "Attempting to push branch '$currentBranch' to remote 'origin'..."
try {
    git push origin "$currentBranch"
    Write-Host "Branch '$currentBranch' pushed successfully to remote 'origin'."
}
catch {
    Write-Error "Failed to push branch '$currentBranch': $_"
    Write-Warning "Ensure your local branch '$currentBranch' is up-to-date with its remote counterpart and can be fast-forwarded on 'origin'."
    Write-Warning "You may need to pull/rebase changes from the remote before retrying."
    exit 1
}
Write-Host ""


# 1. Generate Documentation
Write-Host "--- Step 1: Generating Documentation ---"
Write-Host "Running 'cargo doc --no-deps --open'..."
try {
    cargo doc --no-deps --open
    Write-Host "Documentation generated successfully."
}
catch {
    Write-Error "Failed to generate documentation: $_"
    exit 1
}
Write-Host ""

# 2. Package and Verify (Dry Run Optional but Recommended)
Write-Host "--- Step 2: Packaging and Verifying ---"
Write-Host "Running 'cargo package --allow-dirty' (allowing Cargo.lock changes if any from doc/etc)..."
try {
    # --allow-dirty is used because `cargo doc` might modify Cargo.lock if it pulls new doc dependencies
    # and we don't want to force a commit for that minor change just before packaging.
    # If you prefer a stricter workflow, remove --allow-dirty and ensure Cargo.lock is committed.
    cargo package --allow-dirty
    Write-Host "Crate packaged successfully for verification."
}
catch {
    Write-Error "Failed to package the crate: $_"
    exit 1
}
Write-Host "Consider inspecting the generated .crate file in 'target/package/' before publishing."
Read-Host -Prompt "Press Enter to continue with publishing, or Ctrl+C to abort"
Write-Host ""


# 3. Publish Crate
Write-Host "--- Step 3: Publishing Crate to Crates.io ---"
Write-Host "Attempting to publish crate version '$crateVersion'..."
Write-Host "Running 'cargo publish'..."
try {
    # Using --allow-dirty here again for consistency with the packaging step.
    # `cargo publish` itself will do checks; this just avoids issues if Cargo.lock changed.
    cargo publish

    # Check if the command actually succeeded (cargo publish doesn't always throw on non-zero exit)
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo publish command failed with exit code $LASTEXITCODE."
        exit 1
    }
    Write-Host "Crate version '$crateVersion' published successfully to Crates.io!"
}
catch {
    Write-Error "Failed to publish crate: $_"
    # Attempt to provide more specific cargo error if possible
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo publish command may have failed with exit code $LASTEXITCODE."
    }
    exit 1
}
Write-Host ""

# 4. Create Git Tag
Write-Host "--- Step 4: Creating Git Tag ---"
Write-Host "Creating git tag '$tagName' with message '$commitMessage'..."
try {
    git tag -a "$tagName" -m "$commitMessage"
    Write-Host "Git tag '$tagName' created successfully locally."
}
catch {
    Write-Error "Failed to create git tag: $_"
    # Attempt to clean up if publish succeeded but tagging failed
    Write-Warning "CRATE PUBLISHED, BUT TAGGING FAILED. You may need to manually tag and push."
    exit 1
}
Write-Host ""

# 5. Push Git Tag
Write-Host "--- Step 5: Pushing Git Tag to Remote ---"
Write-Host "Pushing tag '$tagName' to remote 'origin'..."
try {
    git push origin "$tagName"
    Write-Host "Git tag '$tagName' pushed successfully to remote 'origin'."
}
catch {
    Write-Error "Failed to push git tag: $_"
    Write-Warning "CRATE PUBLISHED AND TAGGED LOCALLY, BUT PUSHING TAG FAILED."
    Write-Warning "You may need to manually run 'git push origin $tagName'."
    exit 1
}
Write-Host ""

Write-Host "--- All steps completed successfully! ---"