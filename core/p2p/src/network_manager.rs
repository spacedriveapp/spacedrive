use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use quinn::{NewConnection, VarInt};
use tokio::sync::mpsc;

use crate::{
	quic::new_client, server::Server, ConnectionType, DiscoveryManager, Identity,
	NetworkManagerError, NetworkManagerEvent, Peer, PeerCandidate, PeerId, PeerMetadata,
};

/// NetworkManager is used to manage the P2P networking between cores. This implementation is completely decoupled from the Spacedrive core to aid future refactoring and unit testing.
pub struct NetworkManager {
	pub(crate) server: Arc<Server>,
	discovery: Arc<DiscoveryManager>,
}

impl NetworkManager {
	/// Create a new NetworkManager. The 'app_name' argument must be alphanumeric.
	pub async fn new<TGetMetadata: Fn() -> PeerMetadata + Send + Sync + 'static>(
		app_name: &'static str,
		identity: Identity,
		application_channel: mpsc::Sender<NetworkManagerEvent>,
		get_metadata: TGetMetadata,
	) -> Result<Arc<Self>, NetworkManagerError> {
		if !app_name.chars().all(char::is_alphanumeric) {
			return Err(NetworkManagerError::InvalidAppName);
		}

		let server = Server::new(identity.into_rustls(), application_channel)?;
		Ok(Arc::new(Self {
			discovery: DiscoveryManager::new(app_name, server.clone(), Box::new(get_metadata))
				.await?,
			server,
		}))
	}

	/// peer_id returns the peer ID of the current node. These are unique identifier derived from the nodes public key.
	pub fn peer_id(&self) -> PeerId {
		self.server.peer_id.clone()
	}

	/// listen_addr returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.server.listen_addr
	}

	/// connect will initiate a connection to the given peer if it is available.
	pub async fn connect(&self, peer: PeerCandidate) -> Result<(), ()> {
		// TODO: Skip creating new connection if one already exists

		let NewConnection {
			connection,
			bi_streams,
			..
		} = new_client(self.server.clone(), peer.clone())
			.await
			.map_err(|err| ())?;

		if self
			.server
			.connected_peers
			.read()
			.await
			.contains_key(&peer.id)
			&& self.server.peer_id <= peer.id
		{
			println!(
				"Already found connection {:?}",
				self.server.connected_peers.read().await
			);
			connection.close(VarInt::from_u32(0), b"DUP_CONN");
			return Ok(());
		}

		let peer = Peer::new(ConnectionType::Client, peer.id, connection).unwrap();
		tokio::spawn(peer.handler(bi_streams, self.server.clone()));

		Ok(())
	}

	/// connected_peers returns a list of the connected peers.
	pub async fn connected_peers(&self) -> HashMap<PeerId, Peer> {
		self.server.connected_peers.read().await.clone()
	}

	/// discovered_peers returns a list of the discovered peers.
	pub fn discovered_peers(&self) -> HashMap<PeerId, PeerCandidate> {
		self.discovery
			.discovered_peers
			.clone()
			.into_iter()
			.collect()
	}
}
