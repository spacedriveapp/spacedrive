use std::{
	collections::{HashMap, HashSet, VecDeque},
	fmt,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, PoisonError,
	},
};

use libp2p::{
	futures::StreamExt,
	swarm::{
		dial_opts::{DialOpts, PeerCondition},
		NotifyHandler, SwarmEvent, ToSwarm,
	},
	PeerId, Swarm,
};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

use crate::{
	quic_multiaddr_to_socketaddr, socketaddr_to_quic_multiaddr,
	spacetime::{OutboundRequest, SpaceTime, UnicastStreamBuilder},
	spacetunnel::RemoteIdentity,
	DiscoveryManager, DynamicManagerState, Event, Manager, ManagerConfig, Mdns,
};

/// TODO
///
/// This is `Sync` so it can be used from within rspc.
pub enum ManagerStreamAction {
	/// TODO
	GetConnectedPeers(oneshot::Sender<Vec<RemoteIdentity>>),
	/// Tell the [`libp2p::Swarm`](libp2p::Swarm) to establish a new connection to a peer.
	Dial {
		peer_id: PeerId,
		addresses: Vec<SocketAddr>,
	},
	/// Update the config. This requires the `libp2p::Swarm`
	UpdateConfig(ManagerConfig),
	/// the node is shutting down. The `ManagerStream` should convert this into `Event::Shutdown`
	Shutdown(oneshot::Sender<()>),
}

/// TODO: Get ride of this and merge into `ManagerStreamAction` without breaking rspc procedures
///
/// This is `!Sync` so can't be used from within rspc.
pub enum ManagerStreamAction2 {
	/// Events are returned to the application via the `ManagerStream::next` method.
	Event(Event),
	/// Events are returned to the application via the `ManagerStream::next` method.
	Events(Vec<Event>),
	/// TODO
	StartStream(PeerId, oneshot::Sender<UnicastStreamBuilder>),
}

impl fmt::Debug for ManagerStreamAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("ManagerStreamAction")
	}
}

impl fmt::Debug for ManagerStreamAction2 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("ManagerStreamAction2")
	}
}

impl From<Event> for ManagerStreamAction2 {
	fn from(event: Event) -> Self {
		Self::Event(event)
	}
}

/// TODO
#[must_use = "streams do nothing unless polled"]
pub struct ManagerStream {
	pub(crate) manager: Arc<Manager>,
	pub(crate) event_stream_rx: mpsc::Receiver<ManagerStreamAction>,
	pub(crate) event_stream_rx2: mpsc::Receiver<ManagerStreamAction2>,
	pub(crate) swarm: Swarm<SpaceTime>,
	pub(crate) discovery_manager: DiscoveryManager,
	pub(crate) queued_events: VecDeque<Event>,
	pub(crate) shutdown: AtomicBool,
	pub(crate) on_establish_streams: HashMap<libp2p::PeerId, Vec<OutboundRequest>>,
}

impl ManagerStream {
	/// Setup the libp2p listeners based on the manager config.
	/// This method will take care of removing old listeners if needed
	pub(crate) fn refresh_listeners(swarm: &mut Swarm<SpaceTime>, state: &mut DynamicManagerState) {
		if state.config.enabled {
			let port = state.config.port.unwrap_or(0);

			if state.ipv4_listener_id.is_none() || matches!(state.ipv6_listener_id, Some(Err(_))) {
				state.ipv4_listener_id = Some(
					swarm
						.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
							Ipv4Addr::UNSPECIFIED,
							port,
						))))
						.map(|id| {
							debug!("registered ipv4 listener: {id:?}");
							id
						})
						.map_err(|err| {
							error!("failed to register ipv4 listener on port {port}: {err}");
							err.to_string()
						}),
				);
			}

			if state.ipv4_listener_id.is_none() || matches!(state.ipv6_listener_id, Some(Err(_))) {
				state.ipv6_listener_id = Some(
					swarm
						.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
							Ipv6Addr::UNSPECIFIED,
							port,
						))))
						.map(|id| {
							debug!("registered ipv6 listener: {id:?}");
							id
						})
						.map_err(|err| {
							error!("failed to register ipv6 listener on port {port}: {err}");
							err.to_string()
						}),
				);
			}
		} else {
			if let Some(Ok(listener)) = state.ipv4_listener_id.take() {
				debug!("removing ipv4 listener with id '{:?}'", listener);
				swarm.remove_listener(listener);
			}

			if let Some(Ok(listener)) = state.ipv6_listener_id.take() {
				debug!("removing ipv6 listener with id '{:?}'", listener);
				swarm.remove_listener(listener);
			}
		}
	}
}

enum EitherManagerStreamAction {
	A(ManagerStreamAction),
	B(ManagerStreamAction2),
}

impl From<ManagerStreamAction> for EitherManagerStreamAction {
	fn from(event: ManagerStreamAction) -> Self {
		Self::A(event)
	}
}

