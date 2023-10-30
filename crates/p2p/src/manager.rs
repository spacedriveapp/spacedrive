use std::{
	collections::{HashMap, HashSet},
	fmt,
	net::SocketAddr,
	sync::{
		atomic::{AtomicBool, AtomicU64},
		Arc, PoisonError, RwLock,
	},
};

use libp2p::{
	core::{muxing::StreamMuxerBox, transport::ListenerId, ConnectedPoint},
	swarm::SwarmBuilder,
	PeerId, Transport,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, warn};

use crate::{
	spacetime::{SpaceTime, UnicastStream},
	spacetunnel::{Identity, RemoteIdentity},
	DiscoveryManager, DiscoveryManagerState, Keypair, ManagerStream, ManagerStreamAction,
	ManagerStreamAction2,
};

// State of the manager that may infrequently change
// These are broken out so updates to them can be done in sync (With single RwLock lock)
#[derive(Debug)]
pub(crate) struct DynamicManagerState {
	pub(crate) config: ManagerConfig,
	pub(crate) ipv4_listener_id: Option<ListenerId>,
	pub(crate) ipv6_listener_id: Option<ListenerId>,
	// A map of connected clients.
	// This includes both inbound and outbound connections!
	pub(crate) connected: HashMap<libp2p::PeerId, RemoteIdentity>,
	// TODO: Removing this would be nice. It's a hack to things working after removing the `PeerId` from public API.
	pub(crate) connections: HashMap<libp2p::PeerId, (ConnectedPoint, usize)>,
}

/// Is the core component of the P2P system that holds the state and delegates actions to the other components
pub struct Manager {
	pub(crate) peer_id: PeerId,
	pub(crate) identity: Identity,
	pub(crate) application_name: String,
	pub(crate) stream_id: AtomicU64,
	pub(crate) state: RwLock<DynamicManagerState>,
	pub(crate) discovery_state: Arc<RwLock<DiscoveryManagerState>>,
	event_stream_tx: mpsc::Sender<ManagerStreamAction>,
	event_stream_tx2: mpsc::Sender<ManagerStreamAction2>,
}

impl fmt::Debug for Manager {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Debug").finish()
	}
}

impl Manager {
	/// create a new P2P manager. Please do your best to make the callback closures as fast as possible because they will slow the P2P event loop!
	pub async fn new(
		application_name: &'static str,
		keypair: &Keypair,
		config: ManagerConfig,
	) -> Result<(Arc<Self>, ManagerStream), ManagerError> {
		application_name
			.chars()
			.all(|c| char::is_alphanumeric(c) || c == '-')
			.then_some(())
			.ok_or(ManagerError::InvalidAppName)?;

		let peer_id = keypair.peer_id();
		let (event_stream_tx, event_stream_rx) = mpsc::channel(128);
		let (event_stream_tx2, event_stream_rx2) = mpsc::channel(128);

		let config2 = config.clone();
		let (discovery_state, service_shutdown_rx) = DiscoveryManagerState::new();
		let this = Arc::new(Self {
			application_name: format!("/{}/spacetime/1.0.0", application_name),
			identity: keypair.to_identity(),
			stream_id: AtomicU64::new(0),
			state: RwLock::new(DynamicManagerState {
				config,
				ipv4_listener_id: None,
				ipv6_listener_id: None,
				connected: Default::default(),
				connections: Default::default(),
			}),
			discovery_state,
			peer_id,
			event_stream_tx,
			event_stream_tx2,
		});

		let mut swarm = SwarmBuilder::with_tokio_executor(
			libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
				libp2p_quic::Config::new(&keypair.inner()),
			)
			.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
			.boxed(),
			SpaceTime::new(this.clone()),
			keypair.peer_id(),
		)
		.build();

		ManagerStream::refresh_listeners(
			&mut swarm,
			&mut this.state.write().unwrap_or_else(PoisonError::into_inner),
		);

