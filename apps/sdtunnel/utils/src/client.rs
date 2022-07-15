use std::{io, net::ToSocketAddrs, sync::Arc};

use quinn::{ClientConfig, Endpoint, NewConnection};
use rustls::{Certificate, PrivateKey};
use thiserror::Error;

use crate::{
	quic::client_config,
	rmp_quic::{read_value, UtilError},
	write_value, Message,
};

/// represents an error which can be thrown by the client.
#[derive(Error, Debug)]
pub enum ClientError {
	#[error("no valid Spacetunnel addresses were provided")]
	MissingServerAddr,
	#[error("error resolving DNS for Spacetunnel address")]
	IoError(#[from] io::Error),
	#[error("error setting up client TLS")]
	TlsError(#[from] rustls::Error),
	#[error("error connecting to Spacetunnel")]
	ConnectError(#[from] quinn::ConnectError),
	#[error("error communicating with Spacetunnel")]
	ConnectionError(#[from] quinn::ConnectionError),
	#[error("error writing message to Spacetunnel")]
	UtilError(#[from] UtilError),
	#[error("error writing message to Spacetunnel connection")]
	WriteError(#[from] quinn::WriteError),
}

/// holds a connection to the Spacetunnel server and can be used to send messages to the server.
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

	/// sends a message to the Spacetunnel server and awaits a response.
	pub async fn send_message(&self, msg: Message) -> Result<Message, ClientError> {
		let identity = self.identity.clone();
		let NewConnection { connection, .. } = self
			.endpoint
			.connect_with(
				ClientConfig::new(Arc::new(client_config(
					vec![identity.0],
					identity.1.clone(),
				)?)),
				self.backend_url
					.to_socket_addrs()? // TODO: Make this only lookup IPv4 -> Filter IPV6's
					.into_iter()
					.next()
					.ok_or(ClientError::MissingServerAddr)?,
				"todo",
			)?
			.await?;

		let (mut tx, mut rx) = connection.open_bi().await?;
		write_value(&mut tx, &msg).await?;
		let msg: Message = read_value(&mut rx).await?;

		// tx.finish().await?;
		// connection.close(VarInt::from_u32(0), b"DUP_CONN");

		Ok(msg)
	}
}
