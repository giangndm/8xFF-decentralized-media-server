pub mod cluster;
pub mod endpoint;
pub mod media;
pub mod message_channel;
pub mod multi_tenancy;
pub mod protobuf;
pub mod record;
pub mod tokens;
pub mod transport;

pub const GATEWAY_RPC_PORT: u16 = 10000;
pub const CONNECTOR_RPC_PORT: u16 = 10003;
