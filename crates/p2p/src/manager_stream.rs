use std::{
	collections::{HashMap, HashSet, VecDeque},
	fmt,
	future::poll_fn,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, PoisonError,
	},
	task::Poll,
};

use libp2p::{
	futures::StreamExt,
	swarm::{
		dial_opts::{DialOpts, PeerCondition},
		NotifyHandler, SwarmEvent, ToSwarm,
	},
	Swarm,
};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

use crate::{
	quic_multiaddr_to_socketaddr, socketaddr_to_quic_multiaddr,
	spacetime::{OutboundRequest, SpaceTime, UnicastStream},
	DiscoveryManager, DynamicManagerState, Event, Manager, ManagerConfig, Metadata, PeerId,
};

/// TODO
///
/// This is `Sync` so it can be used from within rspc.
pub enum ManagerStreamAction {
	/// TODO
	GetConnectedPeers(oneshot::Sender<Vec<PeerId>>),
	/// Tell the [`libp2p::Swarm`](libp2p::Swarm) to establish a new connection to a peer.
	Dial {
		peer_id: PeerId,
		addresses: Vec<SocketAddr>,
	},
	/// TODO
	BroadcastData(Vec<u8>),
	/// Update the config. This requires the `libp2p::Swarm`
	UpdateConfig(ManagerConfig),
	/// the node is shutting down. The `ManagerStream` should convert this into `Event::Shutdown`
	Shutdown(oneshot::Sender<()>),
}

/// TODO: Get ride of this and merge into `ManagerStreamAction` without breaking rspc procedures
///
/// This is `!Sync` so can't be used from within rspc.
pub enum ManagerStreamAction2<TMeta: Metadata> {
	/// Events are returned to the application via the `ManagerStream::next` method.
	Event(Event<TMeta>),
	/// TODO
	StartStream(PeerId, oneshot::Sender<UnicastStream>),
}

impl fmt::Debug for ManagerStreamAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("ManagerStreamAction")
	}
}

impl<TMeta: Metadata> fmt::Debug for ManagerStreamAction2<TMeta> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("ManagerStreamAction2")
	}
}

impl<TMeta: Metadata> From<Event<TMeta>> for ManagerStreamAction2<TMeta> {
	fn from(event: Event<TMeta>) -> Self {
		Self::Event(event)
	}
}

/// TODO
pub struct ManagerStream<TMeta: Metadata> {
	pub(crate) manager: Arc<Manager<TMeta>>,
	pub(crate) event_stream_rx: mpsc::Receiver<ManagerStreamAction>,
	pub(crate) event_stream_rx2: mpsc::Receiver<ManagerStreamAction2<TMeta>>,
	pub(crate) swarm: Swarm<SpaceTime<TMeta>>,
	pub(crate) discovery_manager: DiscoveryManager,
	pub(crate) queued_events: VecDeque<Event<TMeta>>,
	pub(crate) shutdown: AtomicBool,
	pub(crate) on_establish_streams: HashMap<libp2p::PeerId, Vec<OutboundRequest>>,
}

impl<TMeta: Metadata> ManagerStream<TMeta> {
	/// Setup the libp2p listeners based on the manager config.
	/// This method will take care of removing old listeners if needed
	pub(crate) fn refresh_listeners(
		swarm: &mut Swarm<SpaceTime<TMeta>>,
		state: &mut DynamicManagerState,
	) {
		if state.config.enabled {
			let port = state.config.port.unwrap_or(0);

			if state.ipv4_listener_id.is_none() {
				match swarm.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
					Ipv4Addr::UNSPECIFIED,
					port,
				)))) {
					Ok(listener_id) => {
						debug!("created ipv4 listener with id '{:?}'", listener_id);
						state.ipv4_listener_id = Some(listener_id);
					}
					Err(err) => error!("failed to listener on '0.0.0.0:{port}': {err}"),
				};
			}

			if state.ipv6_listener_id.is_none() {
				match swarm.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
					Ipv6Addr::UNSPECIFIED,
					port,
				)))) {
					Ok(listener_id) => {
						debug!("created ipv6 listener with id '{:?}'", listener_id);
						state.ipv6_listener_id = Some(listener_id);
					}
					Err(err) => error!("failed to listener on '[::]:{port}': {err}"),
				};
			}
		} else {
			if let Some(listener) = state.ipv4_listener_id.take() {
				debug!("removing ipv4 listener with id '{:?}'", listener);
				swarm.remove_listener(listener);
			}

			if let Some(listener) = state.ipv6_listener_id.take() {
				debug!("removing ipv6 listener with id '{:?}'", listener);
				swarm.remove_listener(listener);
			}
		}
	}
}

enum EitherManagerStreamAction<TMeta: Metadata> {
	A(ManagerStreamAction),
	B(ManagerStreamAction2<TMeta>),
}

impl<TMeta: Metadata> From<ManagerStreamAction> for EitherManagerStreamAction<TMeta> {
	fn from(event: ManagerStreamAction) -> Self {
		Self::A(event)
	}
}

impl<TMeta: Metadata> From<ManagerStreamAction2<TMeta>> for EitherManagerStreamAction<TMeta> {
	fn from(event: ManagerStreamAction2<TMeta>) -> Self {
		Self::B(event)
	}
}

