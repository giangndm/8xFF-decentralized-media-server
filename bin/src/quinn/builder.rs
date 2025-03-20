use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, ServerConfig, TransportConfig};
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

/// Returns default server configuration along with its certificate.
pub(super) fn configure_server(priv_key: PrivatePkcs8KeyDer<'static>, cert: CertificateDer<'static>) -> Result<ServerConfig, Box<dyn Error>> {
    let cert_chain = vec![cert];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key.into())?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok(server_config)
}

pub(super) fn configure_client(server_certs: &[CertificateDer]) -> Result<ClientConfig, Box<dyn Error>> {
    let mut config = if server_certs.is_empty() {
        let provider = rustls::crypto::CryptoProvider::get_default().unwrap();
        ClientConfig::new(Arc::new(QuicClientConfig::try_from(
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(SkipServerVerification::new(provider.clone()))
                .with_no_client_auth(),
        )?))
    } else {
        let mut certs = rustls::RootCertStore::empty();
        for cert in server_certs {
            certs.add(cert.clone())?;
        }
        ClientConfig::with_root_certificates(Arc::new(certs))?
    };

    let mut transport = TransportConfig::default();
    transport.keep_alive_interval(Some(Duration::from_secs(3)));
    config.transport_config(Arc::new(transport));
    Ok(config)
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new(provider: Arc<rustls::crypto::CryptoProvider>) -> Arc<Self> {
        Arc::new(Self(provider))
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_tls12_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }

    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
}
