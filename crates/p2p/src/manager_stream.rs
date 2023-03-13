use std::{
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
		NetworkBehaviourAction, NotifyHandler, SwarmEvent,
	},
	Multiaddr, Swarm,
};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::{
	quic_multiaddr_to_socketaddr, socketaddr_to_quic_multiaddr,
	spacetime::{OutboundRequest, SpaceTime, UnicastStream},
	AsyncFn, Event, Manager, Mdns, Metadata, PeerId,
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
}

impl<TMetadata: Metadata> From<Event<TMetadata>> for ManagerStreamAction<TMetadata> {
	fn from(event: Event<TMetadata>) -> Self {
		Self::Event(event)
	}
}

/// TODO
pub struct ManagerStream<TMetadata, TMetadataFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
{
	pub(crate) manager: Arc<Manager<TMetadata>>,
	pub(crate) event_stream_rx: mpsc::Receiver<ManagerStreamAction<TMetadata>>,
	pub(crate) swarm: Swarm<SpaceTime<TMetadata>>,
	pub(crate) mdns: Mdns<TMetadata, TMetadataFn>,
	pub(crate) is_advertisement_queued: AtomicBool,
}

impl<TMetadata, TMetadataFn> ManagerStream<TMetadata, TMetadataFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
{
	// Your application should keep polling this until `None` is received or the P2P system will be halted.
	pub async fn next(&mut self) -> Option<Event<TMetadata>> {
		// We loop polling internal services until an event comes in that needs to be sent to the parent application.
		loop {
			tokio::select! {
				_ = self.mdns.poll() => {},
				event = self.event_stream_rx.recv() => {
					// If the sender has shut down we return `None` to also shut down too.
					match event? {
						ManagerStreamAction::Event(event) => return Some(event),
						ManagerStreamAction::GetConnectedPeers(response) => {
							response.send(self.swarm.connected_peers().map(|v| PeerId(*v)).collect::<Vec<_>>()).map_err(|_| error!("Error sending response to `GetConnectedPeers` request! Sending was dropped!")).ok();
						},
						ManagerStreamAction::Dial { peer_id, addresses } => {
							match self.swarm.dial(
								DialOpts::peer_id(peer_id.0)
									.condition(PeerCondition::Disconnected)
									.addresses(
										addresses
											.iter()
											.map(socketaddr_to_quic_multiaddr)
											.collect(),
									)
									.build(),
							) {
								Ok(_) => {}
								Err(err) => warn!(
									"error dialing peer '{}' with addresses '{:?}': {}",
									peer_id, addresses, err
								),
							}
						}
						ManagerStreamAction::StartStream(peer_id, rx) => {
							self.swarm.behaviour_mut().pending_events
								.push_back(NetworkBehaviourAction::NotifyHandler {
									peer_id: peer_id.0,
									handler: NotifyHandler::Any,
									event: OutboundRequest::Unicast(rx),
								});
						}
						ManagerStreamAction::BroadcastData(data) => {
							let connected_peers = self.swarm.connected_peers().map(|v| *v).collect::<Vec<_>>();
							let behaviour = self.swarm.behaviour_mut();
							for peer_id in connected_peers {
								behaviour.pending_events
									.push_back(NetworkBehaviourAction::NotifyHandler {
										peer_id: peer_id,
										handler: NotifyHandler::Any,
										event: OutboundRequest::Broadcast(data.clone()),
									});
							}
						}
					}
				}
				event = self.swarm.select_next_some() => {
					match event {
						SwarmEvent::Behaviour(event) => {
							return Some(event);
						},
						SwarmEvent::ConnectionEstablished { .. } => {},
						SwarmEvent::ConnectionClosed { .. } => {},
						SwarmEvent::IncomingConnection { local_addr, .. } => debug!("incoming connection from '{}'", local_addr),
						SwarmEvent::IncomingConnectionError { local_addr, error, .. } => warn!("handshake error with incoming connection from '{}': {}", local_addr, error),
						SwarmEvent::OutgoingConnectionError { peer_id, error } => warn!("error establishing connection with '{:?}': {}", peer_id, error),
						SwarmEvent::BannedPeer { peer_id, .. } => warn!("banned peer '{}' attempted to connection and was rejected", peer_id),
						SwarmEvent::NewListenAddr{ address, .. } => {
							match quic_multiaddr_to_socketaddr(address) {
								Ok(addr) => {
									debug!("listen address added: {}", addr);
									self.manager.listen_addrs.write().await.insert(addr);
									if !self.is_advertisement_queued.load(Ordering::Relaxed) {
										self.is_advertisement_queued.store(true, Ordering::Relaxed);
										self.mdns.advertise();
									}
									self.manager.emit(Event::AddListenAddr(addr).into()).await;
									self.mdns.advertise();
								},
								Err(err) => {
									warn!("error passing listen address: {}", err);
									continue;
								}
							}
						},
						SwarmEvent::ExpiredListenAddr { address, .. } => {
							match Self::unregister_addr(&self.manager, &self.mdns, &self.is_advertisement_queued, address).await {
								Ok(_) => {},
								Err(err) => {
									warn!("error passing listen address: {}", err);
									continue;
								}
							}
						}
						SwarmEvent::ListenerClosed { listener_id, addresses, reason } => {
							debug!("listener '{:?}' was closed due to: {:?}", listener_id, reason);
							for address in addresses {
								match Self::unregister_addr(&self.manager, &self.mdns, &self.is_advertisement_queued, address).await {
									Ok(_) => {},
									Err(err) => {
										warn!("error passing listen address: {}", err);
										continue;
									}
								}
							}
						}
						SwarmEvent::ListenerError { listener_id, error } => warn!("listener '{:?}' reported a non-fatal error: {}", listener_id, error),
						SwarmEvent::Dialing(_peer_id) => {},
					}
				}
			}
		}
	}

	async fn unregister_addr(
		manager: &Arc<Manager<TMetadata>>,
		mdns: &Mdns<TMetadata, TMetadataFn>,
		is_advertisement_queued: &AtomicBool,
		address: Multiaddr,
	) -> Result<(), String> {
		match quic_multiaddr_to_socketaddr(address) {
			Ok(addr) => {
				debug!("listen address removed: {}", addr);
				manager.listen_addrs.write().await.remove(&addr);
				let _ = mdns.unregister_mdns();
				if !is_advertisement_queued.load(Ordering::Relaxed) {
					is_advertisement_queued.store(true, Ordering::Relaxed);
					mdns.advertise();
				}
				manager
					.emit(ManagerStreamAction::Event(Event::RemoveListenAddr(addr)))
					.await;
				Ok(())
			}
			Err(err) => Err(err),
		}
	}
}
