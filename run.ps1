cargo run --manifest-path server/Cargo.toml -- samples/ --output architecture.dot
dot -Tsvg architecture.dot > architecture.svg
