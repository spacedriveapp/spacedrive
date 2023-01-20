use std::{
	collections::{HashMap, HashSet, VecDeque},
	marker::PhantomData,
	sync::Arc,
	task::{Context, Poll},
};

use libp2p::{
	core::{ConnectedPoint, ProtocolName},
	swarm::{
		derive_prelude::{ConnectionEstablished, ConnectionId, FromSwarm},
		ConnectionHandler, NetworkBehaviour, NetworkBehaviourAction, NotifyHandler, PollParameters,
	},
	Multiaddr,
};
use smallvec::SmallVec;
use tokio::sync::oneshot;
use tracing::debug;

use crate::{spacetime::handler::Handler, utils::AsyncFn2, ManagerRef, PeerId};

use super::{OutboundFailure, SpaceTimeMessage};

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
pub const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

/// TODO
pub type SpaceTimeResponseChan = oneshot::Sender<Result<SpaceTimeMessage, OutboundFailure>>;

/// This is a vanity type alias so that the code is clearer.
/// As this is only used internally it doesn't need to be a wrapper type.
pub(super) type RequestId = u64;

// TODO: Maybe remove this once the data ownership structure is clearer
pub struct SpaceTimeState<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	// TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, crate::Event<TMetadata>, Output = ()>,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	pub(crate) manager: Arc<ManagerRef<TMetadata>>,
	pub(crate) fn_on_connect: Arc<TConnFn>,
	phantom: PhantomData<TMetadata>,
}

/// SpaceTime is a [libp2p::NetworkBehaviour] that implements the SpaceTime protocol.
/// This protocol sits under the application to abstract many complexities of 2 way connections and to deal with authentication, chucking, etc.
pub struct SpaceTime<TMetadata, TEventFn, TConnFn>
where
	TMetadata: crate::Metadata,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, crate::Event<TMetadata>, Output = ()>,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	state: Arc<SpaceTimeState<TMetadata, TConnFn>>,
	fn_on_event: Arc<TEventFn>, // TODO: Should be able to remove arc???? Closure may need clone but it would be two clones on startup so fine.
	pending_events: VecDeque<
		NetworkBehaviourAction<<Self as NetworkBehaviour>::OutEvent, Handler<TMetadata, TConnFn>>,
	>,
	connected: HashMap<PeerId, SmallVec<[Connection; 2]>>,
	addresses: HashMap<PeerId, SmallVec<[Multiaddr; 6]>>,
}

impl<TMetadata, TEventFn, TConnFn> ProtocolName for SpaceTime<TMetadata, TEventFn, TConnFn>
where
	TMetadata: crate::Metadata,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, crate::Event<TMetadata>, Output = ()>,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	fn protocol_name(&self) -> &[u8] {
		b"/spacetime/1"
	}
}

