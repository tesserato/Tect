cargo run --manifest-path server/Cargo.toml -- samples/login.tect --output graph.json
Get-Content graph.json
python visualize.py