impl<TMeta> ManagerStream<TMeta>
where
	TMeta: Metadata,
{
	pub fn listen_addrs(&self) -> HashSet<SocketAddr> {
		self.discovery_manager.listen_addrs.clone()
	}

	// Your application should keep polling this until `None` is received or the P2P system will be halted.
	pub async fn next(&mut self) -> Option<Event<TMeta>> {
		// We loop polling internal services until an event comes in that needs to be sent to the parent application.
		loop {
			if self.shutdown.load(Ordering::Relaxed) {
				panic!("`ManagerStream::next` called after shutdown event. This is a mistake in your application code!");
			}

			if let Some(event) = self.queued_events.pop_front() {
				return Some(event);
			}

			tokio::select! {
				event = async {
					// if let Some(mdns) = &mut self.mdns {
					// 	mdns.poll(&self.manager).await
					// } else {
					//    pending().await
					// }
				} => {
					// if let Some(event) = event {
					// 	return Some(event);
					// }
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
						SwarmEvent::ConnectionClosed { .. } => {},
						SwarmEvent::IncomingConnection { local_addr, .. } => debug!("incoming connection from '{}'", local_addr),
						SwarmEvent::IncomingConnectionError { local_addr, error, .. } => warn!("handshake error with incoming connection from '{}': {}", local_addr, error),
						SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => warn!("error establishing connection with '{:?}': {}", peer_id, error),
						SwarmEvent::NewListenAddr { address, .. } => {
							match quic_multiaddr_to_socketaddr(address) {
								Ok(addr) => {
									trace!("listen address added: {}", addr);
									self.discovery_manager.register_addr(addr).await;
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
									self.discovery_manager.unregister_addr(&addr).await;
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
										self.discovery_manager.unregister_addr(&addr).await;
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
		event: EitherManagerStreamAction<TMeta>,
	) -> Option<Event<TMeta>> {
		match event {
			EitherManagerStreamAction::A(event) => match event {
				ManagerStreamAction::GetConnectedPeers(response) => {
					response
						.send(
							self.swarm
								.connected_peers()
								.map(|v| PeerId(*v))
								.collect::<Vec<_>>(),
						)
						.map_err(|_| {
							error!("Error sending response to `GetConnectedPeers` request! Sending was dropped!")
						})
						.ok();
				}
				ManagerStreamAction::Dial { peer_id, addresses } => {
					match self.swarm.dial(
						DialOpts::peer_id(peer_id.0)
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
				ManagerStreamAction::BroadcastData(data) => {
					let connected_peers = self.swarm.connected_peers().copied().collect::<Vec<_>>();
					let behaviour = self.swarm.behaviour_mut();
					debug!("Broadcasting message to '{:?}'", connected_peers);
					for peer_id in connected_peers {
						behaviour.pending_events.push_back(ToSwarm::NotifyHandler {
							peer_id,
							handler: NotifyHandler::Any,
							event: OutboundRequest::Broadcast(data.clone()),
						});
					}
				}
				ManagerStreamAction::UpdateConfig(config) => {
					let mut state = self
						.manager
						.state
						.write()
						.unwrap_or_else(PoisonError::into_inner);

					state.config = config;
					ManagerStream::refresh_listeners(&mut self.swarm, &mut state);

					if !state.config.enabled {
						// if let Some(mdns) = self.discovery_manager.take() {
						// 	drop(state);
						// 	mdns.shutdown().await;
						// }
					} else {
						// if self.mdns.is_none() {
						// 	let mdns = Mdns::new().await;
						// 	self.mdns = Some(mdns);
						// }
						todo!();
					}

					// drop(state);
				}
				ManagerStreamAction::Shutdown(tx) => {
					info!("Shutting down P2P Manager...");
					// if let Some(mdns) = &self.mdns {
					// 	mdns.shutdown().await;
					// }
					tx.send(()).unwrap_or_else(|_| {
						warn!("Error sending shutdown signal to P2P Manager!");
					});

					return Some(Event::Shutdown);
				}
			},
			EitherManagerStreamAction::B(event) => match event {
				ManagerStreamAction2::Event(event) => return Some(event),
				ManagerStreamAction2::StartStream(peer_id, tx) => {
					if !self.swarm.connected_peers().any(|v| *v == peer_id.0) {
						// let addresses = self
						// 	.mdns
						// 	.as_mut()
						// 	.unwrap() // TODO: Error handling
						// 	.state
						// 	.discovered
						// 	.read()
						// 	.await
						// 	.get(&peer_id)
						// 	.unwrap()
						// 	.addresses
						// 	.clone();
						let addresses: Vec<SocketAddr> = todo!();

						match self.swarm.dial(
							DialOpts::peer_id(peer_id.0)
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
							.entry(peer_id.0)
							.or_default()
							.push(OutboundRequest::Unicast(tx));
					} else {
						self.swarm.behaviour_mut().pending_events.push_back(
							ToSwarm::NotifyHandler {
								peer_id: peer_id.0,
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

async fn pending() -> ! {
	poll_fn(|_| Poll::Pending).await
}
