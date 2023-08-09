use std::{
	collections::VecDeque,
	sync::Arc,
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, Endpoint},
	swarm::{
		derive_prelude::{ConnectionEstablished, ConnectionId, FromSwarm},
		ConnectionClosed, ConnectionDenied, ConnectionHandler, NetworkBehaviour, PollParameters,
		THandler, THandlerInEvent, ToSwarm,
	},
	Multiaddr,
};
use thiserror::Error;
use tracing::debug;

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

/// SpaceTime is a [`NetworkBehaviour`](libp2p_swarm::NetworkBehaviour) that implements the SpaceTime protocol.
/// This protocol sits under the application to abstract many complexities of 2 way connections and deals with authentication, chucking, etc.
pub struct SpaceTime<TMetadata: Metadata> {
	pub(crate) manager: Arc<Manager<TMetadata>>,
	pub(crate) pending_events:
		VecDeque<ToSwarm<<Self as NetworkBehaviour>::OutEvent, THandlerInEvent<Self>>>,
	// For future me's sake, DON't try and refactor this to use shared state (for the nth time), it doesn't fit into libp2p's synchronous trait and polling model!!!
	// pub(crate) connected_peers: HashMap<PeerId, ConnectedPeer>,
}

impl<TMetadata: Metadata> SpaceTime<TMetadata> {
	/// intialise the fabric of space time
	pub fn new(manager: Arc<Manager<TMetadata>>) -> Self {
		Self {
			manager,
			pending_events: VecDeque::new(),
			// connected_peers: HashMap::new(),
		}
	}
}

impl<TMetadata: Metadata> NetworkBehaviour for SpaceTime<TMetadata> {
	type ConnectionHandler = SpaceTimeConnection<TMetadata>;
	type OutEvent = ManagerStreamAction<TMetadata>;

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

	fn handle_pending_outbound_connection(
		&mut self,
		_connection_id: ConnectionId,
		_maybe_peer: Option<libp2p::PeerId>,
		_addresses: &[Multiaddr],
		_effective_role: Endpoint,
	) -> Result<Vec<Multiaddr>, ConnectionDenied> {
		// This should be unused but libp2p still calls it
		Ok(vec![])
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

				// TODO: Move this block onto into `connection.rs` -> will probs be required for the ConnectionEstablishmentPayload stuff
				{
					debug!("sending establishment request to peer '{}'", peer_id);
					if other_established == 0 {
						self.pending_events.push_back(ToSwarm::GenerateEvent(
							ManagerStreamAction::Event(Event::PeerConnected(ConnectedPeer {
								peer_id,
								establisher: match endpoint {
									ConnectedPoint::Dialer { .. } => true,
									ConnectedPoint::Listener { .. } => false,
								},
							})),
						));
					}
				}
			}
			FromSwarm::ConnectionClosed(ConnectionClosed {
				peer_id,
				remaining_established,
				..
			}) => {
				let peer_id = PeerId(peer_id);
				if remaining_established == 0 {
					debug!("Disconnected from peer '{}'", peer_id);
					self.pending_events.push_back(ToSwarm::GenerateEvent(
						ManagerStreamAction::Event(Event::PeerDisconnected(peer_id)),
					));
				}
			}
			FromSwarm::AddressChange(event) => {
				debug!(
					"Address change event: {:?} {:?} {:?} {:?}",
					event.peer_id, event.connection_id, event.old, event.new
				);
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
		event: <SpaceTimeConnection<TMetadata> as ConnectionHandler>::OutEvent,
	) {
		self.pending_events.push_back(ToSwarm::GenerateEvent(event));
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		Poll::Pending
	}
}
