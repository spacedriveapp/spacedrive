use std::{
	collections::HashMap,
	net::{Ipv4Addr, SocketAddr},
	sync::Arc,
	time::Duration,
};

use bip39::{Language, Mnemonic};
use dashmap::{DashMap, DashSet};
use futures_util::future::join_all;
use quinn::{Chunk, Endpoint, NewConnection, RecvStream, SendStream, ServerConfig};
use rustls::{Certificate, PrivateKey};
use spake2::{Ed25519Group, Password, Spake2};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};
use tunnel_utils::{quic, write_value, PeerId, UtilError};

use crate::{
	ConnectError, ConnectionEstablishmentPayload, ConnectionType, Identity, NetworkManagerConfig,
	NetworkManagerError, NetworkManagerInternalEvent, P2PManager, PairingParticipantType,
	PairingPayload, Peer, PeerCandidate,
};

/// Is the core of the P2P Library. It manages listening for and creating P2P network connections and also provides a nice API for the application embedding this library to interface with.
pub struct NetworkManager<TP2PManager: P2PManager> {
	/// PeerId is the unique identifier of the current peer.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current peer.
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
		debug!("Creating new NetworkManager...");

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
			identity,
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
		debug!("Discovered peer: {:?}", peer);
		self.discovered_peers.insert(peer.id.clone(), peer.clone());
		self.manager.peer_discovered(self, &peer.id);

