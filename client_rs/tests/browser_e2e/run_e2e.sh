#!/usr/bin/env bash
# Build WASM, start isolated standalone + static server, then run real browser tests.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
client_repo="$PWD"
client_toolchain="${CLIENT_RS_TOOLCHAIN:-stable}"
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
export CLIENT_RS_ARTIFACTS="${CLIENT_RS_ARTIFACTS:-$client_repo/client_rs/tests/browser_e2e/artifacts}"
mkdir -p "$CLIENT_RS_ARTIFACTS"
client_server_pid=""
client_web_pid=""
cleanup() {
    if [[ -n "$client_web_pid" ]]; then kill -TERM "$client_web_pid" 2>/dev/null || true; wait "$client_web_pid" 2>/dev/null || true; fi
    if [[ -n "$client_server_pid" ]]; then kill -TERM -- "-$client_server_pid" 2>/dev/null || true; wait "$client_server_pid" 2>/dev/null || true; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'; }
wait_http() {
    local client_url="$1"
    for _ in $(seq 1 150); do
        if curl --silent --fail --max-time 1 "$client_url" >/dev/null; then return 0; fi
        sleep 0.2
    done
    echo "Timed out waiting for $client_url; see $CLIENT_RS_ARTIFACTS" >&2
    return 1
}
if [[ "${CLIENT_RS_SKIP_BUILD:-0}" != 1 ]]; then
    cargo "+$client_toolchain" build --locked -p atm0s-media-client-browser-examples --target wasm32-unknown-unknown
    wasm-bindgen --target web --out-dir client_rs/browser/examples/pkg target/wasm32-unknown-unknown/debug/atm0s_media_client_browser_examples.wasm
fi
if [[ -z "${CLIENT_RS_GATEWAY:-}" ]]; then
    client_gateway_port="$(free_port)"
    client_console_port="$(free_port)"
    export CLIENT_RS_GATEWAY="http://127.0.0.1:$client_gateway_port"
    cargo "+$client_toolchain" build --locked -p atm0s-media-server
    client_geo_db="${CLIENT_RS_GEO_DB:-$CLIENT_RS_ARTIFACTS/GeoIP2-City-Test.mmdb}"
    if [[ ! -f "$client_geo_db" ]]; then
        curl --fail --location --retry 2 'https://raw.githubusercontent.com/maxmind/MaxMind-DB/main/test-data/GeoIP2-City-Test.mmdb' -o "$client_geo_db"
    fi
    setsid target/debug/atm0s-media-server --sdn-zone-node-id 1 --workers 1 --enable-loopback-ip standalone \
        --gateway-port "$client_gateway_port" --console-port "$client_console_port" --geo-db "$client_geo_db" \
        --max-cpu 100 --max-memory 100 --max-disk 100 \
        --db-uri "sqlite://$CLIENT_RS_ARTIFACTS/connector.db?mode=rwc" \
        --record-cache "$CLIENT_RS_ARTIFACTS/record-cache" > "$CLIENT_RS_ARTIFACTS/standalone.log" 2>&1 &
    client_server_pid=$!
fi
wait_http "$CLIENT_RS_GATEWAY/webrtc/spec"
export CLIENT_RS_WEB_PORT="${CLIENT_RS_WEB_PORT:-$(free_port)}"
export CLIENT_RS_WEB_URL="${CLIENT_RS_WEB_URL:-http://127.0.0.1:$CLIENT_RS_WEB_PORT/}"
node client_rs/browser/examples/serve_examples.mjs > "$CLIENT_RS_ARTIFACTS/web.log" 2>&1 &
client_web_pid=$!
wait_http "$CLIENT_RS_WEB_URL"
node client_rs/tests/browser_e2e/e2e.mjs
