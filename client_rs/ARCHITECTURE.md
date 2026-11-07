# Architecture

```text
Application
    Atm0sClient<H: HttpClient, P: WebRtcPeer>
        Client: commands → effects → completions → events
        H: HTTP requests
        P: peer connection, tracks, streams, render targets
```

## Responsibilities

- `core/src/model.rs`: commands, events, configuration and opaque media IDs.
- `core/src/client.rs`, `commands.rs`, `input.rs`: state, operation lifecycle and transitions.
- `core/src/wire.rs`: shared Protobuf conversion and RPC handling.
- `core/src/adapters.rs`: generic async facade and HTTP/peer contracts.
- `browser/src`: fetch, WebRTC callbacks, media handles, rendering and browser scheduling.
- `mock/src`: scripted HTTP/peer results and virtual-time execution.
- `browser/examples/src`: shared session/UI helpers and one Rust module per scenario.

`Client` is Sans-I/O: it emits effects and accepts completions, peer events and clock ticks. Adapters own I/O resources. Browser callbacks enqueue input; they do not mutate domain state. Adapter futures are local and need not implement `Send`.

## Signaling

HTTP bodies are raw Protobuf, without gRPC framing. Types come from `atm0s-media-server-protocol` with `default-features = false`.

| Endpoint | Purpose |
| --- | --- |
| `POST /webrtc/connect` | Initial offer, track snapshot and optional room join |
| `POST /webrtc/{conn_id}/ice-candidate` | Upload candidates when the server is not ICE-lite |
| `POST /webrtc/{conn_id}/restart-ice` | Restart offer and current track snapshot |

Connect/restart use a backend-issued Bearer WebRTC token. Token issuance stays outside the SDK.

Connection order: create peer and transceivers → create offer → HTTP connect → apply local/remote descriptions → wait for control channel. Sender/receiver preparation order must remain stable because it determines MID mapping.

The control channel is negotiated, binary, labeled `data`, with ID `0`. Room, sender, receiver, mixer and message operations use Protobuf RPCs over this channel. Responses correlate by `req_id`; sending bytes does not complete an operation. Dynamic transceivers negotiate through an SDP-update RPC.

## Lifecycle and extensions

Operation IDs track completion and deadlines. A new peer session advances the generation to reject stale callbacks. ICE restart retains the session generation and buffers candidates until new connection details arrive; obsolete ICE-credential callbacks within that session are not independently validated.

Applications own imported tracks. Detaching or disconnecting releases bindings; the application stops its captures. Browser examples own their captures and stop them during cleanup. HTTP requests own their abort controller and deadline through response-body completion or cancellation.

To add an adapter, implement `HttpClient` and/or `WebRtcPeer`. The peer supplies associated `Track`, `Stream` and `RenderTarget` types, typed registrations, effect execution and callback polling. Return failures as completions so pending operations settle. For custom scheduling, drive `Client` directly or use `ClientRuntime<D>` with an `IoDriver`.

Native adapters, automatic reconnect, buffered-amount backpressure and broader SDK parity remain extension work.
