//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application build on this library.

use std::{
	iter,
	sync::{atomic::AtomicU64, Arc},
};

use libp2p::{core::ProtocolName, swarm::NetworkBehaviour};
use serde::{Deserialize, Serialize};

mod handler;

pub use handler::*;

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
	/// Establish the connection
	Establish,

	/// Send data on behalf of application
	Application(Vec<u8>),
}

/// TODO
#[derive(Clone)]
pub struct SpaceTimeProtocol();

impl ProtocolName for SpaceTimeProtocol {
	fn protocol_name(&self) -> &[u8] {
		"/spacetime/1".as_bytes()
	}
}

///
/// BREAK
///
pub use handler::ProtocolSupport;

use futures::channel::oneshot;
use handler::RequestProtocol;
use libp2p::{
	core::{connection::ConnectionId, ConnectedPoint, Multiaddr, PeerId},
	swarm::{
		behaviour::{
			AddressChange, ConnectionClosed, ConnectionEstablished, DialFailure, FromSwarm,
		},
		dial_opts::DialOpts,
		IntoConnectionHandler, NetworkBehaviourAction, NotifyHandler, PollParameters,
	},
};
use smallvec::SmallVec;
use std::{
	collections::{HashMap, HashSet, VecDeque},
	fmt,
	task::{Context, Poll},
	time::Duration,
};
use tracing::warn;

use crate::{utils::AsyncFn2, ManagerEvent, ManagerRef, Metadata};

/// An inbound request or response.
#[derive(Debug)]
pub enum Message<TRequest, TResponse, TChannelResponse = TResponse> {
	/// A request message.
	Request {
		/// The ID of this request.
		request_id: RequestId,
		/// The request message.
		request: TRequest,
		/// The channel waiting for the response.
		///
		/// If this channel is dropped instead of being used to send a response
		/// via [`Behaviour::send_response`], a [`Event::InboundFailure`]
		/// with [`InboundFailure::ResponseOmission`] is emitted.
		channel: ResponseChannel<TChannelResponse>,
	},
	/// A response message.
	Response {
		/// The ID of the request that produced this response.
		///
		/// See [`Behaviour::send_request`].
		request_id: RequestId,
		/// The response message.
		response: TResponse,
	},
}

/// The events emitted by a request-response [`Behaviour`].
#[derive(Debug)]
pub enum Event {
	/// An incoming message (request or response).
	Message {
		/// The peer who sent the message.
		peer: PeerId,
		/// The incoming message.
		message: Message<SpaceTimeMessage, SpaceTimeMessage, SpaceTimeMessage>,
	},
	/// An outbound request failed.
	OutboundFailure {
		/// The peer to whom the request was sent.
		peer: PeerId,
		/// The (local) ID of the failed request.
		request_id: RequestId,
		/// The error that occurred.
		error: OutboundFailure,
	},
	/// An inbound request failed.
	InboundFailure {
		/// The peer from whom the request was received.
		peer: PeerId,
		/// The ID of the failed inbound request.
		request_id: RequestId,
		/// The error that occurred.
		error: InboundFailure,
	},
	/// A response to an inbound request has been sent.
	///
	/// When this event is received, the response has been flushed on
	/// the underlying transport connection.
	ResponseSent {
		/// The peer to whom the response was sent.
		peer: PeerId,
		/// The ID of the inbound request whose response was sent.
		request_id: RequestId,
	},
}

