use std::{
	fmt::{self, Formatter},
	sync::Arc,
};

use futures_util::StreamExt;
use quinn::{Connection, ConnectionError, IncomingBiStreams, RecvStream, SendStream};

use crate::{NetworkManager, NetworkManagerEvent, PeerId, PeerMetadata};

/// ConnectionType is used to determine the type of connection that is being established.
/// QUIC is a client/server protocol so when communication one client will be the server and one will be the client. The protocol is bi-directional so this doesn't matter a huge amount.
/// The desision for who is the client and server should be treated as arbitrary and shouldn't affect how the protocol operates.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionType {
	/// I am the QUIC server.
	Server,
	/// I am the QUIC client.
	Client,
}

/// Peer represents a currently connected peer. This struct holds all
#[derive(Clone)]
pub struct Peer {
	/// peer_id holds the id of the remote peer. This is their unique identifier.
	pub id: PeerId,
	/// conn_type holds the type of connection that is being established.
	pub conn_type: ConnectionType,
	/// metadata holds the metadata of the remote peer. This includes information such as their display name and version.
	pub metadata: PeerMetadata,
	/// conn holds the quinn::Connection that is being used to communicate with the remote peer. This allows creating new streams.
	conn: Connection,
	/// TODO
	nm: Arc<NetworkManager>,
}

impl fmt::Debug for Peer {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// TODO: Finish this
		f.debug_struct("Peer").field("id", &self.id).finish()
	}
}

impl Peer {
	pub(crate) async fn new(
		conn_type: ConnectionType,
		id: PeerId,
		conn: Connection,
		nm: Arc<NetworkManager>,
	) -> Result<Self, ()> {
		Ok(Self {
			id,
			conn_type,
			metadata: PeerMetadata {
				// TODO: Get this from the remote client
				name: "todo".into(),
				version: None,
			},
			conn,
			nm,
		})
	}

	/// handler is run in a separate thread for each peer connection and is responsible for keep the connection alive.
	pub(crate) async fn handler(self, mut bi_streams: IncomingBiStreams) {
		self.nm
			.state
			.connected_peers
			.write()
			.await
			.insert(self.id.clone(), self.clone());

		while let Some(stream) = bi_streams.next().await {
			match stream {
				Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
					self.nm.state.connected_peers.write().await.remove(&self.id);
					break;
				}
				Err(e) => {
					self.nm.state.connected_peers.write().await.remove(&self.id);
					println!("Error: {:?}", e); // TODO
					break;
				}
				Ok(stream) => {
					self.nm
						.state
						.application_channel
						.send(NetworkManagerEvent::AcceptStream {
							peer: self.clone(),
							stream,
						})
						.await
						.unwrap();
				}
			}
		}
	}

	/// TODO
	pub async fn send(&self, data: &[u8]) -> Result<(), ()> {
		let (mut tx, mut rx) = self.conn.open_bi().await.map_err(|err| ())?;
		tx.write(data).await.map_err(|_err| ())?;
		let peer = self.clone();
		let nm = self.nm.clone();
		tokio::spawn(async move {
			// TODO: Max length????
			// let rx = &rx;
			while let Ok(data) = rx.read_chunk(64 * 1024, true).await {
				match data {
					Some(data) => {
						nm.state
							.application_channel
							.send(NetworkManagerEvent::PeerRequest {
								peer: peer.clone(),
								data: data.bytes.to_vec(),
							})
							.await
							.unwrap();
						// .map_err(|err| ())?;
					}
					None => {
						break;
					}
				}
			}
		});
		Ok(())
	}

	/// stream will return the tx and rx channel to a new stream.
	/// TODO: Document drop behavior on streams.
	pub async fn stream(&self) -> Result<(SendStream, RecvStream), ConnectionError> {
		self.conn.open_bi().await
	}
}
