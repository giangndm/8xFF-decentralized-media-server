#!/usr/bin/env bash
# Check client crates without compiling server-only dependencies for WASM.
set -euo pipefail
cd "$(dirname "$0")/../.."
client_toolchain="${CLIENT_RS_TOOLCHAIN:-stable}"
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
export CARGO_PROFILE_TEST_DEBUG="${CARGO_PROFILE_TEST_DEBUG:-0}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
client_packages=(atm0s-media-client-core atm0s-media-client-mock atm0s-media-client-browser atm0s-media-client-browser-examples)
client_fmt_args=()
for client_package in "${client_packages[@]}"; do
    client_fmt_args+=(-p "$client_package")
done
cargo "+$client_toolchain" fmt "${client_fmt_args[@]}" -- --check
cargo "+$client_toolchain" test --locked -p atm0s-media-server-protocol --no-default-features
cargo "+$client_toolchain" test --locked -p atm0s-media-client-core -p atm0s-media-client-mock
cargo "+$client_toolchain" check --locked -p atm0s-media-client-browser -p atm0s-media-client-browser-examples --target wasm32-unknown-unknown
