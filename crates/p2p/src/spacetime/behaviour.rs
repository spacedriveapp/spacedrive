use std::{
	collections::VecDeque,
	sync::{Arc, PoisonError},
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, Endpoint},
	swarm::{
		derive_prelude::{ConnectionEstablished, ConnectionId, FromSwarm},
		ConnectionClosed, ConnectionDenied, NetworkBehaviour, PollParameters, THandler,
		THandlerInEvent, THandlerOutEvent, ToSwarm,
	},
	Multiaddr,
};
use thiserror::Error;
use tracing::{debug, trace, warn};

use crate::{Event, Manager, ManagerStreamAction2};

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
pub struct SpaceTime {
	pub(crate) manager: Arc<Manager>,
	pub(crate) pending_events:
		VecDeque<ToSwarm<<Self as NetworkBehaviour>::ToSwarm, THandlerInEvent<Self>>>,
}

impl SpaceTime {
	/// intialise the fabric of space time
	pub fn new(manager: Arc<Manager>) -> Self {
		Self {
			manager,
			pending_events: VecDeque::new(),
		}
	}
}

impl NetworkBehaviour for SpaceTime {
	type ConnectionHandler = SpaceTimeConnection;
	type ToSwarm = ManagerStreamAction2;

	fn handle_established_inbound_connection(
		&mut self,
		_connection_id: ConnectionId,
		peer_id: libp2p::PeerId,
		_local_addr: &Multiaddr,
		_remote_addr: &Multiaddr,
	) -> Result<THandler<Self>, ConnectionDenied> {
		Ok(SpaceTimeConnection::new(peer_id, self.manager.clone()))
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
		Ok(SpaceTimeConnection::new(peer_id, self.manager.clone()))
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
				trace!(
					"connection establishing with peer '{}' found at '{:?}'; peer has {} active connections",
					peer_id, address, other_established
				);
				self.manager
					.state
					.write()
					.unwrap_or_else(PoisonError::into_inner)
					.connections
					.insert(peer_id, (endpoint.clone(), other_established));
			}
			FromSwarm::ConnectionClosed(ConnectionClosed {
				peer_id,
				remaining_established,
				..
			}) => {
				if remaining_established == 0 {
					debug!("Disconnected from peer '{}'", peer_id);
					let mut state = self
						.manager
						.state
						.write()
						.unwrap_or_else(PoisonError::into_inner);

					state.connections.remove(&peer_id);
					if let Some(remote_identity) = state.connected.remove(&peer_id) {
						self.pending_events.push_back(ToSwarm::GenerateEvent(
							Event::PeerDisconnected(remote_identity).into(),
						));
					} else {
						warn!("Disconnected peer '{peer_id}' but was not connected. This likely indicates a bug!");
					}
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
			| FromSwarm::NewExternalAddrCandidate(_)
			| FromSwarm::ExternalAddrConfirmed(_)
			| FromSwarm::ExternalAddrExpired(_) => {}
		}
	}

	fn on_connection_handler_event(
		&mut self,
		_peer_id: libp2p::PeerId,
		_connection: ConnectionId,
		event: THandlerOutEvent<Self>,
	) {
		self.pending_events.push_back(ToSwarm::GenerateEvent(event));
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		Poll::Pending
	}
}
