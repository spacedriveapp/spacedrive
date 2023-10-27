use std::{
	collections::HashMap,
	sync::{atomic::AtomicBool, Arc},
};

use sd_p2p::{spacetunnel::RemoteIdentity, Manager, ManagerError, PeerStatus, Service};
use serde::Serialize;
use specta::Type;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tracing::info;
use uuid::Uuid;

use crate::{
	node::config,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

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
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			stream.listen_addrs()
		);

		// need to keep 'rx' around so that the channel isn't dropped
		let (tx, rx) = broadcast::channel(100);
		let pairing = PairingManager::new(manager.clone(), tx.clone());

		let (register_service_tx, register_service_rx) = mpsc::channel(10);
		let this = Arc::new(Self {
			node: Service::new("node", manager.clone()).unwrap(),
			libraries: LibraryServices::new(register_service_tx),
			pairing,
			events: (tx, rx),
			manager,
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancelations: Default::default(),
			node_config_manager: node_config,
		});

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
		Some(self.libraries.get(library_id)?)
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

	pub fn state(&self) -> P2PState {
		P2PState {
			libraries: self
				.libraries
				.libraries()
				.into_iter()
				.map(|(id, lib)| lib.get_state())
				.collect(),
		}
	}

	pub async fn shutdown(&self) {
		self.manager.shutdown().await;
	}
}

#[derive(Debug, Serialize, Type)]
pub struct P2PState {
	libraries: Vec<(Uuid, HashMap<RemoteIdentity, PeerStatus>)>,
}
