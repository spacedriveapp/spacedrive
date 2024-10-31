use crate::{
	node::{
		config::{self, P2PDiscoveryState},
		HardwareModel,
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
	hooks::{Libp2pPeerId, Mdns, QuicHandle, QuicTransport, RelayServerEntry},
	Peer, RemoteIdentity, UnicastStream, P2P,
};
use sd_p2p_tunnel::Tunnel;
use serde::Serialize;
use serde_json::json;
use specta::Type;
use std::{
	collections::HashMap,
	convert::Infallible,
	sync::{atomic::AtomicBool, Arc, Mutex, PoisonError},
	time::Duration,
};
use tower_service::Service;
use tracing::error;

use tokio::sync::{oneshot, Notify};
use tracing::info;
use uuid::Uuid;

use super::{P2PEvents, PeerMetadata};

#[derive(Default, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum ListenerState {
	Listening,
	Error {
		error: String,
	},
	#[default]
	NotListening,
}

#[derive(Default, Clone, Serialize, Type)]
pub struct Listeners {
	ipv4: ListenerState,
	ipv6: ListenerState,
	relay: ListenerState,
}

pub struct P2PManager {
	pub(crate) p2p: Arc<P2P>,
	mdns: Mutex<Option<Mdns>>,
	quic_transport: QuicTransport,
	pub quic: Arc<QuicHandle>,
	// The `libp2p::PeerId`. This is for debugging only, use `RemoteIdentity` instead.
	lp2p_peer_id: Libp2pPeerId,
	pub(crate) events: P2PEvents,
	pub(super) spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub(super) spacedrop_cancellations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
	pub(crate) node_config: Arc<config::Manager>,
	pub listeners: Mutex<Listeners>,
	relay_config: Mutex<Vec<RelayServerEntry>>,
	trigger_relay_config_update: Notify,
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
		let (quic, lp2p_peer_id) = QuicTransport::spawn(p2p.clone()).map_err(|e| e.to_string())?;
		libraries_hook(p2p.clone(), quic.handle(), libraries);
		let this = Arc::new(Self {
			p2p: p2p.clone(),
			lp2p_peer_id,
			mdns: Mutex::new(None),
			events: P2PEvents::spawn(p2p.clone(), quic.handle()),
			quic: quic.handle(),
			quic_transport: quic,
			spacedrop_pairing_reqs: Default::default(),
			spacedrop_cancellations: Default::default(),
			node_config,
			listeners: Default::default(),
			relay_config: Default::default(),
			trigger_relay_config_update: Default::default(),
		});
		this.on_node_config_change().await;

		info!(
			remote_identity = %this.p2p.remote_identity(),
			peer_id = ?this.lp2p_peer_id,
			addresses = ?this.p2p.listeners(),
			"Node is now online listening;",
		);

