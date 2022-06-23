use std::{sync::Arc, time::SystemTime};

use rustls::{
	client::{ServerCertVerified, ServerCertVerifier},
	server::{ClientCertVerified, ClientCertVerifier},
	Certificate, DistinguishedNames, Error, ServerName,
};

use crate::PeerId;

/// The Application-Layer Protocol Negotiation (ALPN) value for QUIC.
const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

/// server_config will return a rustls::ServerConfig for a QUIC server. Ensures this matches the client config below!
pub fn server_config(
	cert_chain: Vec<rustls::Certificate>,
	key: rustls::PrivateKey,
) -> Result<rustls::ServerConfig, Error> {
	let mut cfg = rustls::ServerConfig::builder()
		.with_safe_default_cipher_suites()
		.with_safe_default_kx_groups()
		.with_protocol_versions(&[&rustls::version::TLS13])?
		.with_client_cert_verifier(AllowAllClientCertificateVerifier::dangerously_new())
		.with_single_cert(cert_chain, key)?;
	cfg.max_early_data_size = u32::MAX;
	cfg.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();
	Ok(cfg)
}

/// client_config will return a rustls::ClientConfig for a QUIC client. Ensures this matches the server config above!
pub fn client_config(
	cert_chain: Vec<rustls::Certificate>,
	key: rustls::PrivateKey,
) -> Result<rustls::ClientConfig, Error> {
	let mut cfg = rustls::ClientConfig::builder()
		.with_safe_default_cipher_suites()
		.with_safe_default_kx_groups()
		.with_protocol_versions(&[&rustls::version::TLS13])?
		// .with_root_certificates(root_store) // TODO: Do this
		.with_custom_certificate_verifier(ServerCertificateVerifier::dangerously_new()) // TODO: Remove this and use chain instead
		.with_single_cert(cert_chain, key)?;
	cfg.alpn_protocols = ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();
	Ok(cfg)
}

/// ServerCertificateVerifier is a custom certificate verifier that is responsible for verifying the server certificate when making a QUIC connection.
pub(crate) struct ServerCertificateVerifier; // TODO: Private this

impl ServerCertificateVerifier {
	// TODO: Private this
	pub(crate) fn dangerously_new() -> Arc<Self> {
		Arc::new(Self)
	}
}

impl ServerCertVerifier for ServerCertificateVerifier {
	fn verify_server_cert(
		&self,
		_end_entity: &Certificate,
		_intermediates: &[Certificate],
		_server_name: &ServerName,
		_scts: &mut dyn Iterator<Item = &[u8]>,
		_ocsp_response: &[u8],
		_now: std::time::SystemTime,
	) -> Result<ServerCertVerified, rustls::Error> {
		// TODO: Verify certificate expiry
		// TODO: Verify certificate algorithms match

		Ok(ServerCertVerified::assertion())
	}
}

/// ClientCertificateVerifier is a custom certificate verifier that is responsible for verifying the client certificate when making a QUIC connection.
struct AllowAllClientCertificateVerifier;

impl AllowAllClientCertificateVerifier {
	fn dangerously_new() -> Arc<dyn ClientCertVerifier> {
		Arc::new(Self {})
	}
}

impl ClientCertVerifier for AllowAllClientCertificateVerifier {
	fn offer_client_auth(&self) -> bool {
		true
	}

	fn client_auth_root_subjects(&self) -> Option<DistinguishedNames> {
		Some(vec![])
	}

	fn verify_client_cert(
		&self,
		_end_entity: &Certificate,
		_intermediates: &[Certificate],
		_now: SystemTime,
	) -> Result<ClientCertVerified, Error> {
		// TODO: Verify certificate expiry
		// TODO: Verify certificate algorithms match

		// We accept any client with a valid certificate because any valid certificate will have a valid PeerId. It's ok to accept all connections cause this is the public service.
		Ok(ClientCertVerified::assertion())
	}
}
