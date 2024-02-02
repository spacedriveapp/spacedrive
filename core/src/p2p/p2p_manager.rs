use crate::{
	node::{
		config::{self, P2PDiscoveryState},
		get_hardware_model_name, HardwareModel,
	},
	p2p::{libraries, operations, sync::SyncMessage, Header, OperatingSystem, SPACEDRIVE_APP_ID},
	Node,
};

use sd_p2p2::{Mdns, Peer, QuicTransport, RemoteIdentity, UnicastStream, P2P};
use sd_p2p_tunnel::Tunnel;
use serde_json::json;
use std::{
	collections::HashMap,
	convert::Infallible,
	sync::{atomic::AtomicBool, Arc, Mutex, PoisonError},
};

use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};
use uuid::Uuid;

use super::{P2PEvents, PeerMetadata};

pub struct P2PManager {
	pub(crate) p2p: Arc<P2P>,
	mdns: Mutex<Option<Mdns>>,
	quic: Mutex<Option<QuicTransport>>,
	pub(crate) events: P2PEvents,

	// TODO: Remove these from here in future PR
	pub(super) spacedrop_pairing_reqs:
		Arc<tokio::sync::Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancelations: Arc<tokio::sync::Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	node_config: Arc<config::Manager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<config::Manager>,
		libraries: Arc<crate::library::Libraries>,
	) -> Result<(Arc<P2PManager>, impl FnOnce(Arc<Node>)), Infallible> {
		let (tx, rx) = mpsc::channel(25);
		let p2p = P2P::new(SPACEDRIVE_APP_ID, node_config.get().await.identity, tx);
		let this = Arc::new(Self {
			p2p: p2p.clone(),
			mdns: Mutex::new(None),
			quic: Mutex::new(None),
			events: P2PEvents::spawn(p2p),
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancelations: Default::default(),
			node_config,
		});
		this.on_node_config_change().await;

		libraries::start(this.p2p.clone(), libraries);

		info!(
			"Node RemoteIdentity('{}') libp2p::PeerId('{}') is now online listening at addresses: {:?}",
			this.p2p.remote_identity(),
			"todo", // TODO: Work this out??? // TODO: Work out libp2p `PeerId`
			this.p2p.listeners().values()
		);

		Ok((this.clone(), |node| {
			tokio::spawn(start(this, node, rx));
		}))
	}

	// TODO: Remove this and add a subscription system to `config::Manager`
	pub async fn on_node_config_change(&self) {
		let config = self.node_config.get().await;

		PeerMetadata {
			name: config.name.clone(),
			operating_system: Some(OperatingSystem::get_os()),
			device_model: Some(get_hardware_model_name().unwrap_or(HardwareModel::Other)),
			version: Some(env!("CARGO_PKG_VERSION").to_string()),
		}
		.update(&mut self.p2p.metadata_mut());

		let should_revert = {
			let mut quic = self.quic.lock().unwrap_or_else(PoisonError::into_inner);

			if config.p2p_enabled && quic.is_some() {
				if let Some(quic) = quic.take() {
					quic.shutdown();
				}
			}

			if !config.p2p_enabled && quic.is_none() {
				match QuicTransport::spawn(self.p2p.clone()) {
					Ok(q) => {
						*quic = Some(q);
						false
					}
					Err(err) => {
						error!("Failed to start P2P QUIC transport: {err}");
						true
					}
				}
			} else {
				false
			}
		};

		// The `should_revert` bit is weird but we need this future to stay `Send` as rspc requires.
		// To make it send we have to drop `quic` (a `!Send` `MutexGuard`).
		// Doing it within the above scope seems to not work (even when manually calling `drop`).
		if should_revert {
			let _ = self.node_config.write(|c| c.p2p_enabled = true).await;
		}

		{
			let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);

			let enabled = !config.p2p_enabled
				&& (config.p2p_discovery == P2PDiscoveryState::Everyone
					|| config.p2p_discovery == P2PDiscoveryState::ContactsOnly);

			if !enabled && mdns.is_some() {
				if let Some(mdns) = mdns.take() {
					mdns.shutdown();
				}
			}

			if enabled && mdns.is_none() {
				match Mdns::spawn(self.p2p.clone()) {
					Ok(m) => *mdns = Some(m),
					Err(err) => {
						error!("Failed to start P2P mDNS: {err}");
						// let _ = self.node_config.write(|c| c.p2p_discovery = P2PDiscoveryState::Everyone).await; // TODO: Reenable this
					}
				};
			}
		}
	}

	pub fn get_library_instances(&self, library: &Uuid) -> Vec<(RemoteIdentity, Peer)> {
		let library_id = library.to_string();
		self.p2p
			.discovered()
			.iter()
			.filter(|(_, p)| p.service().contains_key(&library_id))
			.map(|(i, p)| (*i, p.clone()))
			.collect()
	}

	pub fn get_instance(&self, library: &Uuid, identity: RemoteIdentity) -> Option<Peer> {
		let library_id = library.to_string();
		self.p2p
			.discovered()
			.iter()
			.find(|(i, p)| **i == identity && p.service().contains_key(&library_id))
			.map(|(_, p)| p.clone())
	}

	pub fn state(&self) -> serde_json::Value {
		json!({
			"self_identity": self.p2p.remote_identity().to_string(),
			// "self_peer_id": self.p2p.remote_identity().to_string(), // TODO
			"metadata": self.p2p.metadata().clone(),
			"listeners": self.p2p.listeners().iter().map(|(k, v)| (k, v.addr())).collect::<HashMap<_, _>>().clone(),
			"discovered": self.p2p.discovered().clone(),
		})
	}

	pub fn shutdown(&self) {
		// `self.p2p` will automatically take care of shutting down all the hooks. Eg. `self.quic`, `self.mdns`, etc.
		self.p2p.shutdown();
	}
}

async fn start(
	this: Arc<P2PManager>,
	node: Arc<Node>,
	mut rx: mpsc::Receiver<UnicastStream>,
) -> Result<(), ()> {
	while let Some(mut stream) = rx.recv().await {
		let header = Header::from_stream(&mut stream).await.map_err(|err| {
			error!("Failed to read header from stream: {}", err);
		})?;

		match header {
			Header::Ping => operations::ping::reciever(stream).await,
			Header::Spacedrop(req) => operations::spacedrop::reciever(&this, req, stream).await?,
			Header::Sync(library_id) => {
				let mut tunnel = Tunnel::responder(stream).await.map_err(|err| {
					error!("Failed `Tunnel::responder`: {}", err);
				})?;

				let msg = SyncMessage::from_stream(&mut tunnel).await.map_err(|err| {
					error!("Failed `SyncMessage::from_stream`: {}", err);
				})?;

				let library = node
					.libraries
					.get_library(&library_id)
					.await
					.ok_or_else(|| {
						error!("Failed to get library '{library_id}'");

						// TODO: Respond to remote client with warning!
					})?;

				match msg {
					SyncMessage::NewOperations => {
						super::sync::responder(&mut tunnel, library).await?;
					}
				};
			}
			Header::File(req) => {
				operations::request_file::receiver(&node, req, stream).await?;
			}
		};
	}

	Ok::<_, ()>(())
}
