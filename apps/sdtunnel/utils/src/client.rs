use std::sync::Arc;

use quinn::{ClientConfig, Connecting, Endpoint, NewConnection, VarInt};

use crate::Message;

/// TODO: Dynamically discover this url!
pub const SPACETUNNEL_URL: &'static str = "127.0.0.1:443"; // TODO: tunnel.spacedrive.com

/// TODO
pub struct Client {
	endpoint: Endpoint,
}

impl Client {
	pub fn new(endpoint: Endpoint) -> Self {
		Self { endpoint }
	}

	pub async fn send_message(&self, msg: Message) -> Result<Message, ()> {
		let mut client_crypto = rustls::ClientConfig::builder()
			.with_safe_default_cipher_suites()
			.with_safe_default_kx_groups()
			.with_protocol_versions(&[&rustls::version::TLS13])
			.unwrap()
			.with_custom_certificate_verifier(
				crate::quic::ServerCertificateVerifier::new(),
			)
			.with_no_client_auth(); // TODO: Make server certificate verification secure
						// TODO: set QUIC ALPN

		let NewConnection { connection, .. } = self
			.endpoint
			.connect_with(
				ClientConfig::new(Arc::new(client_crypto.clone())),
				SPACETUNNEL_URL.parse().unwrap(),
				"todo",
			)
			.map_err(|err| ())?
			.await
			.map_err(|err| ())?;

		let (mut tx, mut rx) = connection.open_bi().await.map_err(|err| ())?;

		tx.write_all(&msg.encode().map_err(|err| ())?)
			.await
			.map_err(|err| ())?;

		println!("A");

		let mut resp = rx
			.read_chunk(64 * 1024 /* TODO: Constant */, true)
			.await
			.map_err(|err| ())?
			.unwrap();

		println!("B");

		let mut bytes: &[u8] = &resp.bytes;
		let msg = Message::read(&mut bytes).map_err(|err| ())?;

		// tx.finish().await.map_err(|err| {
		// 	panic!("{}", err);
		// 	()
		// })?;

		// connection.close(VarInt::from_u32(0), b"DUP_CONN");

		Ok(msg)
	}
}