		if self.known_peers.contains(&peer.id) {
			match self
				.internal_channel
				.send(NetworkManagerInternalEvent::Connect(peer))
			{
				Ok(_) => {}
				Err(err) => {
					error!("Failed to send on internal_channel: {:?}", err);
				}
			}
		}
	}

	pub(crate) fn remove_discovered_peer(&self, peer_id: PeerId) {
		debug!("Removing discovered peer: {:?}", peer_id);
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
		debug!("Connected with peer: {:?}", peer);
		let peer_id = peer.id.clone();
		self.connected_peers.insert(peer.id.clone(), peer);
		self.manager.peer_connected(self, peer_id);
	}

	pub(crate) fn remove_connected_peer(&self, peer_id: PeerId) {
		debug!("Disconnected with peer: {:?}", peer_id);
		self.connected_peers.remove(&peer_id);
		self.manager.peer_disconnected(self, peer_id);
	}

	/// returns the peer ID of the current peer. These are unique identifier derived from the peers public key.
	pub fn peer_id(&self) -> PeerId {
		self.peer_id.clone()
	}

	/// returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.listen_addr
	}

	/// adds a new peer to the known peers list. This will cause the NetworkManager to attempt to connect to the peer if it is discovered.
	pub fn add_known_peer(&self, peer_id: PeerId) {
		debug!("Adding '{:?}' as a known peer", peer_id);
		self.known_peers.insert(peer_id.clone());

		match self
			.internal_channel
			.send(NetworkManagerInternalEvent::NewKnownPeer(peer_id))
		{
			Ok(_) => {}
			Err(err) => {
				error!("Failed to send on internal_channel: {:?}", err);
			}
		}
	}

	/// send a single message to a peer and await a single response. This is good for quick one-off communications but any longer term communication should be done with a stream.
	/// TODO: Error type
	pub async fn send_to(&self, peer_id: PeerId, data: &[u8]) -> Result<Chunk, NMError> {
		debug!("Sending message to '{:?}'", peer_id);

		tokio::time::sleep(Duration::from_millis(500)).await; // TODO: Fix this issue. This workaround is because DashMap is eventually consistent

		let peer = self
			.connected_peers
			.get(&peer_id)
			.ok_or(NMError::PeerNotConnected)?
			.value()
			.clone();
		let (mut tx, mut rx) = peer.conn.open_bi().await?;
		tx.write(data).await?;
		let (oneshot_tx, oneshot_rx) = oneshot::channel();
		tokio::spawn(async move {
			// TODO: Max length of packet should be a constant in tunnel-utils::quic
			match rx.read_chunk(64 * 1024, true).await {
				Ok(Some(data)) => match oneshot_tx.send(data) {
					Ok(_) => match tx.finish().await {
						Ok(_) => {}
						Err(err) => {
							warn!("Failed to finish connection: {:?}", err);
						}
					},
					Err(_) => {
						error!("Failed to transmit result back to `NetworkManager::send_to` using oneshot! `send_to` will timeout and this error can be ignored.");
					}
				},
				Ok(None) => {}
				Err(err) => {
					warn!(
						"Failed to read from stream with peer '{}': {:?}",
						peer.id, err
					);
				}
			}
		});
		// TODO: add timeout for oneshot
		Ok(oneshot_rx.await?)
	}

	pub fn broadcast(self: &Arc<Self>, data: Vec<u8>) {
		let mut connections = Vec::with_capacity(self.connected_peers.len());
		for peer in self.connected_peers.iter() {
			connections.push((
				peer.key().clone(),
				peer.value().conn.open_bi(),
				data.clone(),
			));
		}
		let connections = connections
			.into_iter()
			.map(move |(peer_id, conn, data)| async move {
				match conn.await {
					Ok((mut tx, _)) => match tx.write(&data).await {
						Ok(_) => {}
						Err(err) => {
							warn!(
								"Failed to write to stream with peer '{}': {:?}",
								peer_id, err
							);
						}
					},
					Err(err) => {
						warn!(
							"Failed to write to stream with peer '{}': {:?}",
							peer_id, err
						);
					}
				}
			});

		tokio::spawn(join_all(connections));
	}

	/// stream will return the tx and rx channel to a new stream with a remote peer.
	/// Be aware that when you drop the rx channel, the stream will be closed and any data in transit will be lost.
	pub async fn stream(&self, peer_id: &PeerId) -> Result<(SendStream, RecvStream), NMError> {
		debug!("Opening stream with peer '{:?}'", peer_id);

		Ok(self
			.connected_peers
			.get(peer_id)
			.ok_or(NMError::PeerNotConnected)?
			.conn
			.open_bi()
			.await?)
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
	) -> Result<String, NMError> {
		debug!("Starting pairing with '{:?}'", remote_peer_id);

		// TODO: Ensure we are not already paired with the peer

		let candidate = self
			.discovered_peers
			.get(&remote_peer_id)
			.ok_or(NMError::PeerNotFound)?
			.clone();

		let m = Mnemonic::generate_in(
			Language::English,
			24, /* This library doesn't work with any number here for some reason */
		)?;
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
		} = Self::connect_to_peer_internal(&self.clone(), candidate).await?;

		let (mut tx, mut rx) = connection.open_bi().await?;

		write_value(
			&mut tx,
			&ConnectionEstablishmentPayload::PairingRequest {
				pake_msg,
				metadata: self.manager.get_metadata(),
				extra_data: extra_data.clone(),
			},
		)
		.await?;

		let nm = self.clone();
		tokio::spawn(async move {
			// TODO: Timeout if reading chunk is not quick

			// TODO: Get max chunk size from constant.
			let data = match rx.read_chunk(64 * 1024, true).await {
				Ok(Some(data)) => data,
				Ok(None) => {
					warn!("connection closed before we could read from it!");
					return;
				}
				Err(err) => {
					warn!("error reading from connection: {}", err);
					return;
				}
			};

			let payload = match rmp_serde::decode::from_read(&data.bytes[..]) {
				Ok(payload) => payload,
				Err(err) => {
					warn!("error decoding pairing payload: {}", err);
					return;
				}
			};

			match payload {
				PairingPayload::PairingAccepted { pake_msg, metadata } => {
					match spake.finish(&pake_msg) {
						Ok(_) => {} // We only use SPAKE2 to ensure the current connection is to the peer we expect, hence we don't use the key which is returned.
						Err(err) => {
							warn!(
								"error pairing with peer. Connection has been tampered with! err: {:?}",
								err
							);
							return;
						}
					};

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
							warn!("p2p manager error: {:?}", err);
							PairingPayload::PairingFailed
						}
					};

					match write_value(&mut tx, &resp).await {
						Ok(_) => {}
						Err(err) => {
							warn!(
								"error encoding and sending pairing response to connection: {}",
								err
							);
							return;
						}
					}

					match Peer::new(
						ConnectionType::Client,
						remote_peer_id,
						connection,
						metadata,
						nm,
					)
					.await
					{
						Ok(peer) => {
							tokio::spawn(peer.handler(bi_streams));
						}
						Err(err) => {
							warn!("error creating peer: {:?}", err);
						}
					}
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

		Ok(preshared_key)
	}
}

// TODO: rename + docs
#[derive(Error, Debug)]
pub enum NMError {
	#[error("The peer is not currently connected")]
	PeerNotConnected,
	#[error("The peer could not be found")]
	PeerNotFound,
	#[error("Error communicating with peer")]
	ConnectionError(#[from] quinn::ConnectionError),
	#[error("Error communicating with peer")]
	UtilError(#[from] UtilError),
	#[error("Internal error receiving result from oneshot")]
	RecvError(#[from] oneshot::error::RecvError),
	#[error("Error writing message to peer")]
	WriteError(#[from] quinn::WriteError),
	#[error("Error connecting to peer")]
	ConnectError(#[from] ConnectError),
	#[error("Error generating preshared key")]
	GeneratePresharedKeyError(#[from] bip39::Error),
}