		Ok((this.clone(), |node: Arc<Node>, router| {
			tokio::spawn(start(this.clone(), node.clone(), rx, router));

			// TODO: Cleanup this thread on p2p shutdown.
			tokio::spawn(async move {
				let client = reqwest::Client::new();
				loop {
					match client
						// FIXME(@fogodev): hardcoded URL for now as I'm moving stuff around
						.get(format!("{}/api/p2p/relays", "https://app.spacedrive.com"))
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
										node.p2p
											.relay_config
											.lock()
											.unwrap_or_else(PoisonError::into_inner)
											.clone_from(&config);

										let config = {
											let node_config = node.config.get().await;
											if !node_config.p2p.disabled
												&& !node_config.p2p.disable_relay
											{
												config
											} else {
												vec![]
											}
										};
										let no_relays = config.len();

										this.listeners
											.lock()
											.unwrap_or_else(PoisonError::into_inner)
											.relay = match this.quic_transport.set_relay_config(config).await {
											Ok(_) => {
												info!(
													"Updated p2p relay configuration successfully."
												);
												if no_relays == 0 {
													this.quic.disable();

													ListenerState::NotListening
												} else {
													this.quic.enable();

													ListenerState::Listening
												}
											}
											Err(err) => ListenerState::Error {
												error: err.to_string(),
											},
										};
									}
									Err(e) => {
										error!(?e, "Failed to parse p2p relay configuration;")
									}
								}
							}
						}
						Err(e) => error!(?e, "Error pulling p2p relay configuration;"),
					}

					tokio::select! {
						_ = this.trigger_relay_config_update.notified() => {}
						_ = tokio::time::sleep(Duration::from_secs(11 * 60)) => {}
					}
				}
			});
		}))
	}

	pub fn peer_metadata(&self) -> HashMap<String, String> {
		self.p2p.metadata().clone()
	}

	// TODO: Remove this and add a subscription system to `config::Manager`
	pub async fn on_node_config_change(&self) {
		self.trigger_relay_config_update.notify_waiters();

		let config = self.node_config.get().await;

		if config.p2p.discovery == P2PDiscoveryState::ContactsOnly {
			PeerMetadata::remove(&mut self.p2p.metadata_mut());

		// TODO: Hash Spacedrive account ID and put it in the metadata.
		} else {
			PeerMetadata {
				name: config.name.clone(),
				operating_system: Some(OperatingSystem::get_os()),
				device_model: Some(HardwareModel::try_get().unwrap_or(HardwareModel::Other)),
				version: Some(env!("CARGO_PKG_VERSION").to_string()),
			}
			.update(&mut self.p2p.metadata_mut());
		}

		let port = config.p2p.port.get();

		let ipv4_port = (!config.p2p.disabled).then_some(port);
		info!(?ipv4_port, "Setting quic ipv4 listener;");
		self.listeners
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.ipv4 = if let Err(e) = self.quic_transport.set_ipv4_enabled(ipv4_port).await {
			error!(?e, "Failed to enabled quic ipv4 listener;");
			self.node_config
				.write(|c| c.p2p.disabled = false)
				.await
				.ok();

			ListenerState::Error {
				error: e.to_string(),
			}
		} else {
			match !config.p2p.disabled {
				true => ListenerState::Listening,
				false => ListenerState::NotListening,
			}
		};

		let enable_ipv6 = !config.p2p.disabled && !config.p2p.disable_ipv6;
		let ipv6_port = enable_ipv6.then_some(port);
		info!(?ipv6_port, "Setting quic ipv6 listener;");
		self.listeners
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.ipv6 = if let Err(e) = self.quic_transport.set_ipv6_enabled(ipv6_port).await {
			error!(?e, "Failed to enabled quic ipv6 listener;");
			self.node_config
				.write(|c| c.p2p.disable_ipv6 = false)
				.await
				.ok();

			ListenerState::Error {
				error: e.to_string(),
			}
		} else {
			match enable_ipv6 {
				true => ListenerState::Listening,
				false => ListenerState::NotListening,
			}
		};

		self.quic_transport
			.set_manual_peer_addrs(config.p2p.manual_peers);

		let should_revert = match (config.p2p.disabled, config.p2p.discovery) {
			(true, _) | (_, P2PDiscoveryState::Disabled) => {
				let mdns = {
					let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);
					mdns.take()
				};
				if let Some(mdns) = mdns {
					mdns.shutdown().await;
					info!("mDNS shutdown successfully.");
				}

				false
			}
			(_, P2PDiscoveryState::Everyone | P2PDiscoveryState::ContactsOnly) => {
				let mut mdns = self.mdns.lock().unwrap_or_else(PoisonError::into_inner);
				if mdns.is_none() {
					match Mdns::spawn(self.p2p.clone()) {
						Ok(m) => {
							info!("mDNS started successfully.");
							*mdns = Some(m);
							false
						}
						Err(e) => {
							error!(?e, "Failed to start mDNS;");
							true
						}
					}
				} else {
					false
				}
			}
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
				"candidates": p.connection_candidates().iter().map(|a| format!("{a:?}")).collect::<Vec<_>>(),
			})).collect::<Vec<_>>(),
			"hooks": self.p2p.hooks().iter().map(|(id, name)| json!({
				"id": format!("{:?}", id),
				"name": name,
				"listener_addrs": listeners.iter().find(|l| l.is_hook_id(*id)).map(|l| l.addrs.clone()),
			})).collect::<Vec<_>>(),
			"config": node_config.p2p,
			"relay_config": self.quic_transport.get_relay_config(),
			"listeners": self.listeners.lock().unwrap_or_else(PoisonError::into_inner).clone(),
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
			let Ok(header) = Header::from_stream(&mut stream).await.map_err(|e| {
				error!(?e, "Failed to read header from stream;");
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
				Header::Sync => {
					let Ok(mut tunnel) = Tunnel::responder(stream).await.map_err(|e| {
						error!(?e, "Failed `Tunnel::responder`;");
					}) else {
						return;
					};

					let Ok(msg) = SyncMessage::from_stream(&mut tunnel).await.map_err(|e| {
						error!(?e, "Failed `SyncMessage::from_stream`");
					}) else {
						return;
					};

					let Ok(library) = node
						.libraries
						.get_library_for_instance(&tunnel.library_remote_identity())
						.await
						.ok_or_else(|| {
							error!(remove_identity = %tunnel.library_remote_identity(), "Failed to get library;");

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
				Header::RspcRemote => {
					let remote = stream.remote_identity();
					let Err(e) = operations::rspc::receiver(stream, &mut service, &node).await
					else {
						return;
					};

					error!(%remote, ?e, "Failed to handling rspc request;");
				}
				Header::LibraryFile {
					file_path_id,
					range,
				} => {
					let remote = stream.remote_identity();
					let Err(e) =
						operations::library::receiver(stream, file_path_id, range, &node).await
					else {
						return;
					};

					error!(
						?remote,
						%file_path_id,
						?e,
						"Failed to handling library file request;",
					);
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
