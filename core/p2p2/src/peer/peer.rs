use std::{
	fmt::{self, Formatter},
	io::Read,
	sync::Arc,
};

use futures_util::StreamExt;
use quinn::{ApplicationClose, Connection, IncomingBiStreams};
use sd_tunnel_utils::PeerId;

use crate::{NetworkManager, P2PManager, PeerMetadata};

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
pub struct Peer<TP2PManager: P2PManager> {
	/// peer_id holds the id of the remote peer. This is their unique identifier.
	pub id: PeerId,
	/// conn_type holds the type of connection that is being established.
	pub conn_type: ConnectionType,
	/// metadata holds the metadata of the remote peer. This includes information such as their display name and version.
	pub metadata: PeerMetadata,
	/// conn holds the quinn::Connection that is being used to communicate with the remote peer. This allows creating new streams.
	pub(crate) conn: Connection,
	/// TODO
	nm: Arc<NetworkManager<TP2PManager>>,
}

impl<TP2PManager: P2PManager> fmt::Debug for Peer<TP2PManager> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// TODO: Finish this
		f.debug_struct("Peer").field("id", &self.id).finish()
	}
}

impl<TP2PManager: P2PManager> Peer<TP2PManager> {
	pub(crate) async fn new(
		conn_type: ConnectionType,
		id: PeerId,
		conn: Connection,
		metadata: PeerMetadata,
		nm: Arc<NetworkManager<TP2PManager>>,
	) -> Result<Self, ()> {
		Ok(Self {
			id,
			conn_type,
			metadata,
			conn,
			nm,
		})
	}

	/// handler is run in a separate thread for each peer connection and is responsible for keep the connection alive.
	pub(crate) async fn handler(self, mut bi_streams: IncomingBiStreams) {
		self.nm.add_connected_peer(self.clone());
		while let Some(stream) = bi_streams.next().await {
			match stream {
				Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
					reason, ..
				})) => {
					// TODO: This is hacky, fix!
					if reason != "DUP_CONN" {
						self.nm.remove_connected_peer(self.id);
					}

					break;
				}
				Err(e) => {
					self.nm.remove_connected_peer(self.id);
					println!("Error: {:?}", e); // TODO
					break;
				}
				Ok(stream) => {
					self.nm.manager.accept_stream(&self, stream);
				}
			}
		}
	}
}
