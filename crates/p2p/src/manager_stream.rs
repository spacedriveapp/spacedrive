use std::{
	collections::{HashMap, VecDeque},
	fmt,
	net::SocketAddr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
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
use tracing::{debug, error, info, warn};

use crate::{
	quic_multiaddr_to_socketaddr, socketaddr_to_quic_multiaddr,
	spacetime::{OutboundRequest, SpaceTime, UnicastStream},
	Event, Manager, Mdns, Metadata, PeerId,
};

/// TODO
pub enum ManagerStreamAction<TMetadata: Metadata> {
	/// Events are returned to the application via the `ManagerStream::next` method.
	Event(Event<TMetadata>),
	/// TODO
	GetConnectedPeers(oneshot::Sender<Vec<PeerId>>),
	/// Tell the [`libp2p::Swarm`](libp2p::Swarm) to establish a new connection to a peer.
	Dial {
		peer_id: PeerId,
		addresses: Vec<SocketAddr>,
	},
	/// TODO
	StartStream(PeerId, oneshot::Sender<UnicastStream>),
	/// TODO
	BroadcastData(Vec<u8>),
	/// the node is shutting down. The `ManagerStream` should convert this into `Event::Shutdown`
	Shutdown(oneshot::Sender<()>),
}

impl<TMetadata: Metadata> fmt::Debug for ManagerStreamAction<TMetadata> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("ManagerStreamAction")
	}
}

impl<TMetadata: Metadata> From<Event<TMetadata>> for ManagerStreamAction<TMetadata> {
	fn from(event: Event<TMetadata>) -> Self {
		Self::Event(event)
	}
}

/// TODO
pub struct ManagerStream<TMetadata: Metadata> {
	pub(crate) manager: Arc<Manager<TMetadata>>,
	pub(crate) event_stream_rx: mpsc::Receiver<ManagerStreamAction<TMetadata>>,
	pub(crate) swarm: Swarm<SpaceTime<TMetadata>>,
	pub(crate) mdns: Mdns<TMetadata>,
	pub(crate) queued_events: VecDeque<Event<TMetadata>>,
	pub(crate) shutdown: AtomicBool,
	pub(crate) on_establish_streams: HashMap<libp2p::PeerId, Vec<OutboundRequest>>,
}

impl<TMetadata> ManagerStream<TMetadata>
where
	TMetadata: Metadata,
{
	// Your application should keep polling this until `None` is received or the P2P system will be halted.
	pub async fn next(&mut self) -> Option<Event<TMetadata>> {
		// We loop polling internal services until an event comes in that needs to be sent to the parent application.
		loop {
			if self.shutdown.load(Ordering::Relaxed) {
				panic!("`ManagerStream::next` called after shutdown event. This is a mistake in your application code!");
			}

			if let Some(event) = self.queued_events.pop_front() {
				return Some(event);
			}

			tokio::select! {
				event = self.mdns.poll(&self.manager) => {
					if let Some(event) = event {
						return Some(event);
					}
					continue;
				},
				event = self.event_stream_rx.recv() => {
					// If the sender has shut down we return `None` to also shut down too.
					if let Some(event) = self.handle_manager_stream_action(event?).await {
						return Some(event);
					}
				}
				event = self.swarm.select_next_some() => {
					match event {
						SwarmEvent::Behaviour(event) => {
							if let Some(event) = self.handle_manager_stream_action(event).await {
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
						SwarmEvent::OutgoingConnectionError { peer_id, error } => warn!("error establishing connection with '{:?}': {}", peer_id, error),
						SwarmEvent::NewListenAddr { address, .. } => {
							match quic_multiaddr_to_socketaddr(address) {
								Ok(addr) => {
									debug!("listen address added: {}", addr);
									self.mdns.register_addr(addr).await;
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
									debug!("listen address added: {}", addr);
									self.mdns.unregister_addr(&addr).await;
									return Some(Event::RemoveListenAddr(addr));
								},
								Err(err) => {
									warn!("error passing listen address: {}", err);
									continue;
								}
							}
						}
						SwarmEvent::ListenerClosed { listener_id, addresses, reason } => {
							debug!("listener '{:?}' was closed due to: {:?}", listener_id, reason);
							for address in addresses {
								match quic_multiaddr_to_socketaddr(address) {
									Ok(addr) => {
										debug!("listen address added: {}", addr);
										self.mdns.unregister_addr(&addr).await;

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
						SwarmEvent::Dialing(_peer_id) => {},
						#[allow(deprecated)]
						SwarmEvent::BannedPeer { .. } => {},
					}
				}
			}
		}
	}

	async fn handle_manager_stream_action(
		&mut self,
		event: ManagerStreamAction<TMetadata>,
	) -> Option<Event<TMetadata>> {
		match event {
			ManagerStreamAction::Event(event) => return Some(event),
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
			ManagerStreamAction::StartStream(peer_id, rx) => {
				if !self.swarm.connected_peers().any(|v| *v == peer_id.0) {
					let addresses = self
						.mdns
						.state
						.discovered
						.read()
						.await
						.get(&peer_id)
						.unwrap()
						.addresses
						.clone();

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

					self.on_establish_streams
						.entry(peer_id.0)
						.or_default()
						.push(OutboundRequest::Unicast(rx));
				} else {
					self.swarm
						.behaviour_mut()
						.pending_events
						.push_back(ToSwarm::NotifyHandler {
							peer_id: peer_id.0,
							handler: NotifyHandler::Any,
							event: OutboundRequest::Unicast(rx),
						});
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
			ManagerStreamAction::Shutdown(tx) => {
				info!("Shutting down P2P Manager...");
				self.mdns.shutdown().await;
				tx.send(()).unwrap_or_else(|_| {
					warn!("Error sending shutdown signal to P2P Manager!");
				});

				return Some(Event::Shutdown);
			}
		}

		None
	}
}