impl From<ManagerStreamAction2> for EitherManagerStreamAction {
	fn from(event: ManagerStreamAction2) -> Self {
		Self::B(event)
	}
}

impl ManagerStream {
	pub fn listen_addrs(&self) -> HashSet<SocketAddr> {
		self.discovery_manager.listen_addrs.clone()
	}

	// Your application should keep polling this until `None` is received or the P2P system will be halted.
	pub async fn next(&mut self) -> Option<Event> {
		// We loop polling internal services until an event comes in that needs to be sent to the parent application.
		loop {
			assert!(!self.shutdown.load(Ordering::Relaxed), "`ManagerStream::next` called after shutdown event. This is a mistake in your application code!");

			if let Some(event) = self.queued_events.pop_front() {
				return Some(event);
			}
			tokio::select! {
				() = self.discovery_manager.poll() => {
					continue;
				},
				event = self.event_stream_rx.recv() => {
					// If the sender has shut down we return `None` to also shut down too.
					if let Some(event) = self.handle_manager_stream_action(event?.into()).await {
						return Some(event);
					}
				}
				event = self.event_stream_rx2.recv() => {
					// If the sender has shut down we return `None` to also shut down too.
					if let Some(event) = self.handle_manager_stream_action(event?.into()).await {
						return Some(event);
					}
				}
				event = self.swarm.select_next_some() => {
					match event {
						SwarmEvent::Behaviour(event) => {
							if let Some(event) = self.handle_manager_stream_action(event.into()).await {
								if let Event::Shutdown { .. } = event {
									self.shutdown.store(true, Ordering::Relaxed);
								}

								return Some(event);
							}
						},
						SwarmEvent::ConnectionEstablished { peer_id, .. } => {
							if let Some(streams) = self.on_establish_streams.remove(&peer_id) {
								for event in streams {
									self.swarm
										.behaviour_mut()
										.pending_events
										.push_back(ToSwarm::NotifyHandler {
											peer_id,
											handler: NotifyHandler::Any,
											event
										});
								}
							}
						},
						SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
							if num_established == 0 {
							let mut state = self.manager.state.write()
								.unwrap_or_else(PoisonError::into_inner);
								if state
									.connected
									.remove(&peer_id).is_none() || state.connections.remove(&peer_id).is_none() {
									   warn!("unable to remove unconnected client from connected map. This indicates a bug!");
								}
							}
						},
						SwarmEvent::IncomingConnection { local_addr, .. } => debug!("incoming connection from '{}'", local_addr),
						SwarmEvent::IncomingConnectionError { local_addr, error, .. } => warn!("handshake error with incoming connection from '{}': {}", local_addr, error),
						SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => warn!("error establishing connection with '{:?}': {}", peer_id, error),
						SwarmEvent::NewListenAddr { listener_id, address, .. } => {
							let addr = match quic_multiaddr_to_socketaddr(address.clone()) {
								Ok(addr) => addr,
								Err(err) => {
									warn!("error passing listen address '{address:?}': {err:?}");
									continue;
								}
							};

							{
								let mut state = self.manager.state.write().unwrap_or_else(PoisonError::into_inner);
								if let Some(Ok(lid)) = &state.ipv4_listener_id {
									if *lid == listener_id {
										state.ipv4_port = Some(addr.port());
									}
								}

								if let Some(Ok(lid)) = &state.ipv6_listener_id {
									if *lid == listener_id {
										state.ipv6_port = Some(addr.port());
									}
								 }
							}

							match quic_multiaddr_to_socketaddr(address) {
								Ok(addr) => {
									trace!("listen address added: {}", addr);
									self.discovery_manager.listen_addrs.insert(addr);
									self.discovery_manager.do_advertisement();
									return Some(Event::AddListenAddr(addr));
								},
								Err(err) => {
									warn!("error passing listen address: {}", err);
									continue;
								}
							}
						},
						SwarmEvent::ExpiredListenAddr { address, .. } => {
							match quic_multiaddr_to_socketaddr(address) {
								Ok(addr) => {
									trace!("listen address expired: {}", addr);
									self.discovery_manager.listen_addrs.remove(&addr);
									self.discovery_manager.do_advertisement();
									return Some(Event::RemoveListenAddr(addr));
								},
								Err(err) => {
									warn!("error passing listen address: {}", err);
									continue;
								}
							}
						}
						SwarmEvent::ListenerClosed { listener_id, addresses, reason } => {
							trace!("listener '{:?}' was closed due to: {:?}", listener_id, reason);
							for address in addresses {
								match quic_multiaddr_to_socketaddr(address) {
									Ok(addr) => {
										trace!("listen address closed: {}", addr);
										self.discovery_manager.listen_addrs.remove(&addr);
										self.queued_events.push_back(Event::RemoveListenAddr(addr));
									},
									Err(err) => {
										warn!("error passing listen address: {}", err);
										continue;
									}
								}
							}

							// The `loop` will restart and begin returning the events from `queued_events`.
						}
						SwarmEvent::ListenerError { listener_id, error } => warn!("listener '{:?}' reported a non-fatal error: {}", listener_id, error),
						SwarmEvent::Dialing { .. } => {},
					}
				}
			}
		}
	}

	async fn handle_manager_stream_action(
		&mut self,
		event: EitherManagerStreamAction,
	) -> Option<Event> {
		match event {
			EitherManagerStreamAction::A(event) => match event {
				ManagerStreamAction::GetConnectedPeers(response) => {
					let result = {
						let state = self
							.manager
							.state
							.read()
							.unwrap_or_else(PoisonError::into_inner);

						self.swarm
							.connected_peers()
							.filter_map(|v| {
								let v = state.connected.get(v);

								if v.is_none() {
									warn!("Error converting PeerId({v:?}) into RemoteIdentity. This is likely a bug in P2P.");
								}

								v.copied()
							})
							.collect::<Vec<_>>()
					};

					response
						.send(result)
						.map_err(|_| {
							error!("Error sending response to `GetConnectedPeers` request! Sending was dropped!");
						})
						.ok();
				}
				ManagerStreamAction::Dial { peer_id, addresses } => {
					match self.swarm.dial(
						DialOpts::peer_id(peer_id)
							.condition(PeerCondition::Disconnected)
							.addresses(addresses.iter().map(socketaddr_to_quic_multiaddr).collect())
							.build(),
					) {
						Ok(()) => {}
						Err(err) => warn!(
							"error dialing peer '{}' with addresses '{:?}': {}",
							peer_id, addresses, err
						),
					}
				}
				ManagerStreamAction::UpdateConfig(config) => {
					let mut state = self
						.manager
						.state
						.write()
						.unwrap_or_else(PoisonError::into_inner);

					state.config = config;
					Self::refresh_listeners(&mut self.swarm, &mut state);

					if !state.config.enabled {
						if let Some(mdns) = self.discovery_manager.mdns.take() {
							drop(state);
							mdns.shutdown();
						}
					} else if self.discovery_manager.mdns.is_none() {
						match Mdns::new(
							self.discovery_manager.application_name,
							self.discovery_manager.identity,
							self.discovery_manager.peer_id,
						) {
							Ok(mdns) => {
								self.discovery_manager.mdns = Some(mdns);
								self.discovery_manager.do_advertisement();
							}
							Err(err) => {
								error!("error starting mDNS service: {err:?}");
								self.discovery_manager.mdns = None;

								// state.config.enabled = false;
								// TODO: Properly reset the UI state cause it will be outa sync
							}
						}
					}

					// drop(state);
				}
				ManagerStreamAction::Shutdown(tx) => {
					info!("Shutting down P2P Manager...");
					self.discovery_manager.shutdown();
					tx.send(()).unwrap_or_else(|()| {
						warn!("Error sending shutdown signal to P2P Manager!");
					});

					return Some(Event::Shutdown);
				}
			},
			EitherManagerStreamAction::B(event) => match event {
				ManagerStreamAction2::Event(event) => return Some(event),
				ManagerStreamAction2::Events(mut events) => {
					let first = events.pop();

					for event in events {
						self.queued_events.push_back(event);
					}

					return first;
				}
				ManagerStreamAction2::StartStream(peer_id, tx) => {
					if !self.swarm.connected_peers().any(|v| *v == peer_id) {
						let Some(addresses) = self
							.discovery_manager
							.state
							.read()
							.unwrap_or_else(PoisonError::into_inner)
							.discovered
							.iter()
							.find_map(|(_, service)| {
								service.iter().find_map(|(_, v)| {
									(v.peer_id == peer_id).then(|| v.addresses.clone())
								})
							})
						else {
							warn!("Peer '{}' is not connected and no addresses are known for it! Skipping connection creation...", peer_id);
							return None;
						};

						match self.swarm.dial(
							DialOpts::peer_id(peer_id)
								.condition(PeerCondition::Disconnected)
								.addresses(
									addresses.iter().map(socketaddr_to_quic_multiaddr).collect(),
								)
								.build(),
						) {
							Ok(()) => {}
							Err(err) => warn!(
								"error dialing peer '{}' with addresses '{:?}': {}",
								peer_id, addresses, err
							),
						}

						self.on_establish_streams
							.entry(peer_id)
							.or_default()
							.push(OutboundRequest::Unicast(tx));
					} else {
						self.swarm.behaviour_mut().pending_events.push_back(
							ToSwarm::NotifyHandler {
								peer_id,
								handler: NotifyHandler::Any,
								event: OutboundRequest::Unicast(tx),
							},
						);
					}
				}
			},
		}

		None
	}
}
