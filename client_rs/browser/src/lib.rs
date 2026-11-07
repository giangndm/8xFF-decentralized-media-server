//! Browser I/O adapter for the generic, Sans-I/O media client.
//!
//! The browser crate owns Web APIs and event-loop scheduling. Session policy
//! and protocol state remain in `atm0s-media-client-core`.

mod http;
mod media;
mod peer;
mod render;
mod runtime;

pub use http::{post_protobuf, request_protobuf, request_protobuf_with_timeout, BrowserHttpClient};
pub use media::{get_user_media, BrowserStream, BrowserTrack};
pub use peer::{BrowserPeer, BrowserWebRtcPeer, DataEvent, EventSink, PeerEvent};
pub use render::{bind_and_play, unbind};
pub use runtime::BrowserRuntime;

/// Ready-to-use generic facade type for browser applications that want to
/// drive the core directly instead of using [`BrowserRuntime`].
pub type BrowserClient = atm0s_media_client_core::Atm0sClient<BrowserHttpClient, BrowserWebRtcPeer>;
