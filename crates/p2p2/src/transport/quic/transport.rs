use std::{io, net::SocketAddr, sync::Arc};

use quinn::{
    ClientConfig, ConnectError, Connecting, ConnectionError, Endpoint, Incoming, NewConnection,
    ServerConfig,
};
use thiserror::Error;

use crate::{
    cert_vertifier::{ClientCertificateVerifier, ServerCertificateVerifier},
    Identity, State, Transport,
};

use super::connection::QuicConnection;

/// The Application-Layer Protocol Negotiation (ALPN) value for QUIC.
const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

pub struct QuicState {
    listen_addr: SocketAddr,
    endpoint: Endpoint,
}

/// TODO
pub struct QuicTransport {
    addr: SocketAddr,
    identity: Identity,
}

impl QuicTransport {
    /// Create a new `QuicTransport`.
    pub async fn new(identity: Identity, addr: SocketAddr) -> Self {
        Self { addr, identity }
    }
}

impl Transport for QuicTransport {
    type State = Arc<QuicState>;
    type RawConn = NewConnection;
    type ListenError = QuicTransportError;
    type EstablishError = ConnectError;
    type ListenStreamError = ConnectionError;
    type ListenStreamItem = Connecting;
    type ListenStream = Incoming;
    type Connection = QuicConnection;

    fn listen(
        &mut self,
        state: Arc<State>,
    ) -> Result<(Self::ListenStream, Self::State), Self::ListenError> {
        let (cert, key) = self.identity.clone().into_rustls();
        let mut cfg = rustls::ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(ClientCertificateVerifier::dangerously_new(state))
            .with_single_cert(vec![cert.clone()], key.clone())?;
        cfg.max_early_data_size = u32::MAX;
        cfg.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();

        let (mut endpoint, incoming) =
            Endpoint::server(ServerConfig::with_crypto(Arc::new(cfg)), self.addr.clone())?;

        let mut cfg = rustls::ClientConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            // .with_root_certificates(root_store) // TODO: Do this
            .with_custom_certificate_verifier(ServerCertificateVerifier::dangerously_new()) // TODO: Remove this and use chain instead
            .with_single_cert(vec![cert], key)?;
        cfg.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();
        endpoint.set_default_client_config(ClientConfig::new(Arc::new(cfg)));

        Ok((
            incoming,
            Arc::new(QuicState {
                listen_addr: endpoint.local_addr()?,
                endpoint,
            }),
        ))
    }

    fn listen_addr(&self, state: Self::State) -> SocketAddr {
        state.listen_addr
    }

    fn establish(
        &self,
        state: Self::State,
        addr: SocketAddr,
    ) -> Result<Self::ListenStreamItem, Self::EstablishError> {
        state.endpoint.connect(addr, "p2pog")
    }

    fn accept(&self, state: Self::State, conn: Self::RawConn) -> Self::Connection {
        QuicConnection::new(conn)
    }
}

#[derive(Error, Debug)]
pub enum QuicTransportError {
    #[error("rustls transport error: {0}")]
    RustTlsError(#[from] rustls::Error),
    #[error("io transport error: {0}")]
    IoError(#[from] io::Error),
}
