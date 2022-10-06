use std::{
	fmt::{self, Formatter},
	sync::Arc,
};

use futures_util::StreamExt;
use quinn::{ApplicationClose, Connection, IncomingBiStreams};
use sd_tunnel_utils::PeerId;
use tracing::{debug, error};

use crate::{NetworkManager, P2PManager, PeerMetadata};

/// This emum represents the type of the connection to the current peer.
/// QUIC is a client/server protocol so when doing P2P communication one client will be the server and one will be the client from a QUIC perspective.
/// The protocol is bi-directional so this doesn't matter a huge amount and the P2P library does it's best to hide this detail from the embedding application as thinking about this can be very confusing.
/// The decision for who is the client and server should be treated as arbitrary and shouldn't affect how the protocol operates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
	/// I am the QUIC server.
	Server,
	/// I am the QUIC client.
	Client,
}

/// Represents a currently connected peer. This struct holds the connection as well as any information the network manager may required about the remote peer.
/// It also stores a reference to the network manager for communication back to the [P2PManager].
/// The [Peer] acts as an abstraction above the QUIC connection which could be a client or server so that when building code we don't have to think about the technicalities of the connection.
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
	/// nm is a reference to the network manager. This is used to send messages back to the P2PManager.
	nm: Arc<NetworkManager<TP2PManager>>,
}

impl<TP2PManager: P2PManager> fmt::Debug for Peer<TP2PManager> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Peer")
			.field("id", &self.id)
			.field("conn_type", &self.conn_type)
			.field("metadata", &self.metadata)
			.finish()
	}
}

impl<TP2PManager: P2PManager> Peer<TP2PManager> {
	/// create a new peer from a [quinn::Connection].
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

	/// handler is run in a separate thread for each peer connection and is responsible for keep the connection alive and handling incoming streams.
	pub(crate) async fn handler(self, mut bi_streams: IncomingBiStreams) {
		debug!(
			"Started handler thread for connection with remote peer '{}'",
			self.id
		);
		self.nm.add_connected_peer(self.clone());
		while let Some(stream) = bi_streams.next().await {
			match stream {
				Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
					reason, ..
				})) => {
					debug!("Connection with peer closed due to '{:?}'", reason);

					// TODO: This is hacky, fix!
					if reason != "DUP_CONN" {
						self.nm.remove_connected_peer(self.id);
					}

					break;
				}
				Err(err) => {
					error!(
						"Connection error when communicating with peer '{:?}': {:?}",
						self.id, err
					);
					self.nm.remove_connected_peer(self.id);
					break;
				}
				Ok(stream) => {
					debug!("Accepting stream from peer '{:?}'", self.id);
					self.nm.manager.accept_stream(&self, stream);
				}
			}
		}
	}
}
