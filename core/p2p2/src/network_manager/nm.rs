use std::{
	collections::{HashMap, HashSet},
	net::{Ipv4Addr, SocketAddr},
	sync::Arc,
	time::Duration,
};

use dashmap::{DashMap, DashSet};
use quinn::{Chunk, Endpoint, ServerConfig};
use rustls::{Certificate, PrivateKey};
use sd_tunnel_utils::{quic, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::{
	Identity, NetworkManagerConfig, NetworkManagerError, NetworkManagerInternalEvent, P2PManager,
	Peer, PeerCandidate,
};

/// TODO
pub struct NetworkManager<TP2PManager: P2PManager> {
	/// PeerId is the unique identifier of the current node.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current node.
	pub(crate) identity: (Certificate, PrivateKey),
	/// known_peers contains a list of all peers which are known to the network. These will be automatically connected if found.
	/// We store these so when making a request to the global discovery server we know who to lookup.
	pub(crate) known_peers: DashSet<PeerId>,
	/// TODO
	discovered_peers: DashMap<PeerId, PeerCandidate>,
	/// TODO
	connected_peers: DashMap<PeerId, Peer<TP2PManager>>,
	/// TODO
	pub(crate) lan_addrs: DashSet<Ipv4Addr>,
	/// TODO
	pub(crate) listen_addr: SocketAddr,
	/// manager is a type which implements P2PManager and is used so the NetworkManager can interact with the host application.
	pub(crate) manager: TP2PManager,
	/// endpoint is the QUIC endpoint that is used to send and receive network traffic between peers.
	pub(crate) endpoint: Endpoint,
	/// TODO
	internal_channel: mpsc::UnboundedSender<NetworkManagerInternalEvent>,
}

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	pub async fn new(
		identity: Identity,
		manager: TP2PManager,
		config: NetworkManagerConfig,
	) -> Result<Arc<Self>, NetworkManagerError> {
		if !TP2PManager::APPLICATION_NAME
			.chars()
			.all(char::is_alphanumeric)
		{
			return Err(NetworkManagerError::InvalidAppName);
		}

		let identity = identity.into_rustls();
		let (endpoint, incoming) = Endpoint::server(
			ServerConfig::with_crypto(Arc::new(quic::server_config(
				vec![identity.0.clone()],
				identity.1.clone(),
			)?)),
			format!("[::]:{}", config.listen_port.unwrap_or(0))
				.parse()
				.expect("unreachable error: invalid connection address. Please report if you encounter this error!"),
		)
		.map_err(NetworkManagerError::Server)?;

		let internal_channel = mpsc::unbounded_channel();
		let this = Arc::new(Self {
			peer_id: PeerId::from_cert(&identity.0),
			identity: identity,
			known_peers: config.known_peers.into_iter().collect(),
			discovered_peers: DashMap::new(),
			connected_peers: DashMap::new(),
			lan_addrs: DashSet::new(),
			listen_addr: endpoint.local_addr().map_err(NetworkManagerError::Server)?,
			manager,
			endpoint,
			internal_channel: internal_channel.0,
		});
		Self::event_loop(&this, incoming, internal_channel.1).await?;
		Ok(this)
	}

	pub(crate) fn add_discovered_peer(&self, peer: PeerCandidate) {
		self.discovered_peers.insert(peer.id.clone(), peer.clone());
		self.manager.peer_discovered(self, &peer.id);

		if self.known_peers.contains(&peer.id) {
			self.internal_channel
				.send(NetworkManagerInternalEvent::Connect(peer))
				.unwrap();
		}
	}

	pub(crate) fn remove_discovered_peer(&self, peer_id: PeerId) {
		self.discovered_peers.remove(&peer_id);
		self.manager.peer_expired(self, peer_id);
	}

	pub(crate) fn is_peer_connected(&self, peer_id: &PeerId) -> bool {
		self.connected_peers.contains_key(peer_id)
	}

	pub(crate) fn add_connected_peer(&self, peer: Peer<TP2PManager>) {
		let peer_id = peer.id.clone();
		self.connected_peers.insert(peer.id.clone(), peer);
		self.manager.peer_connected(self, peer_id);
	}

	pub(crate) fn remove_connected_peer(&self, peer_id: PeerId) {
		self.connected_peers.remove(&peer_id);
		self.manager.peer_disconnected(self, peer_id);
	}

	/// returns the peer ID of the current node. These are unique identifier derived from the nodes public key.
	pub fn peer_id(&self) -> PeerId {
		self.peer_id.clone()
	}

	/// returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.listen_addr.clone()
	}

	/// TODO
	pub fn add_known_peer(&self, peer_id: PeerId) {
		self.known_peers.insert(peer_id);
	}

	/// TODO: Docs + Error type
	pub async fn send_to(&self, peer_id: PeerId, data: &[u8]) -> Result<Chunk, ()> {
		tokio::time::sleep(Duration::from_millis(500)).await; // TODO: Fix this issue. This workaround is because DashMap is eventually consistent

		let peer = self.connected_peers.get(&peer_id).unwrap().value().clone();
		let (mut tx, mut rx) = peer.conn.open_bi().await.map_err(|err| ())?;
		tx.write(data).await.map_err(|_err| ())?;
		let (oneshot_tx, oneshot_rx) = oneshot::channel();
		tokio::spawn(async move {
			// TODO: Max length of packet should be a constant in sd-tunnel-utils::quic
			while let Ok(data) = rx.read_chunk(64 * 1024, true).await {
				match data {
					Some(data) => {
						oneshot_tx.send(data).unwrap();
						tx.finish().await;
						return;
					}
					None => {
						break;
					}
				}
			}
		});
		Ok(oneshot_rx.await.map_err(|_| ())?)
	}

	// TODO: Use stream for sending large amounts of data such as a file.
	// 	/// stream will return the tx and rx channel to a new stream.
	// 	/// TODO: Document drop behavior on streams.
	// 	pub async fn stream(&self) -> Result<(SendStream, RecvStream), ConnectionError> {
	// 		self.conn.open_bi().await
	// 	}

	/// returns a list of the connected peers.
	pub async fn connected_peers(&self) -> HashMap<PeerId, Peer<TP2PManager>> {
		self.connected_peers.clone().into_iter().collect()
	}

	/// discovered_peers returns a list of the discovered peers.
	pub fn discovered_peers(&self) -> HashMap<PeerId, PeerCandidate> {
		self.discovered_peers.clone().into_iter().collect()
	}
}
