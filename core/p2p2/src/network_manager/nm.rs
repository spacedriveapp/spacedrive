use std::{
	collections::HashMap,
	net::{Ipv4Addr, SocketAddr},
	sync::Arc,
	time::Duration,
};

use bip39::{Language, Mnemonic};
use dashmap::{DashMap, DashSet};
use quinn::{
	Chunk, ConnectionError, Endpoint, NewConnection, RecvStream, SendStream, ServerConfig,
};
use rustls::{Certificate, PrivateKey};
use sd_tunnel_utils::{quic, PeerId};
use spake2::{Ed25519Group, Password, Spake2};
use tokio::sync::{mpsc, oneshot};

use crate::{
	ConnectionEstablishmentPayload, ConnectionType, Identity, NetworkManagerConfig,
	NetworkManagerError, NetworkManagerInternalEvent, P2PManager, PairingParticipantType,
	PairingPayload, Peer, PeerCandidate, PeerMetadata,
};

/// Is the core of the P2P Library. It manages listening for and creating P2P network connections and also provides a nice API for the application embedding this library to interface with.
pub struct NetworkManager<TP2PManager: P2PManager> {
	/// PeerId is the unique identifier of the current node.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current node.
	pub(crate) identity: (Certificate, PrivateKey),
	/// known_peers contains a list of all peers which are known to the network. These will be automatically connected if found.
	/// We store these so when making a request to the global discovery server we know who to lookup.
	pub(crate) known_peers: DashSet<PeerId>,
	/// discovered_peers contains a list of all peers which have been discovered by any discovery mechanism.
	discovered_peers: DashMap<PeerId, PeerCandidate>,
	/// connected_peers
	connected_peers: DashMap<PeerId, Peer<TP2PManager>>,
	/// lan_addrs contains a list of all local addresses which exists on the current peer.
	pub(crate) lan_addrs: DashSet<Ipv4Addr>,
	/// listen_addr contains the address which the current peer is listening on. This peer will listening on IPv4 and IPv6 on a random port if none was provided at startup.
	pub(crate) listen_addr: SocketAddr,
	/// manager is a trait which implements P2PManager and is used so the NetworkManager can interact with the host application.
	pub(crate) manager: TP2PManager,
	/// endpoint is the QUIC endpoint that is used to send and receive network traffic between peers.
	pub(crate) endpoint: Endpoint,
	/// spacetunnel_server is the URL used to lookup information about the Spacetunnel server to establish a connection with.
	pub(crate) spacetunnel_url: Option<String>,
	/// internal_channel is a channel which is used to communicate with the main internal event loop.
	internal_channel: mpsc::UnboundedSender<NetworkManagerInternalEvent>,
}

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	/// Initalise a new network manager for your application.
	/// Be aware this will create a separate thread running the P2P manager event loop so this should really only be run once per application.
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
			spacetunnel_url: config.spacetunnel_url,
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

	pub(crate) fn get_discovered_peer(&self, peer_id: &PeerId) -> Option<PeerCandidate> {
		self.discovered_peers.get(peer_id).map(|v| v.clone())
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

	/// adds a new peer to the known peers list. This will cause the NetworkManager to attempt to connect to the peer if it is discovered.
	pub fn add_known_peer(&self, peer_id: PeerId) {
		self.known_peers.insert(peer_id.clone());
		self.internal_channel
			.send(NetworkManagerInternalEvent::NewKnownPeer(peer_id))
			.unwrap();
	}

	/// send a single message to a peer and await a single response. This is good for quick one-off communications but any longer term communication should be done with a stream.
	/// TODO: Error type
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

	/// stream will return the tx and rx channel to a new stream with a remote peer.
	/// Be aware that when you drop the rx channel, the stream will be closed and any data in transit will be lost.
	pub async fn stream(
		&self,
		peer_id: &PeerId,
	) -> Result<(SendStream, RecvStream), ConnectionError> {
		self.connected_peers
			.get(peer_id)
			.unwrap()
			.conn
			.open_bi()
			.await
	}

	/// returns a list of the connected peers.
	pub fn connected_peers(&self) -> HashMap<PeerId, Peer<TP2PManager>> {
		self.connected_peers.clone().into_iter().collect()
	}

	/// returns a list of the discovered peers.
	pub fn discovered_peers(&self) -> HashMap<PeerId, PeerCandidate> {
		self.discovered_peers.clone().into_iter().collect()
	}

	// initiate_pairing_with_peer will initiate a pairing with a peer.
	// This will cause the NetworkManager to attempt to connect to the peer if it is discovered and if it is, verify the preshared_key using PAKE before telling the [crate::P2PManager] that the pairing is complete.
	pub async fn initiate_pairing_with_peer(
		self: &Arc<Self>,
		remote_peer_id: PeerId,
		extra_data: HashMap<String, String>,
	) -> String {
		// TODO: Ensure we are not already paired with the peer

		let candidate = self.discovered_peers.get(&remote_peer_id).unwrap().clone();

		let m = Mnemonic::generate_in(
			Language::English,
			24, /* This library doesn't work with any number here for some reason */
		)
		.unwrap();
		let preshared_key: String = m.word_iter().take(4).collect::<Vec<_>>().join("-");

		let (spake, pake_msg) = Spake2::<Ed25519Group>::start_a(
			&Password::new(preshared_key.as_bytes()),
			&spake2::Identity::new(self.peer_id.as_bytes()),
			&spake2::Identity::new(remote_peer_id.as_bytes()),
		);

		let NewConnection {
			connection,
			bi_streams,
			..
		} = Self::connect_to_peer_internal(&self.clone(), candidate)
			.await
			.unwrap();

		let (mut tx, mut rx) = connection.open_bi().await.unwrap();

		// rmp_serde doesn't support `AsyncWrite` so we have to allocate buffer here.
		tx.write_all(
			&rmp_serde::encode::to_vec_named(&ConnectionEstablishmentPayload::PairingRequest {
				pake_msg,
				metadata: self.manager.get_metadata(),
				extra_data: extra_data.clone(),
			})
			.unwrap(),
		)
		.await
		.unwrap();

		let nm = self.clone();
		tokio::spawn(async move {
			// TODO: Get max chunk size from constant.
			let data = rx.read_chunk(64 * 1024, true).await.unwrap().unwrap();
			let payload: PairingPayload = rmp_serde::decode::from_read(&data.bytes[..]).unwrap();

			match payload {
				PairingPayload::PairingAccepted { pake_msg, metadata } => {
					let _spake_key = spake.finish(&pake_msg).unwrap();

					let resp = match nm
						.manager
						.peer_paired(
							&nm,
							PairingParticipantType::Initiator,
							&remote_peer_id,
							&metadata,
							&extra_data,
						)
						.await
					{
						Ok(_) => PairingPayload::PairingComplete,
						Err(err) => {
							println!("p2p manager error: {:?}", err);
							PairingPayload::PairingFailed
						}
					};

					// rmp_serde doesn't support `AsyncWrite` so we have to allocate buffer here.
					tx.write_all(&rmp_serde::encode::to_vec_named(&resp).unwrap())
						.await
						.unwrap();

					let peer = Peer::new(
						ConnectionType::Client,
						remote_peer_id,
						connection,
						metadata,
						nm,
					)
					.await
					.unwrap();
					tokio::spawn(peer.handler(bi_streams));
				}
				PairingPayload::PairingFailed => {
					panic!("Pairing failed");

					// TODO
					// self.manager
					// 			.peer_paired_rollback(&self, &remote_peer_id, &extra_data)
					// 			.await;

					// TODO: emit event to frontend
				}
				_ => panic!("Invalid request!"),
			}
		});

		preshared_key
	}
}
