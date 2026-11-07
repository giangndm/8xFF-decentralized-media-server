# Usage

## Construct a client

`Atm0sClient<H, P>` accepts independent HTTP and WebRTC adapters:

```rust
use atm0s_media_client_core::{Atm0sClient, Client, ClientConfig, ClientError};
use atm0s_media_client_mock::{MockHttp, MockPeer};

fn main() -> Result<(), ClientError> {
    let mut client = Atm0sClient::new(
        Client::new(ClientConfig::default()),
        MockHttp::default(),
        MockPeer::default(),
    );
    client.register_track("microphone", "mock-track".to_owned())?;
    client.register_stream("local", "mock-stream".to_owned())?;
    client.register_render_target("preview", "mock-element".to_owned())?;
    Ok(())
}
```

For browsers, use `BrowserHttpClient` and `BrowserWebRtcPeer`, or their `BrowserClient` alias. Registered handles are `BrowserTrack`, `BrowserStream` and `web_sys::HtmlMediaElement`. `BrowserRuntime` provides browser scheduling and async command helpers; see the [example session](browser/examples/src/session.rs).

## Drive commands

1. Register media handles and declare senders/receivers.
2. Submit `command(now, command)` using monotonic milliseconds.
3. Call `pump().await` and drain `poll_event()`.
4. Match `ClientEvent::OperationCompleted` by operation ID.
5. Feed `Input::Tick` with updated time to expire pending operations.

Peer callbacks arrive through `WebRtcPeer::poll_input()` or explicit `on_input()`. Retain unrelated events while waiting for an operation. Connection, room membership and received-media readiness are separate events.

Use `Command::Connect { offer: None, .. }` for SDK-managed peer preparation, offer creation and signaling. `RestartIce { offer: None }` also manages negotiation. With `Some(sdp)`, the caller must prepare the matching peer and offer.

Supply the gateway URL and a backend-issued token. Keep application secrets on the backend. Browser capture requires localhost or HTTPS.

## Media ownership

The peer adapter owns registered handles and exposes received streams. Render through browser helpers or bind the streams in your application. Stop application-owned camera/microphone tracks when finished; disconnect does not stop borrowed captures.

For deterministic tests, script mock HTTP responses and peer events with virtual timestamps. For custom event loops, use the lower-level `Client`, `Input` and `Output` API.

See [architecture](ARCHITECTURE.md) for adapter implementation and [browser tests](tests/browser_e2e/README.md) for runnable examples.
