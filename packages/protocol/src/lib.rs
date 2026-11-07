//! Shared wire protocol. Disable default features for SDK Protobuf types only.

#[cfg(feature = "server")]
pub mod cluster;
#[cfg(feature = "server")]
pub mod connector;
#[cfg(feature = "server")]
pub mod endpoint;
#[cfg(feature = "server")]
pub mod gateway;
#[cfg(feature = "server")]
pub mod media;
#[cfg(feature = "server")]
pub mod message_channel;
#[cfg(feature = "server")]
pub mod multi_tenancy;
pub mod protobuf;
#[cfg(feature = "server")]
pub mod record;
#[cfg(feature = "server")]
pub mod rpc;
#[cfg(feature = "server")]
pub mod tokens;
#[cfg(feature = "server")]
pub mod transport;
