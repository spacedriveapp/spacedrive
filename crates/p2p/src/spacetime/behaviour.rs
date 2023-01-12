use std::{
	collections::{HashMap, HashSet},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, ProtocolName},
	swarm::{
		derive_prelude::{ConnectionId, FromSwarm},
		ConnectionHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
	},
	Multiaddr, PeerId,
};
use smallvec::SmallVec;
use tracing::debug;

use crate::spacetime::{handler::Handler, Event};

use super::{SpaceTimeMessage, SpaceTimeResponseChan};

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

/// This is a vanity type alias so that the code is clearer.
/// As this is only used internally it doesn't need to be a wrapper type.
pub(super) type RequestId = u64;

/// SpaceTime is a [libp2p::NetworkBehaviour] that implements the SpaceTime protocol.
/// This protocol sits under the application to abstract many complexities of 2 way connections and to deal with authentication, chucking, etc.
// #[derive(Clone)]
pub struct SpaceTime {
	request_id: Arc<AtomicU64>, // TODO: Remove `Arc` if removing `Self: Clone`
	outbound_requests: HashMap<u64, Option<Arc<SpaceTimeResponseChan>>>,
	connected: HashMap<PeerId, SmallVec<[Connection; 2]>>,
}

impl ProtocolName for SpaceTime {
	fn protocol_name(&self) -> &[u8] {
		"/spacetime/1".as_bytes()
	}
}

impl SpaceTime {
	/// intialise the fabric of space time
	pub fn new() -> Self {
		Self {
			request_id: Arc::new(AtomicU64::new(0)),
			outbound_requests: HashMap::new(),

			/// TODO: This is effectivly a better `ManagerRef.connected_peers`. Maybe we should storing this once and sharing it between that system and here???
			connected: HashMap::new(),
			// next_request_id: RequestId(1),
			// next_inbound_id: Arc::new(AtomicU64::new(1)),
			// pending_events: VecDeque::new(),
			// pending_outbound_requests: HashMap::new(),
			// addresses: HashMap::new(),
		}
	}

	/// send a message to a single peer.
	/// This will attempt to establish a connection with them if they are not currently connected.
	pub fn send(
		&mut self,
		peer: &PeerId,
		data: SpaceTimeMessage,
		resp: Option<SpaceTimeResponseChan>,
	) {
		let request_id = self.request_id.fetch_add(1, Ordering::Relaxed);
		let resp = resp.map(|v| Arc::new(v)); // Done so that `Self: Clone`
		self.outbound_requests.insert(request_id, resp);

		// 	if let Some(connections) = self.connected.get_mut(peer) {
		// 		if connections.is_empty() {
		// 			return Some(request);
		// 		}
		// 		let ix = (request.request_id.0 as usize) % connections.len();
		// 		let conn = &mut connections[ix];
		// 		conn.pending_inbound_responses.insert(request.request_id);
		// 		self.pending_events
		// 			.push_back(NetworkBehaviourAction::NotifyHandler {
		// 				peer_id: *peer,
		// 				handler: NotifyHandler::One(conn.id),
		// 				event: request,
		// 			});
		// 		None
		// 	} else {
		// 		Some(request)
		// 	}

		// 	let request = RequestProtocol {
		// 		request_id,
		// 		protocols: self.outbound_protocols.clone(),
		// 		request,
		// 	};

		// 	if let Some(request) = self.try_send_request(peer, request) {
		// 		let handler = self.new_handler();
		// 		self.pending_events.push_back(NetworkBehaviourAction::Dial {
		// 			opts: DialOpts::peer_id(*peer).build(),
		// 			handler,
		// 		});
		// 		self.pending_outbound_requests
		// 			.entry(*peer)
		// 			.or_default()
		// 			.push(request);
		// 	}
	}

	// TODO: Expose all these but put them where the hashmap of connected peers ends up

	// /// Adds a known address for a peer that can be used for
	// /// dialing attempts by the `Swarm`, i.e. is returned
	// /// by [`NetworkBehaviour::addresses_of_peer`].
	// ///
	// /// Addresses added in this way are only removed by `remove_address`.
	// pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
	// 	self.addresses.entry(*peer).or_default().push(address);
	// }

	// /// Removes an address of a peer previously added via `add_address`.
	// pub fn remove_address(&mut self, peer: &PeerId, address: &Multiaddr) {
	// 	let mut last = false;
	// 	if let Some(addresses) = self.addresses.get_mut(peer) {
	// 		addresses.retain(|a| a != address);
	// 		last = addresses.is_empty();
	// 	}
	// 	if last {
	// 		self.addresses.remove(peer);
	// 	}
	// }

