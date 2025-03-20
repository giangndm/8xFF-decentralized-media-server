use atm0s_sdn::services::async_socket::AsyncSocket;
use quinn::{Endpoint, EndpointConfig, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::{error::Error, sync::Arc};
use vsocket::VirtualUdpSocket;

mod builder;
mod vsocket;

/// create a quinn endpoint for both client and server
pub fn make_quinn_endpoint(node_id: u32, port: u16, socket: AsyncSocket, priv_key: PrivatePkcs8KeyDer<'static>, cert: CertificateDer<'static>) -> Result<Endpoint, Box<dyn Error>> {
    let server_config = builder::configure_server(priv_key, cert.clone())?;
    let client_config = builder::configure_client(&[cert])?;

    let runtime = Arc::new(TokioRuntime);
    let socket = VirtualUdpSocket::new(node_id, port, socket);

    let mut endpoint = Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), Arc::new(socket), runtime)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}