// TODO: Remove
impl Event {
	pub async fn handle<TMetadata: Metadata, TConnFn>(
		self,
		state: &Arc<ManagerRef<TMetadata>>,
		fn_on_connect: Arc<TConnFn>,
		active_requests: &mut HashMap<
			RequestId,
			tokio::sync::oneshot::Sender<Result<SpaceTimeMessage, OutboundFailure>>,
		>,
	) where
		TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
	{
		match self {
			Self::Message { peer, message } => {
				match message {
					Message::Request {
						request_id,
						request,
						channel,
					} => {
						match request {
							SpaceTimeMessage::Establish => {
								println!("WE ESTBALISHED BI");
								// TODO: Handle authentication here by moving over the old `ConnectionEstablishmentPayload` from `p2p`
							}
							SpaceTimeMessage::Application(data) => {
								// TODO: Should this be put in the `active_requests` queue???
								let state = state.clone();
								tokio::spawn(async move {
									let req = (fn_on_connect)(
										crate::Connection {
											manager: state.clone(),
										},
										data,
									)
									.await;

									match req {
										Ok(data) => {
											// swarm.behaviour().send_response(channel, SpaceTimeMessage::Application(data)).unwrap();

											// TODO: This is so cringe. The channel should be so unnecessary! Can we force the behavior into an `Arc`. Although I will probs yeet it from the codebase soon.
											match state
												.internal_tx
												.send(ManagerEvent::SendResponse(
													peer,
													SpaceTimeMessage::Application(data),
													channel,
												))
												.await
											{
												Ok(_) => {}
												Err(_err) => todo!(),
											}
										}
										Err(_err) => todo!(), // TODO: Imagine causing an error
									}
								});
							}
						}
					}
					Message::Response {
						request_id,
						response,
					} => match active_requests.remove(&request_id) {
						Some(resp) => resp.send(Ok(response)).unwrap(),
						None => warn!(
							"error unable to find destination for response id '{:?}'",
							request_id
						),
					},
				}
			}
			Self::OutboundFailure {
				peer,
				request_id,
				error,
			} => match active_requests.remove(&request_id) {
				Some(resp) => resp.send(Err(error)).unwrap(),
				None => warn!(
					"error with onbound request '{:?}' to peer '{:?}': '{:?}'",
					request_id, peer, error
				),
			},
			Self::InboundFailure {
				peer,
				request_id,
				error,
			} => {
				// TODO: Handle error

				warn!(
					"error with inbound request '{:?}' from peer '{:?}': '{:?}'",
					request_id, peer, error
				);
			}
			Self::ResponseSent { peer, request_id } => {
				// todo!();
			}
		}
	}
}

/// Possible failures occurring in the context of sending
/// an outbound request and receiving the response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundFailure {
	/// The request could not be sent because a dialing attempt failed.
	DialFailure,
	/// The request timed out before a response was received.
	///
	/// It is not known whether the request may have been
	/// received (and processed) by the remote peer.
	Timeout,
	/// The connection closed before a response was received.
	///
	/// It is not known whether the request may have been
	/// received (and processed) by the remote peer.
	ConnectionClosed,
	/// The remote supports none of the requested protocols.
	UnsupportedProtocols,
}

impl fmt::Display for OutboundFailure {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			OutboundFailure::DialFailure => write!(f, "Failed to dial the requested peer"),
			OutboundFailure::Timeout => write!(f, "Timeout while waiting for a response"),
			OutboundFailure::ConnectionClosed => {
				write!(f, "Connection was closed before a response was received")
			}
			OutboundFailure::UnsupportedProtocols => {
				write!(f, "The remote supports none of the requested protocols")
			}
		}
	}
}

impl std::error::Error for OutboundFailure {}

/// Possible failures occurring in the context of receiving an
/// inbound request and sending a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundFailure {
	/// The inbound request timed out, either while reading the
	/// incoming request or before a response is sent, e.g. if
	/// [`Behaviour::send_response`] is not called in a
	/// timely manner.
	Timeout,
	/// The connection closed before a response could be send.
	ConnectionClosed,
	/// The local peer supports none of the protocols requested
	/// by the remote.
	UnsupportedProtocols,
	/// The local peer failed to respond to an inbound request
	/// due to the [`ResponseChannel`] being dropped instead of
	/// being passed to [`Behaviour::send_response`].
	ResponseOmission,
}

impl fmt::Display for InboundFailure {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			InboundFailure::Timeout => {
				write!(f, "Timeout while receiving request or sending response")
			}
			InboundFailure::ConnectionClosed => {
				write!(f, "Connection was closed before a response could be sent")
			}
			InboundFailure::UnsupportedProtocols => write!(
				f,
				"The local peer supports none of the protocols requested by the remote"
			),
			InboundFailure::ResponseOmission => write!(
				f,
				"The response channel was dropped without sending a response to the remote"
			),
		}
	}
}

impl std::error::Error for InboundFailure {}

/// A channel for sending a response to an inbound request.
///
/// See [`Behaviour::send_response`].
#[derive(Debug)]
pub struct ResponseChannel<TResponse> {
	sender: oneshot::Sender<TResponse>,
}

