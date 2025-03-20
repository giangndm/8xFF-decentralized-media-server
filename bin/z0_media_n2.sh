RUST_LOG=info \
cargo run -- \
    --http-port 3102 \
    --enable-private-ip \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 2 \
    --seeds-from-url "http://localhost:3000/api/node/address" \
    --workers 4 \
    media --enable-token-api
