use crate::{
	node::{
		config::{self, P2PDiscoveryState, Port},
		get_hardware_model_name, HardwareModel,
	},
	p2p::{libraries, operations, sync::SyncMessage, Header, OperatingSystem, SPACEDRIVE_APP_ID},
	Node,
};

use sd_p2p2::{
	flume::{bounded, Receiver},
	Libp2pPeerId, Listener, Mdns, Peer, QuicTransport, RemoteIdentity, UnicastStream, P2P,
};
use sd_p2p_tunnel::Tunnel;
use serde::Serialize;
use serde_json::json;
use specta::Type;
use std::{
	collections::{HashMap, HashSet},
	convert::Infallible,
	net::SocketAddr,
	sync::{atomic::AtomicBool, Arc, Mutex, PoisonError},
};

use tokio::sync::oneshot;
use tracing::{error, info};
use uuid::Uuid;

use super::{P2PEvents, PeerMetadata};

pub struct P2PManager {
	pub(crate) p2p: Arc<P2P>,
	mdns: Mutex<Option<Mdns>>,
	quic: QuicTransport,
	// The `libp2p::PeerId`. This is for debugging only, use `RemoteIdentity` instead.
	lp2p_peer_id: Libp2pPeerId,
	pub(crate) events: P2PEvents,

	pub(super) spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancelations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	pub(crate) node_config: Arc<config::Manager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<config::Manager>,
		libraries: Arc<crate::library::Libraries>,
	) -> Result<(Arc<P2PManager>, impl FnOnce(Arc<Node>)), Infallible> {
		let (tx, rx) = bounded(25);
		let p2p = P2P::new(SPACEDRIVE_APP_ID, node_config.get().await.identity, tx);
		let (quic, lp2p_peer_id) = QuicTransport::spawn(p2p.clone()).unwrap(); // TODO: Error handling
		let this = Arc::new(Self {
			p2p: p2p.clone(),
			lp2p_peer_id,
			mdns: Mutex::new(None),
			quic,
			events: P2PEvents::spawn(p2p),
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancelations: Default::default(),
			node_config,
		});
		this.on_node_config_change().await;

		libraries::start(this.p2p.clone(), libraries);

		info!(
			"Node RemoteIdentity('{}') libp2p::PeerId('{:?}') is now online listening at addresses: {:?}",
			this.p2p.remote_identity(),
			this.lp2p_peer_id,
			this.p2p.listeners()
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

		if let Err(err) = self
			.quic
			.set_ipv4_enabled(match config.p2p_ipv4_port {
				Port::Disabled => None,
				Port::Random => Some(0),
				Port::Discrete(port) => Some(port),
			})
			.await
		{
			error!("Failed to enabled quic ipv4 listener: {err}");
			self.node_config
				.write(|c| c.p2p_ipv4_port = Port::Disabled)
				.await
				.ok();
		}

		if let Err(err) = self
			.quic
			.set_ipv6_enabled(match config.p2p_ipv6_port {
				Port::Disabled => None,
				Port::Random => Some(0),
				Port::Discrete(port) => Some(port),
			})
			.await
		{
			error!("Failed to enabled quic ipv6 listener: {err}");
			self.node_config
				.write(|c| c.p2p_ipv6_port = Port::Disabled)
				.await
				.ok();
		}

		let should_revert = match config.p2p_discovery {
			P2PDiscoveryState::Everyone
			// TODO: Make `ContactsOnly` work
			| P2PDiscoveryState::ContactsOnly => {
				let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);
				if mdns.is_none() {
					match Mdns::spawn(self.p2p.clone()) {
						Ok(m) => {
							*mdns = Some(m);
							false
						}
						Err(err) => {
							error!("Failed to start mDNS: {err}");
							true
						}
					}
				} else {
					false
				}
			}
			P2PDiscoveryState::Disabled => {
				if let Some(mdns) = self.mdns.lock().unwrap_or_else(PoisonError::into_inner).take() {
					mdns.shutdown();
				}

				false
			},
		};

		// The `should_revert` bit is weird but we need this future to stay `Send` as rspc requires.
		// To make it send we have to drop `quic` (a `!Send` `MutexGuard`).
		// Doing it within the above scope seems to not work (even when manually calling `drop`).
		if should_revert {
			let _ = self
				.node_config
				.write(|c| c.p2p_discovery = P2PDiscoveryState::Disabled)
				.await;
		}
	}

	pub fn get_library_instances(&self, library: &Uuid) -> Vec<(RemoteIdentity, Arc<Peer>)> {
		let library_id = library.to_string();
		self.p2p
			.peers()
			.iter()
			.filter(|(_, p)| p.metadata().contains_key(&library_id))
			.map(|(i, p)| (*i, p.clone()))
			.collect()
	}

	pub fn get_instance(&self, library: &Uuid, identity: RemoteIdentity) -> Option<Arc<Peer>> {
		let library_id = library.to_string();
		self.p2p
			.peers()
			.iter()
			.find(|(i, p)| **i == identity && p.metadata().contains_key(&library_id))
			.map(|(_, p)| p.clone())
	}

	pub fn state(&self) -> serde_json::Value {
		let listeners = self.p2p.listeners();
		json!({
			"self_identity": self.p2p.remote_identity().to_string(),
			"self_peer_id": format!("{:?}", self.lp2p_peer_id),
			"metadata": self.p2p.metadata().clone(),
			"peers": self.p2p.peers().iter().map(|(identity, p)| json!({
				"identity": identity.to_string(),
				"metadata": p.metadata().clone(),
				"can_connect": p.can_connect(),
				"is_connected": p.is_connected(),
				"active_connections": p.active_connections(),
				"connection_methods": p.connection_methods().iter().map(|id| format!("{:?}", id)).collect::<Vec<_>>(),
				"discovered_by": p.discovered_by().iter().map(|id| format!("{:?}", id)).collect::<Vec<_>>(),
			})).collect::<Vec<_>>(),
			"hooks": self.p2p.hooks().iter().map(|(id, name)| json!({
				"id": format!("{:?}", id),
				"name": name,
				"listener_addrs": listeners.iter().find(|l| l.is_hook_id(*id)).map(|l| l.addrs.clone()),
			})).collect::<Vec<_>>(),

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
	rx: Receiver<UnicastStream>,
) -> Result<(), ()> {
	while let Ok(mut stream) = rx.recv_async().await {
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

#[derive(Debug, Serialize, Type)]
pub struct Listener2 {
	pub id: String,
	pub name: &'static str,
	pub addrs: HashSet<SocketAddr>,
}

pub fn into_listener2(l: &Vec<Listener>) -> Vec<Listener2> {
	l.into_iter()
		.map(|l| Listener2 {
			id: format!("{:?}", l.id),
			name: l.name,
			addrs: l.addrs.clone(),
		})
		.collect()
}
