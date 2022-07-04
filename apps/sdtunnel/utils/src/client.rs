use std::{net::ToSocketAddrs, sync::Arc};

use quinn::{ClientConfig, Connecting, Endpoint, NewConnection, VarInt};
use rustls::{Certificate, PrivateKey};

use crate::{quic::client_config, Message, MAX_MESSAGE_SIZE};

/// TODO
pub struct Client {
	backend_url: String,
	endpoint: Endpoint,
	identity: (Certificate, PrivateKey),
}

impl Client {
	pub fn new(
		backend_url: String,
		endpoint: Endpoint,
		identity: (Certificate, PrivateKey),
	) -> Self {
		Self {
			backend_url,
			endpoint,
			identity,
		}
	}

	pub async fn send_message(&self, msg: Message) -> Result<Message, ()> {
		let identity = self.identity.clone();
		let NewConnection { connection, .. } = self
			.endpoint
			.connect_with(
				ClientConfig::new(Arc::new(
					client_config(vec![identity.0], identity.1.clone()).unwrap(),
				)),
				self.backend_url
					.to_socket_addrs() // TODO: Make this only lookup IPv4 -> Filter IPV6's
					.unwrap()
					.into_iter()
					.next()
					.unwrap(),
				"todo",
			)
			.map_err(|err| {
				panic!("{}", err);
				()
			})?
			.await
			.map_err(|err| {
				panic!("{}", err);
				()
			})?;

		let (mut tx, mut rx) = connection.open_bi().await.map_err(|err| {
			panic!("{}", err);
			()
		})?;

		tx.write_all(&msg.encode().map_err(|err| ())?)
			.await
			.map_err(|err| {
				panic!("{}", err);
				()
			})?;

		let resp = rx
			.read_chunk(MAX_MESSAGE_SIZE, true)
			.await
			.map_err(|err| {
				panic!("{}", err);
				()
			})?
			.unwrap();

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
