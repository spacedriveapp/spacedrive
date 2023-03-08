use std::{
	collections::{HashMap, VecDeque},
	sync::Arc,
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, Endpoint},
	swarm::{
		derive_prelude::{ConnectionEstablished, ConnectionId, FromSwarm},
		ConnectionClosed, ConnectionDenied, ConnectionHandler, NetworkBehaviour,
		NetworkBehaviourAction, PollParameters, THandler, THandlerInEvent,
	},
	Multiaddr,
};
use thiserror::Error;
use tracing::{debug, warn};

use crate::{ConnectedPeer, Event, Manager, ManagerStreamAction, Metadata, PeerId};

use super::SpaceTimeConnection;

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
pub const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

// TODO: Remove this?
#[derive(Debug, Error)]
pub enum OutboundFailure {}

/// SpaceTime is a [`NetworkBehaviour`](libp2p::NetworkBehaviour) that implements the SpaceTime protocol.
/// This protocol sits under the application to abstract many complexities of 2 way connections and deals with authentication, chucking, etc.
pub struct SpaceTime<TMetadata: Metadata> {
	pub(crate) manager: Arc<Manager<TMetadata>>,
	pub(crate) pending_events: VecDeque<
		NetworkBehaviourAction<<Self as NetworkBehaviour>::OutEvent, THandlerInEvent<Self>>,
	>,
	// For future me's sake, DON't try and refactor this to use shared state (for the nth time), it doesn't fit into libp2p's synchronous trait and polling model!!!
	pub(crate) connected_peers: HashMap<PeerId, ConnectedPeer>,
}

impl<TMetadata: Metadata> SpaceTime<TMetadata> {
	/// intialise the fabric of space time
	pub fn new(manager: Arc<Manager<TMetadata>>) -> Self {
		Self {
			manager,
			pending_events: VecDeque::new(),
			connected_peers: HashMap::new(),
		}
	}
}

impl<TMetadata: Metadata> NetworkBehaviour for SpaceTime<TMetadata> {
	type ConnectionHandler = SpaceTimeConnection<TMetadata>;
	type OutEvent = ();

	fn handle_established_inbound_connection(
		&mut self,
		_connection_id: ConnectionId,
		peer_id: libp2p::PeerId,
		_local_addr: &Multiaddr,
		_remote_addr: &Multiaddr,
	) -> Result<THandler<Self>, ConnectionDenied> {
		Ok(SpaceTimeConnection::new(
			PeerId(peer_id),
			self.manager.clone(),
		))
	}

	// TODO: Are we even using the response to this??
	// TODO: Do we need to load from state or can be just pass through the `addresses` arg?
	fn handle_pending_outbound_connection(
		&mut self,
		_connection_id: ConnectionId,
		maybe_peer: Option<libp2p::PeerId>,
		_addresses: &[Multiaddr],
		_effective_role: Endpoint,
	) -> Result<Vec<Multiaddr>, ConnectionDenied> {
		if let Some(peer_id) = maybe_peer {
			let mut addresses = Vec::new();
			if let Some(connection) = self.connected_peers.get(&PeerId(peer_id)) {
				addresses.extend(
					connection
						.connections
						.iter()
						.filter_map(|(_, cp)| match cp {
							ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
							ConnectedPoint::Listener {
								local_addr,
								send_back_addr,
							} => {
								println!(
									"TODO: Handle this case! ({} -> {})",
									local_addr, send_back_addr
								);
								todo!();
							}
						}),
				)
			}
			Ok(addresses)
		} else {
			Ok(vec![])
		}
	}

	fn handle_established_outbound_connection(
		&mut self,
		_connection_id: ConnectionId,
		peer_id: libp2p::PeerId,
		_addr: &Multiaddr,
		_role_override: Endpoint,
	) -> Result<THandler<Self>, ConnectionDenied> {
		Ok(SpaceTimeConnection::new(
			PeerId(peer_id),
			self.manager.clone(),
		))
	}

	fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
		match event {
			FromSwarm::ConnectionEstablished(ConnectionEstablished {
				peer_id,
				connection_id,
				endpoint,
				other_established,
				..
			}) => {
				let address = match endpoint {
					ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
					ConnectedPoint::Listener { .. } => None,
				};
				debug!(
					"connection established with peer '{}' found at '{:?}'; peer has {} active connections",
					peer_id, address, other_established
				);

				let peer_id = PeerId(peer_id);
				let endpoint = endpoint.clone();
				let conn = match self.connected_peers.get_mut(&peer_id) {
					Some(peer) => {
						peer.connections.insert(connection_id, endpoint);

						peer.clone()
					}
					None => {
						self.connected_peers.insert(
							peer_id,
							ConnectedPeer {
								peer_id,
								connections: HashMap::from([(connection_id, endpoint)]),
							},
						);
						self.connected_peers
							.get(&peer_id)
							.expect("We legit have a mutable reference")
							.clone()
					}
				};

				// TODO: Move this block onto into `connection.rs` -> will probs be required for the ConnectionEstablishmentPayload stuff
				{
					debug!("sending establishment request to peer '{}'", peer_id);
					if other_established == 0 {
						let manager = self.manager.clone();
						tokio::spawn(async move {
							manager
								.emit(ManagerStreamAction::Event(Event::PeerConnected(conn)))
								.await;
						});
					}
				}
			}
			FromSwarm::ConnectionClosed(ConnectionClosed {
				peer_id,
				connection_id,
				..
			}) => {
				let peer_id = PeerId(peer_id);
				match self.connected_peers.get_mut(&peer_id) {
					Some(peer) => {
						if peer.connections.len() == 1 {
							let conn = self.connected_peers.remove(&peer_id).expect("Literally impossible. We have a mutable reference to it, no shot it's already been removed.");
							debug!("Disconnected from peer '{}'", conn.peer_id);

							let manager = self.manager.clone();
							tokio::spawn(async move {
								manager
									.emit(ManagerStreamAction::Event(Event::PeerDisconnected(
										conn.peer_id.clone(),
									)))
									.await;
							});
						} else {
							peer.connections.remove(&connection_id);
						}
					}
					None => {
						warn!(
                            "Received connection closed event for peer '{}' but no connection was found!",
                            peer_id
                        );
					}
				}
			}
			FromSwarm::AddressChange(_event) => {
				// TODO: Reenable?
				// let new_address = match event.new {
				//     ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
				//     ConnectedPoint::Listener { .. } => None,
				// };
				// let connected = self
				//     .manager
				//     .connections
				//     .blocking_read()
				//     .get_mut(&PeerId(event.peer_id))
				//     .expect("Address change can only happen on an established connection.");

				// let connection = connected
				//     .connections
				//     .iter_mut()
				//     .find(|c| c.id == event.connection_id)
				//     .expect("Address change can only happen on an established connection.");
				// connection.address = new_address;
			}
			FromSwarm::DialFailure(event) => {
				if let Some(peer_id) = event.peer_id {
					debug!("Dialing failure to peer '{}': {:?}", peer_id, event.error);

					// TODO
					// If there are pending outgoing requests when a dial failure occurs,
					// it is implied that we are not connected to the peer, since pending
					// outgoing requests are drained when a connection is established and
					// only created when a peer is not connected when a request is made.
					// Thus these requests must be considered failed, even if there is
					// another, concurrent dialing attempt ongoing.
					// if let Some(pending) = self.pending_outbound_requests.remove(&peer_id) {
					// 	for request in pending {
					// 		self.pending_events
					// 			.push_back(NetworkBehaviourAction::GenerateEvent(
					// 				Event::OutboundFailure {
					// 					peer_id,
					// 					request_id: request.request_id,
					// 					error: OutboundFailure::DialFailure,
					// 				},
					// 			));
					// 	}
					// }
				}
			}
			FromSwarm::ListenFailure(_)
			| FromSwarm::NewListener(_)
			| FromSwarm::NewListenAddr(_)
			| FromSwarm::ExpiredListenAddr(_)
			| FromSwarm::ListenerError(_)
			| FromSwarm::ListenerClosed(_)
			| FromSwarm::NewExternalAddr(_)
			| FromSwarm::ExpiredExternalAddr(_) => {}
		}
	}

	fn on_connection_handler_event(
		&mut self,
		_peer_id: libp2p::PeerId,
		_connection: ConnectionId,
		_event: <SpaceTimeConnection<TMetadata> as ConnectionHandler>::OutEvent,
	) {
		todo!();
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<Self::OutEvent, THandlerInEvent<Self>>> {
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		Poll::Pending
	}
}
