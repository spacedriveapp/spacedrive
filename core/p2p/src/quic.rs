use std::{
	net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
	sync::Arc,
	time::SystemTime,
};

use quinn::{ClientConfig, Endpoint, Incoming, NewConnection};
use rustls::{
	server::{ClientCertVerified, ClientCertVerifier},
	Certificate, DistinguishedNames, Error, PrivateKey,
};

use crate::{
	p2p_application, server::Server, NetworkManagerError, P2PApplication, PeerCandidate, PeerId,
};

/// The Application-Layer Protocol Negotiation (ALPN) value for QUIC.
const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

/// new_server will make a new QUIC server with a standard configuration.
pub(crate) fn new_server(
	identity: (Certificate, PrivateKey),
	p2p_application: Arc<dyn P2PApplication + Send + Sync>,
) -> Result<(Endpoint, Incoming, SocketAddr), NetworkManagerError> {
	let mut server_crypto = rustls::ServerConfig::builder()
		.with_safe_default_cipher_suites()
		.with_safe_default_kx_groups()
		.with_protocol_versions(&[&rustls::version::TLS13])
		.map_err(|err| NetworkManagerError::Crypto(err))?
		.with_client_cert_verifier(ClientCertificateVerifier::new(p2p_application))
		.with_single_cert(vec![identity.0], identity.1)
		.map_err(|err| NetworkManagerError::Crypto(err))?;
	server_crypto.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();

	let socket = UdpSocket::bind(format!("{}:0", Ipv4Addr::UNSPECIFIED.to_string()))
		.map_err(|err| NetworkManagerError::Server(err))?;
	let listen_addr = socket
		.local_addr()
		.map_err(|err| NetworkManagerError::Server(err))?;

	let (endpoint, incoming) = Endpoint::new(
		Default::default(),
		Some(quinn::ServerConfig::with_crypto(Arc::new(server_crypto))),
		socket,
	)
	.map_err(|err| NetworkManagerError::Server(err))?;

	Ok((endpoint, incoming, listen_addr))
}

/// new_client will create a new QUIC client with a standard configuration.
pub(crate) async fn new_client(
	server: Arc<Server>,
	peer: PeerCandidate,
) -> Result<NewConnection, Box<dyn std::error::Error>> {
	let mut client_crypto = rustls::ClientConfig::builder()
		.with_safe_defaults()
		.with_custom_certificate_verifier(ServerCertificateVerifier::new())
		.with_single_cert(vec![server.identity.0.clone()], server.identity.1.clone())?;
	client_crypto.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();

	// TODO: Handle connecting on address other than the first one
	Ok(server
		.endpoint
		.connect_with(
			ClientConfig::new(Arc::new(client_crypto.clone())),
			SocketAddrV4::new(peer.addresses[0], peer.port).into(),
			&"todo",
		)?
		.await?)
}

/// ServerCertificateVerifier is a custom certificate verifier that is responsible for verifying the server certificate when making a QUIC connection.
struct ServerCertificateVerifier;

impl ServerCertificateVerifier {
	fn new() -> Arc<Self> {
		Arc::new(Self)
	}
}

impl rustls::client::ServerCertVerifier for ServerCertificateVerifier {
	fn verify_server_cert(
		&self,
		_end_entity: &rustls::Certificate,
		_intermediates: &[rustls::Certificate],
		_server_name: &rustls::ServerName,
		_scts: &mut dyn Iterator<Item = &[u8]>,
		_ocsp_response: &[u8],
		_now: std::time::SystemTime,
	) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
		// TODO: Verify certificate expiry
		// TODO: Verify certificate algorithms match

		Ok(rustls::client::ServerCertVerified::assertion())
	}
}

/// ClientCertificateVerifier is a custom certificate verifier that is responsible for verifying the client certificate when making a QUIC connection.
struct ClientCertificateVerifier(Arc<dyn P2PApplication + Send + Sync>);

impl ClientCertificateVerifier {
	pub fn new(
		p2p_application: Arc<dyn P2PApplication + Send + Sync>,
	) -> Arc<dyn ClientCertVerifier> {
		Arc::new(Self(p2p_application))
	}
}

impl ClientCertVerifier for ClientCertificateVerifier {
	fn offer_client_auth(&self) -> bool {
		true
	}

	fn client_auth_root_subjects(&self) -> Option<DistinguishedNames> {
		Some(vec![])
	}

	fn verify_client_cert(
		&self,
		end_entity: &Certificate,
		_intermediates: &[Certificate],
		_now: SystemTime,
	) -> Result<ClientCertVerified, Error> {
		// TODO: Verify certificate expiry
		// TODO: Verify certificate algorithms match

		let peer_id = PeerId::from_cert(end_entity);
		if self.0.can_peer_connection(peer_id) {
			Ok(ClientCertVerified::assertion())
		} else {
			Err(Error::General("TODO".into()))
		}
	}
}
