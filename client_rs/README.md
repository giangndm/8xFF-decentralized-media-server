# Rust media client

A generic Rust SDK for the atm0s media server. Supports sessions, rooms, senders/receivers, ICE restart, room messaging and audio mixing.

## Packages

All packages use the repository's root Cargo workspace and require Rust 1.84 or newer.

| Package | Purpose |
| --- | --- |
| `core` | `Atm0sClient<H, P>`, adapter contracts and event-based state machine |
| `browser` | HTTP, WebRTC, media and rendering through Web APIs |
| `mock` | Scripted adapters and virtual-time testing |
| `browser/examples` | Rust/Sycamore example application |

HTTP and WebRTC adapters are independent generic parameters. Track, stream and render-target types belong to the peer adapter. The core has no browser or async-runtime dependency. Server Protobuf types are reused directly.

## Documentation

- [Usage](USAGE.md): construct a client, drive commands and handle media.
- [Architecture](ARCHITECTURE.md): module boundaries, signaling and adapter implementation.
- [Browser tests](tests/browser_e2e/README.md): setup, examples and standalone E2E commands.

## Checks

Run from the repository root:

```sh
bash client_rs/scripts/check.sh
bash client_rs/tests/browser_e2e/run_e2e.sh
```

The first command checks formatting, protocol fixtures, core/mock tests and WASM compilation. The second starts a standalone server and runs all eight scenarios with two browser peers each.

## Compatibility

Chromium passes all eight scenarios: basic connection/messaging, lazy join, simulcast configuration, ICE restart, track replacement, dynamic receivers, automatic mixing and manual mixing.

Firefox passes HTTP deadline tests; WebRTC echo currently times out waiting for the control channel. Safari, native adapters, SIP and cross-server failover are not validated.

Media tests check continuing decoded frames or audio samples. They do not establish selected simulcast layers, replacement-source identity, audio energy or multi-source mixing correctness.
