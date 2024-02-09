use std::{
	collections::{HashMap, VecDeque},
	sync::{
		atomic::{AtomicU64, AtomicUsize},
		Arc, Mutex, PoisonError,
	},
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, Endpoint},
	swarm::{
		derive_prelude::{ConnectionEstablished, ConnectionId, FromSwarm},
		ConnectionClosed, ConnectionDenied, NetworkBehaviour, THandler, THandlerInEvent,
		THandlerOutEvent, ToSwarm,
	},
	Multiaddr,
};
use thiserror::Error;
use tracing::{debug, trace, warn};

use crate::{ConnectionRequest, ListenerId, P2P};

use super::connection::SpaceTimeConnection;

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
pub const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

// TODO: Remove this?
#[derive(Debug, Error)]
pub enum OutboundFailure {}

pub(crate) struct SpaceTimeState {
	pub p2p: Arc<P2P>,
	pub listener_id: ListenerId,
	pub stream_id: Arc<AtomicU64>,
	// A list of the `new_stream` callbacks that are waiting for a connection to be established.
	// Once established, the outbound protocol can return the `UnicastStream` to the user.
	pub establishing_outbound: Mutex<HashMap<ConnectionId, ConnectionRequest>>,
}

/// `SpaceTime` is a [`NetworkBehaviour`](libp2p_swarm::NetworkBehaviour) that implements the `SpaceTime` protocol.
/// This protocol sits under the application to abstract many complexities of 2 way connections and deals with authentication, chucking, etc.
pub struct SpaceTime {
	pub(crate) state: Arc<SpaceTimeState>,
	pub(crate) pending_events:
		VecDeque<ToSwarm<<Self as NetworkBehaviour>::ToSwarm, THandlerInEvent<Self>>>,
}

impl SpaceTime {
	/// intialise the fabric of space time
	pub fn new(p2p: Arc<P2P>, listener_id: ListenerId) -> Self {
		Self {
			state: Arc::new(SpaceTimeState {
				p2p,
				listener_id,
				stream_id: Default::default(),
				establishing_outbound: Default::default(),
			}),
			pending_events: VecDeque::new(),
		}
	}
}

impl NetworkBehaviour for SpaceTime {
	type ConnectionHandler = SpaceTimeConnection;
	type ToSwarm = (); // TODO: ManagerStreamAction2

	fn handle_established_inbound_connection(
		&mut self,
		connection_id: ConnectionId,
		peer_id: libp2p::PeerId,
		_local_addr: &Multiaddr,
		_remote_addr: &Multiaddr,
	) -> Result<THandler<Self>, ConnectionDenied> {
		Ok(SpaceTimeConnection::new(
			connection_id,
			peer_id,
			self.state.clone(),
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
		connection_id: ConnectionId,
		peer_id: libp2p::PeerId,
		_addr: &Multiaddr,
		_role_override: Endpoint,
	) -> Result<THandler<Self>, ConnectionDenied> {
		Ok(SpaceTimeConnection::new(
			connection_id,
			peer_id,
			self.state.clone(),
		))
	}

	fn on_swarm_event(&mut self, event: FromSwarm) {
		match event {
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
			_ => {}
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

	fn poll(&mut self, _: &mut Context<'_>) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		Poll::Pending
	}
}