	// /// Checks whether a peer is currently connected.
	// pub fn is_connected(&self, peer: &PeerId) -> bool {
	// 	if let Some(connections) = self.connected.get(peer) {
	// 		!connections.is_empty()
	// 	} else {
	// 		false
	// 	}
	// }
}

impl NetworkBehaviour for SpaceTime {
	type ConnectionHandler = Handler;
	type OutEvent = Event;

	fn new_handler(&mut self) -> Self::ConnectionHandler {
		Handler::new(
			// self.inbound_protocols.clone(),
			// Duration::from_secs(10),
			// Duration::from_secs(10),
			// self.next_inbound_id.clone(),
		)
	}

	fn addresses_of_peer(&mut self, peer: &PeerId) -> Vec<Multiaddr> {
		let mut addresses = Vec::new();
		if let Some(connections) = self.connected.get(peer) {
			addresses.extend(connections.iter().filter_map(|c| c.address.clone()))
		}
		if let Some(more) = self.addresses.get(peer) {
			addresses.extend(more.into_iter().cloned());
		}
		addresses
	}

	fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
		match event {
			FromSwarm::ConnectionEstablished(event) => {
				let address = match event.endpoint {
					ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
					ConnectedPoint::Listener { .. } => None,
				};
				self.connected
					.entry(event.peer_id)
					.or_default()
					.push(Connection::new(event.connection_id, event.address));

				// TODO
				// if other_established == 0 {
				// 	if let Some(pending) = self.pending_outbound_requests.remove(&peer_id) {
				// 		for request in pending {
				// 			let request = self.try_send_request(&peer_id, request);
				// 			assert!(request.is_none());
				// 		}
				// 	}
				// }
			}
			FromSwarm::ConnectionClosed(event) => {
				let connections = self
					.connected
					.get_mut(&event.peer_id)
					.expect("Expected some established connection to peer before closing.");

				let connection = connections
					.iter()
					.position(|c| c.id == event.connection_id)
					.map(|p: usize| connections.remove(p))
					.expect("Expected connection to be established before closing.");

				debug_assert_eq!(connections.is_empty(), event.remaining_established == 0);
				if connections.is_empty() {
					self.connected.remove(&event.peer_id);
				}

				// TODO
				// for request_id in connection.pending_outbound_responses {
				// 	self.pending_events
				// 		.push_back(NetworkBehaviourAction::GenerateEvent(
				// 			Event::InboundFailure {
				// 				peer: peer_id,
				// 				request_id,
				// 				error: InboundFailure::ConnectionClosed,
				// 			},
				// 		));
				// }

				// TODO
				// for request_id in connection.pending_inbound_responses {
				// 	self.pending_events
				// 		.push_back(NetworkBehaviourAction::GenerateEvent(
				// 			Event::OutboundFailure {
				// 				peer: peer_id,
				// 				request_id,
				// 				error: OutboundFailure::ConnectionClosed,
				// 			},
				// 		));
				// }
			}
			FromSwarm::AddressChange(event) => {
				let new_address = match event.new {
					ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
					ConnectedPoint::Listener { .. } => None,
				};
				let connections = self
					.connected
					.get_mut(&event.peer_id)
					.expect("Address change can only happen on an established connection.");

				let connection = connections
					.iter_mut()
					.find(|c| c.id == event.connection_id)
					.expect("Address change can only happen on an established connection.");
				connection.address = event.new_address;
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
		peer_id: PeerId,
		connection: ConnectionId,
		event: <Handler as ConnectionHandler>::OutEvent,
	) {
		event.handle(peer_id, connection);
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
		// TODO
		// if let Some(ev) = self.pending_events.pop_front() {
		// 	return Poll::Ready(ev);
		// } else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
		// 	self.pending_events.shrink_to_fit();
		// }

		Poll::Pending
	}
}

/// Internal information tracked for an established connection.
struct Connection {
	id: ConnectionId,
	address: Option<Multiaddr>,
	/// Pending outbound responses where corresponding inbound requests have
	/// been received on this connection and emitted via `poll` but have not yet
	/// been answered.
	pending_outbound_responses: HashSet<RequestId>,
	/// Pending inbound responses for previously sent requests on this
	/// connection.
	pending_inbound_responses: HashSet<RequestId>,
}

impl Connection {
	fn new(id: ConnectionId, address: Option<Multiaddr>) -> Self {
		Self {
			id,
			address,
			pending_outbound_responses: Default::default(),
			pending_inbound_responses: Default::default(),
		}
	}
}