impl<TResponse> ResponseChannel<TResponse> {
	/// Checks whether the response channel is still open, i.e.
	/// the `Behaviour` is still waiting for a
	/// a response to be sent via [`Behaviour::send_response`]
	/// and this response channel.
	///
	/// If the response channel is no longer open then the inbound
	/// request timed out waiting for the response.
	pub fn is_open(&self) -> bool {
		!self.sender.is_canceled()
	}
}

/// The ID of an inbound or outbound request.
///
/// Note: [`RequestId`]'s uniqueness is only guaranteed between two
/// inbound and likewise between two outbound requests. There is no
/// uniqueness guarantee in a set of both inbound and outbound
/// [`RequestId`]s nor in a set of inbound or outbound requests
/// originating from different [`Behaviour`]'s.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

impl fmt::Display for RequestId {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// A request/response protocol for some message codec.
pub struct Behaviour {
	/// The supported inbound protocols.
	inbound_protocols: SmallVec<[SpaceTimeProtocol; 2]>,
	/// The supported outbound protocols.
	outbound_protocols: SmallVec<[SpaceTimeProtocol; 2]>,
	/// The next (local) request ID.
	next_request_id: RequestId,
	/// The next (inbound) request ID.
	next_inbound_id: Arc<AtomicU64>,
	/// Pending events to return from `poll`.
	pending_events: VecDeque<
		NetworkBehaviourAction<
			Event, // <<SpaceTimeCodec as Codec>::Request, <SpaceTimeCodec as Codec>::Response>,
			Handler,
		>,
	>,
	/// The currently connected peers, their pending outbound and inbound responses and their known,
	/// reachable addresses, if any.
	connected: HashMap<PeerId, SmallVec<[Connection; 2]>>,
	/// Externally managed addresses via `add_address` and `remove_address`.
	addresses: HashMap<PeerId, SmallVec<[Multiaddr; 6]>>,
	/// Requests that have not yet been sent and are waiting for a connection
	/// to be established.
	pending_outbound_requests: HashMap<PeerId, SmallVec<[RequestProtocol; 10]>>,
}

impl Behaviour {
	/// TODO
	pub fn new() -> Self {
		let protocols = iter::once((SpaceTimeProtocol(), ProtocolSupport::Full));

		let mut inbound_protocols = SmallVec::new();
		let mut outbound_protocols = SmallVec::new();
		for (p, s) in protocols {
			if s.inbound() {
				inbound_protocols.push(p.clone());
			}
			if s.outbound() {
				outbound_protocols.push(p.clone());
			}
		}
		Behaviour {
			inbound_protocols,
			outbound_protocols,
			next_request_id: RequestId(1),
			next_inbound_id: Arc::new(AtomicU64::new(1)),
			pending_events: VecDeque::new(),
			connected: HashMap::new(),
			pending_outbound_requests: HashMap::new(),
			addresses: HashMap::new(),
		}
	}

	/// Initiates sending a request.
	///
	/// If the targeted peer is currently not connected, a dialing
	/// attempt is initiated and the request is sent as soon as a
	/// connection is established.
	///
	/// > **Note**: In order for such a dialing attempt to succeed,
	/// > the `RequestResonse` protocol must either be embedded
	/// > in another `NetworkBehaviour` that provides peer and
	/// > address discovery, or known addresses of peers must be
	/// > managed via [`Behaviour::add_address`] and
	/// > [`Behaviour::remove_address`].
	pub fn send_request(&mut self, peer: &PeerId, request: SpaceTimeMessage) -> RequestId {
		let request_id = self.next_request_id();
		let request = RequestProtocol {
			request_id,
			protocols: self.outbound_protocols.clone(),
			request,
		};

		if let Some(request) = self.try_send_request(peer, request) {
			let handler = self.new_handler();
			self.pending_events.push_back(NetworkBehaviourAction::Dial {
				opts: DialOpts::peer_id(*peer).build(),
				handler,
			});
			self.pending_outbound_requests
				.entry(*peer)
				.or_default()
				.push(request);
		}

		request_id
	}

	/// Initiates sending a response to an inbound request.
	///
	/// If the [`ResponseChannel`] is already closed due to a timeout or the
	/// connection being closed, the response is returned as an `Err` for
	/// further handling. Once the response has been successfully sent on the
	/// corresponding connection, [`Event::ResponseSent`] is
	/// emitted. In all other cases [`Event::InboundFailure`]
	/// will be or has been emitted.
	///
	/// The provided `ResponseChannel` is obtained from an inbound
	/// [`Message::Request`].
	pub fn send_response(
		&mut self,
		ch: ResponseChannel<SpaceTimeMessage>,
		rs: SpaceTimeMessage,
	) -> Result<(), SpaceTimeMessage> {
		ch.sender.send(rs)
	}