impl<TMetadata, TEventFn, TConnFn> SpaceTime<TMetadata, TEventFn, TConnFn>
where
	TMetadata: crate::Metadata,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, crate::Event<TMetadata>, Output = ()>,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	/// intialise the fabric of space time
	pub fn new(
		fn_on_connect: Arc<TConnFn>,
		manager: Arc<ManagerRef<TMetadata>>,
		fn_on_event: Arc<TEventFn>,
	) -> Self {
		Self {
			state: Arc::new(SpaceTimeState {
				manager,
				fn_on_connect,
				phantom: PhantomData,
			}),
			fn_on_event,
			pending_events: VecDeque::new(),
			connected: HashMap::new(),
			addresses: HashMap::new(),
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
		println!("TODO: SEND REQUEST");

		// let request_id = self.request_id.fetch_add(1, Ordering::Relaxed);
		// let resp = resp.map(|v| Arc::new(v)); // Done so that `Self: Clone`
		// self.outbound_requests.insert(request_id, resp);

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

	/// TODO
	/// TODO: Allow broadcasting to all or a defined set of peers -> Deal with establishing connection if not already connected
	pub fn broadcast(&mut self, data: SpaceTimeMessage) {
		debug!("TODO: Broadcast");
		for (peer_id, conns) in &self.connected {
			debug!("TODO: Broadcast to peer: {:?}", peer_id);
			self.pending_events
				.push_back(NetworkBehaviourAction::NotifyHandler {
					peer_id: peer_id.0.clone(),
					handler: NotifyHandler::One(conns.first().unwrap().id), // TODO: Error handling
					event: data.clone(),
				});
		}
	}
}

impl<TMetadata, TEventFn, TConnFn> NetworkBehaviour for SpaceTime<TMetadata, TEventFn, TConnFn>
where
	TMetadata: crate::Metadata,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, crate::Event<TMetadata>, Output = ()>,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	type ConnectionHandler = Handler<TMetadata, TConnFn>;
	type OutEvent = ();

	fn new_handler(&mut self) -> Self::ConnectionHandler {
		Handler::new(self.state.clone())
	}

	fn addresses_of_peer(&mut self, peer: &libp2p::PeerId) -> Vec<Multiaddr> {
		let peer = &PeerId(*peer);

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
				self.connected
					.entry(PeerId(peer_id))
					.or_default()
					.push(Connection::new(connection_id, address));

				// TODO
				// if other_established == 0 {
				// 	if let Some(pending) = self.pending_outbound_requests.remove(&peer_id) {
				// 		for request in pending {
				// 			let request = self.try_send_request(&peer_id, request);
				// 			assert!(request.is_none());
				// 		}
				// 	}
				// }

				// let (peer, send_create_event) = {
				// 	let mut connected_peers = self.connected_peers.write().await;

				// 	let (peer, send_create_event) =
				// 		if let Some(mut peer) = connected_peers.remove(&peer_id) {
				// 			peer.active_connections = num_established;
				// 			(peer, false)
				// 		} else {
				// 			(
				// 				ConnectedPeer {
				// 					active_connections: num_established,
				// 					conn_type: endpoint.into(),
				// 				},
				// 				true,
				// 			)
				// 		};
				// 	connected_peers.insert(peer_id, peer.clone());
				// 	(peer, send_create_event)
				// };

				// if send_create_event {
				// 	// if matches!(peer.conn_type, ConnectionType::Dialer) { // TODO: This check is not working. Both are Dialer
				// 	if this.state.peer_id < peer_id { // TODO: Move back to previous check once it's fixed. This will work for now.
				// 		// TODO: This should be stored into request map to be handled properly and so errors can be reported
				// 		// TODO: handle the event of this not being sent properly because it means the other side won't startup.
				// 		debug!("sending establishment request to peer '{}'", peer_id);
				// 		// swarm.behaviour_mut().send_request(&peer_id, SpaceTimeMessage::Establish); // TODO
				// 	}

				// (this.fn_on_event)(this.state.clone(), Event::PeerConnected(peer)).await; // TODO
				// }

				// let temp_peer = ConnectedPeer {
				// 	active_connections: num_established,
				// 	conn_type: endpoint.into(),
				// };

				// TODO: Only emit this on first connection
				// tokio::spawn((this.fn_on_event)(
				// 	this.state.clone(), // TODO
				// 	Event::PeerConnected(temp_peer),
				// ));
			}
			FromSwarm::ConnectionClosed(event) => {
				let connections = self
					.connected
					.get_mut(&PeerId(event.peer_id))
					.expect("Expected some established connection to peer before closing.");

				let connection = connections
					.iter()
					.position(|c| c.id == event.connection_id)
					.map(|p: usize| connections.remove(p))
					.expect("Expected connection to be established before closing.");

				debug_assert_eq!(connections.is_empty(), event.remaining_established == 0);
				if connections.is_empty() {
					self.connected.remove(&PeerId(event.peer_id));
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
					.get_mut(&PeerId(event.peer_id))
					.expect("Address change can only happen on an established connection.");

				let connection = connections
					.iter_mut()
					.find(|c| c.id == event.connection_id)
					.expect("Address change can only happen on an established connection.");
				connection.address = new_address;
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
		peer_id: libp2p::PeerId,
		connection: ConnectionId,
		event: <Handler<TMetadata, TConnFn> as ConnectionHandler>::OutEvent,
	) {
		// let peer = &PeerId(*peer);
		// event.handle(peer_id, connection);
		todo!();
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
		// TODO
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		// TODO: This should emit a view of the active connections which we can shared with the UI.
		// TODO: We can't use async mutex locks here so this push based approach is probs best.

		Poll::Pending
	}
}

// TODO: Probs merge this into my connection management system and then remove it.
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
