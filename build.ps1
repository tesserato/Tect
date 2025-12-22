# Build Rust Server
cd server
cargo test
cargo build

# Build VS Code Extension
cd ../editors/vscode
npm i
npm run compile

# Go back to root
cd ../..