	/// Adds a known address for a peer that can be used for
	/// dialing attempts by the `Swarm`, i.e. is returned
	/// by [`NetworkBehaviour::addresses_of_peer`].
	///
	/// Addresses added in this way are only removed by `remove_address`.
	pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
		self.addresses.entry(*peer).or_default().push(address);
	}

	/// Removes an address of a peer previously added via `add_address`.
	pub fn remove_address(&mut self, peer: &PeerId, address: &Multiaddr) {
		let mut last = false;
		if let Some(addresses) = self.addresses.get_mut(peer) {
			addresses.retain(|a| a != address);
			last = addresses.is_empty();
		}
		if last {
			self.addresses.remove(peer);
		}
	}

	/// Checks whether a peer is currently connected.
	pub fn is_connected(&self, peer: &PeerId) -> bool {
		if let Some(connections) = self.connected.get(peer) {
			!connections.is_empty()
		} else {
			false
		}
	}

	/// Checks whether an outbound request to the peer with the provided
	/// [`PeerId`] initiated by [`Behaviour::send_request`] is still
	/// pending, i.e. waiting for a response.
	pub fn is_pending_outbound(&self, peer: &PeerId, request_id: &RequestId) -> bool {
		// Check if request is already sent on established connection.
		let est_conn = self
			.connected
			.get(peer)
			.map(|cs| {
				cs.iter()
					.any(|c| c.pending_inbound_responses.contains(request_id))
			})
			.unwrap_or(false);
		// Check if request is still pending to be sent.
		let pen_conn = self
			.pending_outbound_requests
			.get(peer)
			.map(|rps| rps.iter().any(|rp| rp.request_id == *request_id))
			.unwrap_or(false);

		est_conn || pen_conn
	}

	/// Checks whether an inbound request from the peer with the provided
	/// [`PeerId`] is still pending, i.e. waiting for a response by the local
	/// node through [`Behaviour::send_response`].
	pub fn is_pending_inbound(&self, peer: &PeerId, request_id: &RequestId) -> bool {
		self.connected
			.get(peer)
			.map(|cs| {
				cs.iter()
					.any(|c| c.pending_outbound_responses.contains(request_id))
			})
			.unwrap_or(false)
	}

	/// Returns the next request ID.
	fn next_request_id(&mut self) -> RequestId {
		let request_id = self.next_request_id;
		self.next_request_id.0 += 1;
		request_id
	}

	/// Tries to send a request by queueing an appropriate event to be
	/// emitted to the `Swarm`. If the peer is not currently connected,
	/// the given request is return unchanged.
	fn try_send_request(
		&mut self,
		peer: &PeerId,
		request: RequestProtocol,
	) -> Option<RequestProtocol> {
		if let Some(connections) = self.connected.get_mut(peer) {
			if connections.is_empty() {
				return Some(request);
			}
			let ix = (request.request_id.0 as usize) % connections.len();
			let conn = &mut connections[ix];
			conn.pending_inbound_responses.insert(request.request_id);
			self.pending_events
				.push_back(NetworkBehaviourAction::NotifyHandler {
					peer_id: *peer,
					handler: NotifyHandler::One(conn.id),
					event: request,
				});
			None
		} else {
			Some(request)
		}
	}

	/// Remove pending outbound response for the given peer and connection.
	///
	/// Returns `true` if the provided connection to the given peer is still
	/// alive and the [`RequestId`] was previously present and is now removed.
	/// Returns `false` otherwise.
	fn remove_pending_outbound_response(
		&mut self,
		peer: &PeerId,
		connection: ConnectionId,
		request: RequestId,
	) -> bool {
		self.get_connection_mut(peer, connection)
			.map(|c| c.pending_outbound_responses.remove(&request))
			.unwrap_or(false)
	}

	/// Remove pending inbound response for the given peer and connection.
	///
	/// Returns `true` if the provided connection to the given peer is still
	/// alive and the [`RequestId`] was previously present and is now removed.
	/// Returns `false` otherwise.
	fn remove_pending_inbound_response(
		&mut self,
		peer: &PeerId,
		connection: ConnectionId,
		request: &RequestId,
	) -> bool {
		self.get_connection_mut(peer, connection)
			.map(|c| c.pending_inbound_responses.remove(request))
			.unwrap_or(false)
	}

