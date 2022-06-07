use std::sync::Arc;

use futures_util::StreamExt;
use quinn::{Connection, ConnectionError, IncomingBiStreams, RecvStream, SendStream};

use crate::{server::Server, NetworkManagerEvent, PeerId, PeerMetadata};

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
#[derive(Debug, Clone)]
pub struct Peer {
	/// peer_id holds the id of the remote peer. This is their unique identifier.
	pub id: PeerId,
	/// conn_type holds the type of connection that is being established.
	pub conn_type: ConnectionType,
	/// metadata holds the metadata of the remote peer. This includes information such as their display name and version.
	pub metadata: PeerMetadata,
	/// conn holds the quinn::Connection that is being used to communicate with the remote peer. This allows creating new streams.
	pub(crate) conn: Connection,
}

impl Peer {
	pub(crate) fn new(conn_type: ConnectionType, id: PeerId, conn: Connection) -> Result<Self, ()> {
		Ok(Self {
			id,
			conn_type,
			metadata: PeerMetadata {
				// TODO: Get this from the remote client
				name: "todo".into(),
				version: None,
			},
			conn,
		})
	}

	/// handler is run in a separate thread for each peer connection and is responsible for keep the connection alive.
	pub(crate) async fn handler(self, mut bi_streams: IncomingBiStreams, server: Arc<Server>) {
		server
			.connected_peers
			.write()
			.await
			.insert(self.id.clone(), self.clone());

		while let Some(stream) = bi_streams.next().await {
			match stream {
				Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
					// self.connected_peers.remove(); // TODO
					println!("ApplicationClosed");
					break;
				}
				Err(e) => {
					println!("Error: {:?}", e); // TODO
					break;
				}
				Ok(stream) => {
					server
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

	/// stream will return the tx and rx channel to a new stream.
	pub async fn stream(&self) -> Result<(SendStream, RecvStream), ConnectionError> {
		self.conn.open_bi().await
	}
}
