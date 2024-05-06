use crate::{
	node::{
		config::{self, P2PDiscoveryState},
		get_hardware_model_name, HardwareModel,
	},
	p2p::{
		libraries::libraries_hook, operations, sync::SyncMessage, Header, OperatingSystem,
		SPACEDRIVE_APP_ID,
	},
	Node,
};

use axum::routing::IntoMakeService;

use sd_p2p::{
	flume::{bounded, Receiver},
	HookId, Libp2pPeerId, Mdns, Peer, QuicTransport, RelayServerEntry, RemoteIdentity,
	UnicastStream, P2P,
};
use sd_p2p_tunnel::Tunnel;
use serde_json::json;
use std::{
	collections::HashMap,
	convert::Infallible,
	sync::{atomic::AtomicBool, Arc, Mutex, PoisonError},
	time::Duration,
};
use tower_service::Service;
use tracing::error;

use tokio::sync::oneshot;
use tracing::info;
use uuid::Uuid;

use super::{P2PEvents, PeerMetadata};

#[derive(Default)]
pub struct ListenerErrors {
	pub ipv4: Option<String>,
	pub ipv6: Option<String>,
}

pub struct P2PManager {
	pub(crate) p2p: Arc<P2P>,
	mdns: Mutex<Option<Mdns>>,
	quic: QuicTransport,
	// The `libp2p::PeerId`. This is for debugging only, use `RemoteIdentity` instead.
	lp2p_peer_id: Libp2pPeerId,
	pub(crate) events: P2PEvents,
	pub(super) spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancellations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	pub(crate) node_config: Arc<config::Manager>,
	pub libraries_hook_id: HookId,
	pub listener_errors: Mutex<ListenerErrors>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<config::Manager>,
		libraries: Arc<crate::library::Libraries>,
	) -> Result<
		(
			Arc<P2PManager>,
			impl FnOnce(Arc<Node>, IntoMakeService<axum::Router<()>>),
		),
		String,
	> {
		let (tx, rx) = bounded(25);
		let p2p = P2P::new(SPACEDRIVE_APP_ID, node_config.get().await.identity, tx);
		let (quic, lp2p_peer_id) = QuicTransport::spawn(p2p.clone())?;
		let libraries_hook_id = libraries_hook(p2p.clone(), libraries);
		let this = Arc::new(Self {
			p2p: p2p.clone(),
			lp2p_peer_id,
			mdns: Mutex::new(None),
			quic,
			events: P2PEvents::spawn(p2p.clone(), libraries_hook_id),
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancellations: Default::default(),
			node_config,
			libraries_hook_id,
			listener_errors: Default::default(),
		});
		this.on_node_config_change().await;

		info!(
			"Node RemoteIdentity('{}') libp2p::PeerId('{:?}') is now online listening at addresses: {:?}",
			this.p2p.remote_identity(),
			this.lp2p_peer_id,
			this.p2p.listeners()
		);

		Ok((this.clone(), |node: Arc<Node>, router| {
			tokio::spawn(start(this.clone(), node.clone(), rx, router));

			// TODO: Cleanup this thread on p2p shutdown.
			tokio::spawn(async move {
				let client = reqwest::Client::new();
				loop {
					match client
						.get(format!("{}/api/p2p/relays", node.env.api_url.lock().await))
						.send()
						.await
					{
						Ok(resp) => {
							if resp.status() != 200 {
								error!(
									"Failed to pull p2p relay configuration: {} {:?}",
									resp.status(),
									resp.text().await
								);
							} else {
								match resp.json::<Vec<RelayServerEntry>>().await {
									Ok(config) => {
										this.quic.set_relay_config(config).await;
										info!("Updated p2p relay configuration successfully.")
									}
									Err(err) => {
										error!("Failed to parse p2p relay configuration: {err:?}")
									}
								}
							}
						}
						Err(err) => error!("Error pulling p2p relay configuration: {err:?}"),
					}

					tokio::time::sleep(Duration::from_secs(11 * 60)).await;
				}
			});
		}))
	}

	pub fn peer_metadata(&self) -> HashMap<String, String> {
		self.p2p.metadata().clone()
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

		let port = config.p2p.port.get();

		info!(
			"Setting quic ipv4 listener to: {:?}",
			config.p2p.ipv4.then_some(port)
		);
		if let Err(err) = self
			.quic
			.set_ipv4_enabled(config.p2p.ipv4.then_some(port))
			.await
		{
			error!("Failed to enabled quic ipv4 listener: {err}");
			self.node_config.write(|c| c.p2p.ipv4 = false).await.ok();

			self.listener_errors
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.ipv4 = Some(err.to_string());
		}

		info!(
			"Setting quic ipv6 listener to: {:?}",
			config.p2p.ipv6.then_some(port)
		);
		if let Err(err) = self
			.quic
			.set_ipv6_enabled(config.p2p.ipv6.then_some(port))
			.await
		{
			error!("Failed to enabled quic ipv6 listener: {err}");
			self.node_config.write(|c| c.p2p.ipv6 = false).await.ok();

			self.listener_errors
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.ipv6 = Some(err.to_string());
		}

		let should_revert = match config.p2p.discovery {
			P2PDiscoveryState::Everyone
			// TODO: Make `ContactsOnly` work
			| P2PDiscoveryState::ContactsOnly => {
				let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);
				if mdns.is_none() {
					match Mdns::spawn(self.p2p.clone()) {
						Ok(m) => {
							info!("mDNS started successfully.");
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
				let mdns = {
					let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);
					mdns.take()
				};
				if let Some(mdns) = mdns {
					mdns.shutdown().await;
					info!("mDNS shutdown successfully.");
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
				.write(|c| c.p2p.discovery = P2PDiscoveryState::Disabled)
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

	pub async fn state(&self) -> serde_json::Value {
		let listeners = self.p2p.listeners();
		let node_config = self.node_config.get().await;
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
			"config": node_config.p2p,
			"relay_config": self.quic.get_relay_config(),
		})
	}

	pub async fn shutdown(&self) {
		// `self.p2p` will automatically take care of shutting down all the hooks. Eg. `self.quic`, `self.mdns`, etc.
		self.p2p.shutdown().await;
	}
}

async fn start(
	this: Arc<P2PManager>,
	node: Arc<Node>,
	rx: Receiver<UnicastStream>,
	mut service: IntoMakeService<axum::Router<()>>,
) -> Result<(), ()> {
	while let Ok(mut stream) = rx.recv_async().await {
		let this = this.clone();
		let node = node.clone();
		let mut service = unwrap_infallible(service.call(()).await);

		tokio::spawn(async move {
			let Ok(header) = Header::from_stream(&mut stream).await.map_err(|err| {
				error!("Failed to read header from stream: {}", err);
			}) else {
				return;
			};

			match header {
				Header::Ping => operations::ping::receiver(stream).await,
				Header::Spacedrop(req) => {
					let Err(()) = operations::spacedrop::receiver(&this, req, stream).await else {
						return;
					};

					error!("Failed to handle Spacedrop request");
				}
				Header::Sync(library_id) => {
					let Ok(mut tunnel) = Tunnel::responder(stream).await.map_err(|err| {
						error!("Failed `Tunnel::responder`: {}", err);
					}) else {
						return;
					};

					let Ok(msg) = SyncMessage::from_stream(&mut tunnel).await.map_err(|err| {
						error!("Failed `SyncMessage::from_stream`: {}", err);
					}) else {
						return;
					};

					let Ok(library) =
						node.libraries
							.get_library(&library_id)
							.await
							.ok_or_else(|| {
								error!("Failed to get library '{library_id}'");

								// TODO: Respond to remote client with warning!
							})
					else {
						return;
					};

					match msg {
						SyncMessage::NewOperations => {
							let Err(()) = super::sync::responder(&mut tunnel, library).await else {
								return;
							};

							error!("Failed to handle sync responder request");
						}
					};
				}
				Header::Http => {
					let remote = stream.remote_identity();
					let Err(err) = operations::rspc::receiver(stream, &mut service, &node).await
					else {
						return;
					};

					error!("Failed to handling rspc request with '{remote}': {err:?}");
				}
			};
		});
	}

	Ok::<_, ()>(())
}

fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
	match result {
		Ok(value) => value,
		Err(err) => match err {},
	}
}