	/// Returns a mutable reference to the connection in `self.connected`
	/// corresponding to the given [`PeerId`] and [`ConnectionId`].
	fn get_connection_mut(
		&mut self,
		peer: &PeerId,
		connection: ConnectionId,
	) -> Option<&mut Connection> {
		self.connected
			.get_mut(peer)
			.and_then(|connections| connections.iter_mut().find(|c| c.id == connection))
	}

	fn on_address_change(
		&mut self,
		AddressChange {
			peer_id,
			connection_id,
			new,
			..
		}: AddressChange,
	) {
		let new_address = match new {
			ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
			ConnectedPoint::Listener { .. } => None,
		};
		let connections = self
			.connected
			.get_mut(&peer_id)
			.expect("Address change can only happen on an established connection.");

		let connection = connections
			.iter_mut()
			.find(|c| c.id == connection_id)
			.expect("Address change can only happen on an established connection.");
		connection.address = new_address;
	}

	fn on_connection_established(
		&mut self,
		ConnectionEstablished {
			peer_id,
			connection_id,
			endpoint,
			other_established,
			..
		}: ConnectionEstablished,
	) {
		let address = match endpoint {
			ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
			ConnectedPoint::Listener { .. } => None,
		};
		self.connected
			.entry(peer_id)
			.or_default()
			.push(Connection::new(connection_id, address));

		if other_established == 0 {
			if let Some(pending) = self.pending_outbound_requests.remove(&peer_id) {
				for request in pending {
					let request = self.try_send_request(&peer_id, request);
					assert!(request.is_none());
				}
			}
		}
	}

	fn on_connection_closed(
		&mut self,
		ConnectionClosed {
			peer_id,
			connection_id,
			remaining_established,
			..
		}: ConnectionClosed<<Self as NetworkBehaviour>::ConnectionHandler>,
	) {
		let connections = self
			.connected
			.get_mut(&peer_id)
			.expect("Expected some established connection to peer before closing.");

		let connection = connections
			.iter()
			.position(|c| c.id == connection_id)
			.map(|p: usize| connections.remove(p))
			.expect("Expected connection to be established before closing.");

		debug_assert_eq!(connections.is_empty(), remaining_established == 0);
		if connections.is_empty() {
			self.connected.remove(&peer_id);
		}

		for request_id in connection.pending_outbound_responses {
			self.pending_events
				.push_back(NetworkBehaviourAction::GenerateEvent(
					Event::InboundFailure {
						peer: peer_id,
						request_id,
						error: InboundFailure::ConnectionClosed,
					},
				));
		}

		for request_id in connection.pending_inbound_responses {
			self.pending_events
				.push_back(NetworkBehaviourAction::GenerateEvent(
					Event::OutboundFailure {
						peer: peer_id,
						request_id,
						error: OutboundFailure::ConnectionClosed,
					},
				));
		}
	}

	fn on_dial_failure(
		&mut self,
		DialFailure { peer_id, .. }: DialFailure<<Self as NetworkBehaviour>::ConnectionHandler>,
	) {
		if let Some(peer) = peer_id {
			// If there are pending outgoing requests when a dial failure occurs,
			// it is implied that we are not connected to the peer, since pending
			// outgoing requests are drained when a connection is established and
			// only created when a peer is not connected when a request is made.
			// Thus these requests must be considered failed, even if there is
			// another, concurrent dialing attempt ongoing.
			if let Some(pending) = self.pending_outbound_requests.remove(&peer) {
				for request in pending {
					self.pending_events
						.push_back(NetworkBehaviourAction::GenerateEvent(
							Event::OutboundFailure {
								peer,
								request_id: request.request_id,
								error: OutboundFailure::DialFailure,
							},
						));
				}
			}
		}
	}
}

impl NetworkBehaviour for Behaviour {
	type ConnectionHandler = Handler;
	type OutEvent = Event;

