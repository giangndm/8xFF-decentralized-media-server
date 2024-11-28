RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --sdn-zone-id 1 \
    --sdn-zone-node-id 4 \
    --seeds-from-url "http://localhost:4000/api/node/address" \
    connector \
        --s3-uri "http://minioadmin:minioadmin@127.0.0.1:9000/record"
