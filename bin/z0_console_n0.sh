RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 8080 \
    --sdn-port 10000 \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 0 \
    --workers 2 \
    console