	fn new_handler(&mut self) -> Self::ConnectionHandler {
		Handler::new(
			self.inbound_protocols.clone(),
			Duration::from_secs(10),
			Duration::from_secs(10),
			self.next_inbound_id.clone(),
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
			FromSwarm::ConnectionEstablished(connection_established) => {
				self.on_connection_established(connection_established)
			}
			FromSwarm::ConnectionClosed(connection_closed) => {
				self.on_connection_closed(connection_closed)
			}
			FromSwarm::AddressChange(address_change) => self.on_address_change(address_change),
			FromSwarm::DialFailure(dial_failure) => self.on_dial_failure(dial_failure),
			FromSwarm::ListenFailure(_) => {}
			FromSwarm::NewListener(_) => {}
			FromSwarm::NewListenAddr(_) => {}
			FromSwarm::ExpiredListenAddr(_) => {}
			FromSwarm::ListenerError(_) => {}
			FromSwarm::ListenerClosed(_) => {}
			FromSwarm::NewExternalAddr(_) => {}
			FromSwarm::ExpiredExternalAddr(_) => {}
		}
	}

	fn on_connection_handler_event(
		&mut self,
		peer: PeerId,
		connection: ConnectionId,
		event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as
            libp2p::swarm::ConnectionHandler>::OutEvent,
	) {
		match event {
			handler::TODOEvent::Response {
				request_id,
				response,
			} => {
				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				debug_assert!(
					removed,
					"Expect request_id to be pending before receiving response.",
				);

				let message = Message::Response {
					request_id,
					response,
				};
				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
						peer,
						message,
					}));
			}
			handler::TODOEvent::Request {
				request_id,
				request,
				sender,
			} => {
				let channel = ResponseChannel { sender };
				let message = Message::Request {
					request_id,
					request,
					channel,
				};
				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
						peer,
						message,
					}));

				match self.get_connection_mut(&peer, connection) {
					Some(connection) => {
						let inserted = connection.pending_outbound_responses.insert(request_id);
						debug_assert!(inserted, "Expect id of new request to be unknown.");
					}
					// Connection closed after `Event::Request` has been emitted.
					None => {
						self.pending_events
							.push_back(NetworkBehaviourAction::GenerateEvent(
								Event::InboundFailure {
									peer,
									request_id,
									error: InboundFailure::ConnectionClosed,
								},
							));
					}
				}
			}
			handler::TODOEvent::ResponseSent(request_id) => {
				let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
				debug_assert!(
					removed,
					"Expect request_id to be pending before response is sent."
				);

				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(Event::ResponseSent {
						peer,
						request_id,
					}));
			}
			handler::TODOEvent::ResponseOmission(request_id) => {
				let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
				debug_assert!(
					removed,
					"Expect request_id to be pending before response is omitted.",
				);

				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(
						Event::InboundFailure {
							peer,
							request_id,
							error: InboundFailure::ResponseOmission,
						},
					));
			}
			handler::TODOEvent::OutboundTimeout(request_id) => {
				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				debug_assert!(
					removed,
					"Expect request_id to be pending before request times out."
				);

				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(
						Event::OutboundFailure {
							peer,
							request_id,
							error: OutboundFailure::Timeout,
						},
					));
			}
			handler::TODOEvent::InboundTimeout(request_id) => {
				// Note: `Event::InboundTimeout` is emitted both for timing
				// out to receive the request and for timing out sending the response. In the former
				// case the request is never added to `pending_outbound_responses` and thus one can
				// not assert the request_id to be present before removing it.
				self.remove_pending_outbound_response(&peer, connection, request_id);

				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(
						Event::InboundFailure {
							peer,
							request_id,
							error: InboundFailure::Timeout,
						},
					));
			}
			handler::TODOEvent::OutboundUnsupportedProtocols(request_id) => {
				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				debug_assert!(
					removed,
					"Expect request_id to be pending before failing to connect.",
				);

				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(
						Event::OutboundFailure {
							peer,
							request_id,
							error: OutboundFailure::UnsupportedProtocols,
						},
					));
			}
			handler::TODOEvent::InboundUnsupportedProtocols(request_id) => {
				// Note: No need to call `self.remove_pending_outbound_response`,
				// `Event::Request` was never emitted for this request and
				// thus request was never added to `pending_outbound_responses`.
				self.pending_events
					.push_back(NetworkBehaviourAction::GenerateEvent(
						Event::InboundFailure {
							peer,
							request_id,
							error: InboundFailure::UnsupportedProtocols,
						},
					));
			}
		}
	}

	fn poll(
		&mut self,
		_: &mut Context<'_>,
		_: &mut impl PollParameters,
	) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
		if let Some(ev) = self.pending_events.pop_front() {
			return Poll::Ready(ev);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		Poll::Pending
	}
}

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

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
