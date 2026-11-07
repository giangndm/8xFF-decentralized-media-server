//! Sans-I/O state machine for the Atm0s SDK protocol.
//!
//! The core never performs I/O. A driver consumes [`Effect`] values and feeds
//! completions/events back through [`Client::on_input`]. This makes the same
//! state machine usable by browser, mock, and future native adapters.

use media_server_protocol::protobuf::{gateway, session};
use prost::Message;
use std::collections::{HashMap, VecDeque};

mod adapters;
mod commands;
mod input;
mod runtime;
mod wire;
pub use adapters::{Atm0sClient, HttpClient, HttpResponse, LocalFuture, WebRtcPeer};
pub use runtime::ClientRuntime;
use wire::*;

mod model;
pub use model::*;
mod client;
pub use client::Client;
