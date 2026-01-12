# Build Rust Server
Set-Location server
cargo test
cargo run -- --help

# Build VS Code Extension
Set-Location ../editors/vscode
npm i
npm run compile

# # Go back to root
# cd ../..