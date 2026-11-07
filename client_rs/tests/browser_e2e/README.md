# Browser examples and E2E

The Rust/Sycamore application lives in `browser/examples`. Each scenario has its own Rust file. Node serves static assets and runs Playwright; the SDK runs as Rust/WASM.

## Setup

Requires server build dependencies, `protoc`, Node.js 20+, Python 3 and curl. Run commands from the repository root:

```sh
rustup target add --toolchain stable wasm32-unknown-unknown
cargo +stable install wasm-bindgen-cli --version 0.2.123 --locked
npm ci --prefix client_rs/tests/browser_e2e
(cd client_rs/tests/browser_e2e && npx playwright install --with-deps chromium firefox)
```

## Run

```sh
bash client_rs/tests/browser_e2e/run_e2e.sh
```

This builds WASM and standalone, chooses free ports, starts both services and runs all eight scenarios with two independent peers. It downloads a GeoIP test fixture if needed and stops its services on exit.

To use an existing gateway:

```sh
CLIENT_RS_GATEWAY=http://127.0.0.1:13100 \
  bash client_rs/tests/browser_e2e/run_e2e.sh
```

The runner requests a scoped token for each peer. `CLIENT_RS_APP_SECRET` defaults to `insecure` for local development and stays in the test process. An external gateway must expose the compatible token issuer.

## Manual examples

After an E2E build, serve the generated application:

```sh
node client_rs/browser/examples/serve_examples.mjs
```

Open `http://127.0.0.1:4173`, select a scenario, enter your gateway, room, peer and token, then connect. Use two tabs with different peer identities and matching tokens in the same room. Keep the standalone server running separately. Capture requires localhost or HTTPS.

## Options

| Variable | Purpose |
| --- | --- |
| `CLIENT_RS_SCENARIOS=echo_fast,echo_lazy` | Select scenarios; default: all eight |
| `CLIENT_RS_BROWSER=firefox` | Select Firefox; default: Chromium |
| `CLIENT_RS_BROWSER_EXECUTABLE` | Use an installed browser executable |
| `CLIENT_RS_GATEWAY` | Skip managed standalone startup |
| `CLIENT_RS_APP_SECRET` | Test token-issuer credential |
| `CLIENT_RS_E2E_TIMEOUT=60000` | Per-assertion timeout, milliseconds |
| `CLIENT_RS_KEEP_GOING=1` | Continue after a failed scenario |
| `CLIENT_RS_SKIP_BUILD=1` | Reuse generated WASM |
| `CLIENT_RS_TOOLCHAIN` | Rust toolchain; default: `stable` |
| `CLIENT_RS_GEO_DB` | Use an existing GeoIP database |
| `CLIENT_RS_WEB_PORT` | Override the example server port |
| `CLIENT_RS_ARTIFACTS` | Override the log/report directory |

## Results and compatibility

Reports, event traces and failure screenshots are written to `artifacts/<browser>/`; server logs are in `artifacts/`. Generated artifacts are ignored by Git.

A pass requires control-channel readiness, acknowledged operations, advancing decoded video or mixer audio samples, no page exceptions and completed cleanup. Chromium passes all eight scenarios. Firefox passes the HTTP probe, but WebRTC echo currently times out waiting for its control channel.

## HTTP deadline probe

```sh
node client_rs/tests/browser_e2e/http_deadline.mjs
CLIENT_RS_BROWSER=firefox node client_rs/tests/browser_e2e/http_deadline.mjs
```

Checks stalled headers/body, success, network failure and canceled futures. Every case must release its timer.
