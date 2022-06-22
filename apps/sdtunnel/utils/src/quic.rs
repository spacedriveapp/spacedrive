use std::sync::Arc;

use rustls::{
	client::{ServerCertVerified, ServerCertVerifier},
	Certificate, ServerName,
};

/// ServerCertificateVerifier is a custom certificate verifier that is responsible for verifying the server certificate when making a QUIC connection.
pub(crate) struct ServerCertificateVerifier; // TODO: Private this

impl ServerCertificateVerifier {
	// TODO: Private this
	pub(crate) fn new() -> Arc<Self> {
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