		Ok((
			this.clone(),
			ManagerStream {
				discovery_manager: DiscoveryManager::new(
					application_name,
					this.identity.to_remote_identity(),
					this.peer_id,
					&config2,
					this.discovery_state.clone(),
					service_shutdown_rx,
				)?,
				manager: this,
				event_stream_rx,
				event_stream_rx2,
				swarm,
				queued_events: Default::default(),
				shutdown: AtomicBool::new(false),
				on_establish_streams: HashMap::new(),
			},
		))
	}

	pub(crate) async fn emit(&self, event: ManagerStreamAction) {
		match self.event_stream_tx.send(event).await {
			Ok(_) => {}
			Err(err) => warn!("error emitting event: {}", err),
		}
	}

	pub fn identity(&self) -> RemoteIdentity {
		self.identity.to_remote_identity()
	}

	pub fn libp2p_peer_id(&self) -> PeerId {
		self.peer_id
	}

	pub async fn update_config(&self, config: ManagerConfig) {
		self.emit(ManagerStreamAction::UpdateConfig(config)).await;
	}

	pub async fn get_connected_peers(&self) -> Result<Vec<RemoteIdentity>, ()> {
		let (tx, rx) = oneshot::channel();
		self.emit(ManagerStreamAction::GetConnectedPeers(tx)).await;
		rx.await.map_err(|_| {
			warn!("failed to get connected peers 3 times, returning error");
		})
	}

	// TODO: Maybe remove this?
	pub async fn stream(&self, identity: RemoteIdentity) -> Result<UnicastStream, ()> {
		let peer_id = {
			let state = self
				.discovery_state
				.read()
				.unwrap_or_else(PoisonError::into_inner);

			// TODO: This should not depend on a `Service` existing. Either we should store discovered peers separatly for this or we should remove this method (prefered).
			state
				.discovered
				.iter()
				.find_map(|(_, i)| i.iter().find(|(i, _)| **i == identity))
				.ok_or(())?
				.1
				.peer_id
		};

		self.stream_inner(peer_id).await
	}

	// TODO: Should this be private now that connections can be done through the `Service`.
	// TODO: Does this need any timeouts to be added cause hanging forever is bad?
	// be aware this method is `!Sync` so can't be used from rspc. // TODO: Can this limitation be removed?
	#[allow(clippy::unused_unit)] // TODO: Remove this clippy override once error handling is added
	pub(crate) async fn stream_inner(&self, peer_id: PeerId) -> Result<UnicastStream, ()> {
		// TODO: With this system you can send to any random peer id. Can I reduce that by requiring `.connect(peer_id).unwrap().send(data)` or something like that.
		let (tx, rx) = oneshot::channel();
		match self
			.event_stream_tx2
			.send(ManagerStreamAction2::StartStream(peer_id, tx))
			.await
		{
			Ok(_) => {}
			Err(err) => warn!("error emitting event: {}", err),
		}
		let stream = rx.await.map_err(|_| {
			warn!("failed to queue establishing stream to peer '{peer_id}'!");

			()
		})?;
		Ok(stream.build(self, peer_id).await)
	}

	pub async fn broadcast(&self, data: Vec<u8>) {
		self.emit(ManagerStreamAction::BroadcastData(data)).await;
	}

	// TODO: Cleanup return type and this API in general
	#[allow(clippy::type_complexity)]
	pub fn get_debug_state(
		&self,
	) -> (
		PeerId,
		RemoteIdentity,
		ManagerConfig,
		HashMap<PeerId, RemoteIdentity>,
		HashSet<PeerId>,
		HashMap<String, Option<HashMap<String, String>>>,
		HashMap<
			String,
			HashMap<RemoteIdentity, (PeerId, HashMap<String, String>, Vec<SocketAddr>)>,
		>,
		HashMap<String, HashSet<RemoteIdentity>>,
	) {
		let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
		let discovery_state = self
			.discovery_state
			.read()
			.unwrap_or_else(PoisonError::into_inner);

		(
			self.peer_id,
			self.identity.to_remote_identity(),
			state.config.clone(),
			state.connected.clone(),
			state.connections.keys().copied().collect(),
			discovery_state
				.services
				.iter()
				.map(|(k, v)| (k.clone(), v.1.clone()))
				.collect(),
			discovery_state
				.discovered
				.iter()
				.map(|(k, v)| {
					(
						k.clone(),
						v.clone()
							.iter()
							.map(|(k, v)| (*k, (v.peer_id, v.meta.clone(), v.addresses.clone())))
							.collect::<HashMap<_, _>>(),
					)
				})
				.collect(),
			discovery_state.known.clone(),
		)
	}

	pub async fn shutdown(&self) {
		let (tx, rx) = oneshot::channel();
		if self
			.event_stream_tx
			.send(ManagerStreamAction::Shutdown(tx))
			.await
			.is_ok()
		{
			rx.await.unwrap_or_else(|_| {
				warn!("Error receiving shutdown signal to P2P Manager!");
			}); // Await shutdown so we don't kill the app before the Mdns broadcast
		} else {
			warn!("p2p was already shutdown, skipping...");
		}
	}
}

#[derive(Error, Debug)]
pub enum ManagerError {
	#[error(
		"the application name you application provided is invalid. Ensure it is alphanumeric!"
	)]
	InvalidAppName,
	#[error("error with mdns discovery: {0}")]
	Mdns(#[from] mdns_sd::Error),
}

/// The configuration for the P2P Manager
/// DO NOT MAKE BREAKING CHANGES - This is embedded in the `node_config.json`
/// For future me: `Keypair` is not on here cause hot reloading it hard.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ManagerConfig {
	// Enable or disable the P2P layer
	pub enabled: bool,
	// `None` will chose a random free port on startup
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub port: Option<u16>,
}

impl Default for ManagerConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			port: None,
		}
	}
}
