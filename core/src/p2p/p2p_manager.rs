use crate::{
	node::config,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use sd_p2p::{
	spacetunnel::RemoteIdentity, Manager, ManagerConfig, ManagerError, PeerStatus, Service,
};

use std::{
	collections::{HashMap, HashSet},
	net::SocketAddr,
	sync::{atomic::AtomicBool, Arc},
};

use serde::Serialize;
use specta::Type;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tracing::info;
use uuid::Uuid;

use super::{
	LibraryMetadata, LibraryServices, P2PEvent, P2PManagerActor, PairingManager, PeerMetadata,
};

pub struct P2PManager {
	pub(crate) node: Service<PeerMetadata>,
	pub(crate) libraries: LibraryServices,

	pub events: (broadcast::Sender<P2PEvent>, broadcast::Receiver<P2PEvent>),
	pub manager: Arc<Manager>,
	pub(super) spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancelations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	pub pairing: Arc<PairingManager>,
	node_config_manager: Arc<config::Manager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<config::Manager>,
		libraries: Arc<crate::library::Libraries>,
	) -> Result<(Arc<P2PManager>, P2PManagerActor), ManagerError> {
		let (keypair, manager_config) = {
			let config = node_config.get().await;
			(config.keypair, config.p2p.clone())
		};

		let (manager, stream) =
			sd_p2p::Manager::new(SPACEDRIVE_APP_ID, &keypair, manager_config).await?;

		info!(
			"Node RemoteIdentity('{}') libp2p::PeerId('{}') is now online listening at addresses: {:?}",
			manager.identity(),
			manager.libp2p_peer_id(),
			stream.listen_addrs()
		);

		// need to keep 'rx' around so that the channel isn't dropped
		let (tx, rx) = broadcast::channel(100);
		let pairing = PairingManager::new(manager.clone(), tx.clone());

		let (register_service_tx, register_service_rx) = mpsc::channel(10);
		let this = Arc::new(Self {
			node: Service::new("node", manager.clone())
				.expect("Hardcoded service name will never be a duplicate!"),
			libraries: LibraryServices::new(register_service_tx),
			pairing,
			events: (tx, rx),
			manager,
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancelations: Default::default(),
			node_config_manager: node_config,
		});
		this.update_metadata().await;

		tokio::spawn(LibraryServices::start(this.clone(), libraries));

		Ok((
			this.clone(),
			P2PManagerActor {
				manager: this,
				stream,
				register_service_rx,
			},
		))
	}

	pub fn get_library_service(&self, library_id: &Uuid) -> Option<Arc<Service<LibraryMetadata>>> {
		self.libraries.get(library_id)
	}

	pub async fn update_metadata(&self) {
		self.node.update({
			let config = self.node_config_manager.get().await;
			PeerMetadata {
				name: config.name.clone(),
				operating_system: Some(OperatingSystem::get_os()),
				version: Some(env!("CARGO_PKG_VERSION").to_string()),
			}
		});
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.0.subscribe()
	}

	// TODO: Replace this with a better system that is more built into `sd-p2p` crate
	pub fn state(&self) -> P2PState {
		let (
			self_peer_id,
			self_identity,
			config,
			manager_connected,
			manager_connections,
			dicovery_services,
			discovery_discovered,
			discovery_known,
		) = self.manager.get_debug_state();

		P2PState {
			node: self.node.get_state(),
			libraries: self
				.libraries
				.libraries()
				.into_iter()
				.map(|(id, lib)| (id, lib.get_state()))
				.collect(),
			self_peer_id: PeerId(self_peer_id),
			self_identity,
			config,
			manager_connected: manager_connected
				.into_iter()
				.map(|(k, v)| (PeerId(k), v))
				.collect(),
			manager_connections: manager_connections.into_iter().map(PeerId).collect(),
			dicovery_services,
			discovery_discovered: discovery_discovered
				.into_iter()
				.map(|(k, v)| {
					(
						k,
						v.into_iter()
							.map(|(k, (k1, v, b))| (k, (PeerId(k1), v, b)))
							.collect(),
					)
				})
				.collect(),
			discovery_known,
		}
	}

	pub async fn shutdown(&self) {
		self.manager.shutdown().await;
	}
}

#[derive(Debug, Serialize, Type)]
#[allow(clippy::type_complexity)]
pub struct P2PState {
	node: HashMap<RemoteIdentity, PeerStatus>,
	libraries: Vec<(Uuid, HashMap<RemoteIdentity, PeerStatus>)>,
	self_peer_id: PeerId,
	self_identity: RemoteIdentity,
	config: ManagerConfig,
	manager_connected: HashMap<PeerId, RemoteIdentity>,
	manager_connections: HashSet<PeerId>,
	dicovery_services: HashMap<String, Option<HashMap<String, String>>>,
	discovery_discovered: HashMap<
		String,
		HashMap<RemoteIdentity, (PeerId, HashMap<String, String>, Vec<SocketAddr>)>,
	>,
	discovery_known: HashMap<String, HashSet<RemoteIdentity>>,
}

// TODO: Get this back into `sd-p2p` but keep it private
#[derive(Debug, Serialize, Type, Hash, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct PeerId(#[specta(type = String)] sd_p2p::internal::PeerId);
