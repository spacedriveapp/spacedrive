//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application build on this library.

use std::{
	iter,
	sync::{atomic::AtomicU64, Arc},
};

use libp2p::{core::ProtocolName, swarm::NetworkBehaviour};
use serde::{Deserialize, Serialize};

mod codec;
mod handler;

pub use codec::*;
pub use handler::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
	/// Establish the connection
	Establish,

	/// Send data on behalf of application
	Application(Vec<u8>),
}

#[derive(Clone)]
pub struct SpaceTimeProtocol();

impl ProtocolName for SpaceTimeProtocol {
	fn protocol_name(&self) -> &[u8] {
		"/spacetime/1".as_bytes()
	}
}

// ///
// /// BREAK
// ///

// /// TODO
// pub struct SpacetimeBehaviour {
// 	request_id: Arc<AtomicU64>,
// 	// TODO
// }

// impl NetworkBehaviour for SpacetimeBehaviour {
// 	type ConnectionHandler = Handler<TCodec>;
// 	type OutEvent = Event;

// 	fn new_handler(&mut self) -> Self::ConnectionHandler {
// 		Handler::new(
// 			self.inbound_protocols.clone(),
// 			self.codec.clone(),
// 			self.config.connection_keep_alive,
// 			self.config.request_timeout,
// 			self.next_inbound_id.clone(),
// 		)
// 	}

// 	fn addresses_of_peer(&mut self, peer: &PeerId) -> Vec<Multiaddr> {
// 		let mut addresses = Vec::new();
// 		if let Some(connections) = self.connected.get(peer) {
// 			addresses.extend(connections.iter().filter_map(|c| c.address.clone()))
// 		}
// 		if let Some(more) = self.addresses.get(peer) {
// 			addresses.extend(more.into_iter().cloned());
// 		}
// 		addresses
// 	}

// 	fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
// 		match event {
// 			FromSwarm::ConnectionEstablished(connection_established) => {
// 				self.on_connection_established(connection_established)
// 			}
// 			FromSwarm::ConnectionClosed(connection_closed) => {
// 				self.on_connection_closed(connection_closed)
// 			}
// 			FromSwarm::AddressChange(address_change) => self.on_address_change(address_change),
// 			FromSwarm::DialFailure(dial_failure) => self.on_dial_failure(dial_failure),
// 			FromSwarm::ListenFailure(_) => {}
// 			FromSwarm::NewListener(_) => {}
// 			FromSwarm::NewListenAddr(_) => {}
// 			FromSwarm::ExpiredListenAddr(_) => {}
// 			FromSwarm::ListenerError(_) => {}
// 			FromSwarm::ListenerClosed(_) => {}
// 			FromSwarm::NewExternalAddr(_) => {}
// 			FromSwarm::ExpiredExternalAddr(_) => {}
// 		}
// 	}

// 	fn on_connection_handler_event(
// 		&mut self,
// 		peer: PeerId,
// 		connection: ConnectionId,
// 		event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as
//             libp2p_swarm::ConnectionHandler>::OutEvent,
// 	) {
// 		match event {
// 			handler::Event::Response {
// 				request_id,
// 				response,
// 			} => {
// 				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
// 				debug_assert!(
// 					removed,
// 					"Expect request_id to be pending before receiving response.",
// 				);

// 				let message = Message::Response {
// 					request_id,
// 					response,
// 				};
// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
// 						peer,
// 						message,
// 					}));
// 			}
// 			handler::Event::Request {
// 				request_id,
// 				request,
// 				sender,
// 			} => {
// 				let channel = ResponseChannel { sender };
// 				let message = Message::Request {
// 					request_id,
// 					request,
// 					channel,
// 				};
// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
// 						peer,
// 						message,
// 					}));

// 				match self.get_connection_mut(&peer, connection) {
// 					Some(connection) => {
// 						let inserted = connection.pending_outbound_responses.insert(request_id);
// 						debug_assert!(inserted, "Expect id of new request to be unknown.");
// 					}
// 					// Connection closed after `Event::Request` has been emitted.
// 					None => {
// 						self.pending_events
// 							.push_back(NetworkBehaviourAction::GenerateEvent(
// 								Event::InboundFailure {
// 									peer,
// 									request_id,
// 									error: InboundFailure::ConnectionClosed,
// 								},
// 							));
// 					}
// 				}
// 			}
// 			handler::Event::ResponseSent(request_id) => {
// 				let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
// 				debug_assert!(
// 					removed,
// 					"Expect request_id to be pending before response is sent."
// 				);

// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(Event::ResponseSent {
// 						peer,
// 						request_id,
// 					}));
// 			}
// 			handler::Event::ResponseOmission(request_id) => {
// 				let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
// 				debug_assert!(
// 					removed,
// 					"Expect request_id to be pending before response is omitted.",
// 				);

// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(
// 						Event::InboundFailure {
// 							peer,
// 							request_id,
// 							error: InboundFailure::ResponseOmission,
// 						},
// 					));
// 			}
// 			handler::Event::OutboundTimeout(request_id) => {
// 				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
// 				debug_assert!(
// 					removed,
// 					"Expect request_id to be pending before request times out."
// 				);

// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(
// 						Event::OutboundFailure {
// 							peer,
// 							request_id,
// 							error: OutboundFailure::Timeout,
// 						},
// 					));
// 			}
// 			handler::Event::InboundTimeout(request_id) => {
// 				// Note: `Event::InboundTimeout` is emitted both for timing
// 				// out to receive the request and for timing out sending the response. In the former
// 				// case the request is never added to `pending_outbound_responses` and thus one can
// 				// not assert the request_id to be present before removing it.
// 				self.remove_pending_outbound_response(&peer, connection, request_id);

// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(
// 						Event::InboundFailure {
// 							peer,
// 							request_id,
// 							error: InboundFailure::Timeout,
// 						},
// 					));
// 			}
// 			handler::Event::OutboundUnsupportedProtocols(request_id) => {
// 				let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
// 				debug_assert!(
// 					removed,
// 					"Expect request_id to be pending before failing to connect.",
// 				);

// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(
// 						Event::OutboundFailure {
// 							peer,
// 							request_id,
// 							error: OutboundFailure::UnsupportedProtocols,
// 						},
// 					));
// 			}
// 			handler::Event::InboundUnsupportedProtocols(request_id) => {
// 				// Note: No need to call `self.remove_pending_outbound_response`,
// 				// `Event::Request` was never emitted for this request and
// 				// thus request was never added to `pending_outbound_responses`.
// 				self.pending_events
// 					.push_back(NetworkBehaviourAction::GenerateEvent(
// 						Event::InboundFailure {
// 							peer,
// 							request_id,
// 							error: InboundFailure::UnsupportedProtocols,
// 						},
// 					));
// 			}
// 		}
// 	}

// 	fn poll(
// 		&mut self,
// 		_: &mut Context<'_>,
// 		_: &mut impl PollParameters,
// 	) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
// 		if let Some(ev) = self.pending_events.pop_front() {
// 			return Poll::Ready(ev);
// 		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
// 			self.pending_events.shrink_to_fit();
// 		}

// 		Poll::Pending
// 	}
// }

// struct Handler {
// 	// /// The supported inbound protocols.
// 	// inbound_protocols: SmallVec<[TCodec::Protocol; 2]>,
// 	// /// The request/response message codec.
// 	// codec: TCodec,
// 	// /// The keep-alive timeout of idle connections. A connection is considered
// 	// /// idle if there are no outbound substreams.
// 	// keep_alive_timeout: Duration,
// 	// /// The timeout for inbound and outbound substreams (i.e. request
// 	// /// and response processing).
// 	// substream_timeout: Duration,
// 	// /// The current connection keep-alive.
// 	// keep_alive: KeepAlive,
// 	// /// A pending fatal error that results in the connection being closed.
// 	// pending_error: Option<ConnectionHandlerUpgrErr<io::Error>>,
// 	// /// Queue of events to emit in `poll()`.
// 	// pending_events: VecDeque<Event<TCodec>>,
// 	// /// Outbound upgrades waiting to be emitted as an `OutboundSubstreamRequest`.
// 	// outbound: VecDeque<RequestProtocol<TCodec>>,
// 	// /// Inbound upgrades waiting for the incoming request.
// 	// inbound: FuturesUnordered<
// 	// 	BoxFuture<
// 	// 		'static,
// 	// 		Result<
// 	// 			(
// 	// 				(RequestId, TCodec::Request),
// 	// 				oneshot::Sender<TCodec::Response>,
// 	// 			),
// 	// 			oneshot::Canceled,
// 	// 		>,
// 	// 	>,
// 	// >,
// 	// inbound_request_id: Arc<AtomicU64>,
// }

// impl Handler {
// 	pub(super) fn new() -> Self {
// 		Self {}
// 	}

// 	// fn on_fully_negotiated_inbound(
// 	// 	&mut self,
// 	// 	FullyNegotiatedInbound {
// 	// 		protocol: sent,
// 	// 		info: request_id,
// 	// 	}: FullyNegotiatedInbound<
// 	// 		<Self as ConnectionHandler>::InboundProtocol,
// 	// 		<Self as ConnectionHandler>::InboundOpenInfo,
// 	// 	>,
// 	// ) {
// 	// 	if sent {
// 	// 		self.pending_events
// 	// 			.push_back(Event::ResponseSent(request_id))
// 	// 	} else {
// 	// 		self.pending_events
// 	// 			.push_back(Event::ResponseOmission(request_id))
// 	// 	}
// 	// }

// 	// fn on_dial_upgrade_error(
// 	// 	&mut self,
// 	// 	DialUpgradeError { info, error }: DialUpgradeError<
// 	// 		<Self as ConnectionHandler>::OutboundOpenInfo,
// 	// 		<Self as ConnectionHandler>::OutboundProtocol,
// 	// 	>,
// 	// ) {
// 	// 	match error {
// 	// 		ConnectionHandlerUpgrErr::Timeout => {
// 	// 			self.pending_events.push_back(Event::OutboundTimeout(info));
// 	// 		}
// 	// 		ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
// 	// 			// The remote merely doesn't support the protocol(s) we requested.
// 	// 			// This is no reason to close the connection, which may
// 	// 			// successfully communicate with other protocols already.
// 	// 			// An event is reported to permit user code to react to the fact that
// 	// 			// the remote peer does not support the requested protocol(s).
// 	// 			self.pending_events
// 	// 				.push_back(Event::OutboundUnsupportedProtocols(info));
// 	// 		}
// 	// 		_ => {
// 	// 			// Anything else is considered a fatal error or misbehaviour of
// 	// 			// the remote peer and results in closing the connection.
// 	// 			self.pending_error = Some(error);
// 	// 		}
// 	// 	}
// 	// }
// 	// fn on_listen_upgrade_error(
// 	// 	&mut self,
// 	// 	ListenUpgradeError { info, error }: ListenUpgradeError<
// 	// 		<Self as ConnectionHandler>::InboundOpenInfo,
// 	// 		<Self as ConnectionHandler>::InboundProtocol,
// 	// 	>,
// 	// ) {
// 	// 	match error {
// 	// 		ConnectionHandlerUpgrErr::Timeout => {
// 	// 			self.pending_events.push_back(Event::InboundTimeout(info))
// 	// 		}
// 	// 		ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
// 	// 			// The local peer merely doesn't support the protocol(s) requested.
// 	// 			// This is no reason to close the connection, which may
// 	// 			// successfully communicate with other protocols already.
// 	// 			// An event is reported to permit user code to react to the fact that
// 	// 			// the local peer does not support the requested protocol(s).
// 	// 			self.pending_events
// 	// 				.push_back(Event::InboundUnsupportedProtocols(info));
// 	// 		}
// 	// 		_ => {
// 	// 			// Anything else is considered a fatal error or misbehaviour of
// 	// 			// the remote peer and results in closing the connection.
// 	// 			self.pending_error = Some(error);
// 	// 		}
// 	// 	}
// 	// }
// }

/// The events emitted by the [`Handler`].
// pub enum Event {
// /// A request has been received.
// Request {
// 	request_id: RequestId,
// 	request: TCodec::Request,
// 	sender: oneshot::Sender<TCodec::Response>,
// },
// /// A response has been received.
// Response {
// 	request_id: RequestId,
// 	response: TCodec::Response,
// },
// /// A response to an inbound request has been sent.
// ResponseSent(RequestId),
// /// A response to an inbound request was omitted as a result
// /// of dropping the response `sender` of an inbound `Request`.
// ResponseOmission(RequestId),
// /// An outbound request timed out while sending the request
// /// or waiting for the response.
// OutboundTimeout(RequestId),
// /// An outbound request failed to negotiate a mutually supported protocol.
// OutboundUnsupportedProtocols(RequestId),
// /// An inbound request timed out while waiting for the request
// /// or sending the response.
// InboundTimeout(RequestId),
// /// An inbound request failed to negotiate a mutually supported protocol.
// InboundUnsupportedProtocols(RequestId),
// }

// impl<TCodec: Codec> fmt::Debug for Event<TCodec> {
// 	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
// 		match self {
// 			Event::Request {
// 				request_id,
// 				request: _,
// 				sender: _,
// 			} => f
// 				.debug_struct("Event::Request")
// 				.field("request_id", request_id)
// 				.finish(),
// 			Event::Response {
// 				request_id,
// 				response: _,
// 			} => f
// 				.debug_struct("Event::Response")
// 				.field("request_id", request_id)
// 				.finish(),
// 			Event::ResponseSent(request_id) => f
// 				.debug_tuple("Event::ResponseSent")
// 				.field(request_id)
// 				.finish(),
// 			Event::ResponseOmission(request_id) => f
// 				.debug_tuple("Event::ResponseOmission")
// 				.field(request_id)
// 				.finish(),
// 			Event::OutboundTimeout(request_id) => f
// 				.debug_tuple("Event::OutboundTimeout")
// 				.field(request_id)
// 				.finish(),
// 			Event::OutboundUnsupportedProtocols(request_id) => f
// 				.debug_tuple("Event::OutboundUnsupportedProtocols")
// 				.field(request_id)
// 				.finish(),
// 			Event::InboundTimeout(request_id) => f
// 				.debug_tuple("Event::InboundTimeout")
// 				.field(request_id)
// 				.finish(),
// 			Event::InboundUnsupportedProtocols(request_id) => f
// 				.debug_tuple("Event::InboundUnsupportedProtocols")
// 				.field(request_id)
// 				.finish(),
// 		}
// 	}
// }

// impl ConnectionHandler for Handler {
// 	type InEvent = RequestProtocol<SpaceTIme>;
// type OutEvent = Event<TCodec>;
// type Error = ConnectionHandlerUpgrErr<io::Error>;
// type InboundProtocol = ResponseProtocol<TCodec>;
// type OutboundProtocol = RequestProtocol<TCodec>;
// type OutboundOpenInfo = RequestId;
// type InboundOpenInfo = RequestId;

// fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
// 	// A channel for notifying the handler when the inbound
// 	// upgrade received the request.
// 	let (rq_send, rq_recv) = oneshot::channel();

// 	// A channel for notifying the inbound upgrade when the
// 	// response is sent.
// 	let (rs_send, rs_recv) = oneshot::channel();

// 	let request_id = RequestId(self.inbound_request_id.fetch_add(1, Ordering::Relaxed));

// 	// By keeping all I/O inside the `ResponseProtocol` and thus the
// 	// inbound substream upgrade via above channels, we ensure that it
// 	// is all subject to the configured timeout without extra bookkeeping
// 	// for inbound substreams as well as their timeouts and also make the
// 	// implementation of inbound and outbound upgrades symmetric in
// 	// this sense.
// 	let proto = ResponseProtocol {
// 		protocols: self.inbound_protocols.clone(),
// 		codec: self.codec.clone(),
// 		request_sender: rq_send,
// 		response_receiver: rs_recv,
// 		request_id,
// 	};

// 	// The handler waits for the request to come in. It then emits
// 	// `Event::Request` together with a
// 	// `ResponseChannel`.
// 	self.inbound
// 		.push(rq_recv.map_ok(move |rq| (rq, rs_send)).boxed());

// 	SubstreamProtocol::new(proto, request_id).with_timeout(self.substream_timeout)
// }

// fn on_behaviour_event(&mut self, request: Self::InEvent) {
// 	self.keep_alive = KeepAlive::Yes;
// 	self.outbound.push_back(request);
// }

// fn connection_keep_alive(&self) -> KeepAlive {
// 	self.keep_alive
// }

// fn poll(
// 	&mut self,
// 	cx: &mut Context<'_>,
// ) -> Poll<ConnectionHandlerEvent<RequestProtocol<TCodec>, RequestId, Self::OutEvent, Self::Error>>
// {
// 	// Check for a pending (fatal) error.
// 	if let Some(err) = self.pending_error.take() {
// 		// The handler will not be polled again by the `Swarm`.
// 		return Poll::Ready(ConnectionHandlerEvent::Close(err));
// 	}

// 	// Drain pending events.
// 	if let Some(event) = self.pending_events.pop_front() {
// 		return Poll::Ready(ConnectionHandlerEvent::Custom(event));
// 	} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
// 		self.pending_events.shrink_to_fit();
// 	}

// 	// Check for inbound requests.
// 	while let Poll::Ready(Some(result)) = self.inbound.poll_next_unpin(cx) {
// 		match result {
// 			Ok(((id, rq), rs_sender)) => {
// 				// We received an inbound request.
// 				self.keep_alive = KeepAlive::Yes;
// 				return Poll::Ready(ConnectionHandlerEvent::Custom(Event::Request {
// 					request_id: id,
// 					request: rq,
// 					sender: rs_sender,
// 				}));
// 			}
// 			Err(oneshot::Canceled) => {
// 				// The inbound upgrade has errored or timed out reading
// 				// or waiting for the request. The handler is informed
// 				// via `inject_listen_upgrade_error`.
// 			}
// 		}
// 	}

// 	// Emit outbound requests.
// 	if let Some(request) = self.outbound.pop_front() {
// 		let info = request.request_id;
// 		return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
// 			protocol: SubstreamProtocol::new(request, info)
// 				.with_timeout(self.substream_timeout),
// 		});
// 	}

// 	debug_assert!(self.outbound.is_empty());

// 	if self.outbound.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
// 		self.outbound.shrink_to_fit();
// 	}

// 	if self.inbound.is_empty() && self.keep_alive.is_yes() {
// 		// No new inbound or outbound requests. However, we may just have
// 		// started the latest inbound or outbound upgrade(s), so make sure
// 		// the keep-alive timeout is preceded by the substream timeout.
// 		let until = Instant::now() + self.substream_timeout + self.keep_alive_timeout;
// 		self.keep_alive = KeepAlive::Until(until);
// 	}

// 	Poll::Pending
// }

// fn on_connection_event(
// 	&mut self,
// 	event: ConnectionEvent<
// 		Self::InboundProtocol,
// 		Self::OutboundProtocol,
// 		Self::InboundOpenInfo,
// 		Self::OutboundOpenInfo,
// 	>,
// ) {
// 	match event {
// 		ConnectionEvent::FullyNegotiatedInbound(fully_negotiated_inbound) => {
// 			self.on_fully_negotiated_inbound(fully_negotiated_inbound)
// 		}
// 		ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
// 			protocol: response,
// 			info: request_id,
// 		}) => {
// 			self.pending_events.push_back(Event::Response {
// 				request_id,
// 				response,
// 			});
// 		}
// 		ConnectionEvent::DialUpgradeError(dial_upgrade_error) => {
// 			self.on_dial_upgrade_error(dial_upgrade_error)
// 		}
// 		ConnectionEvent::ListenUpgradeError(listen_upgrade_error) => {
// 			self.on_listen_upgrade_error(listen_upgrade_error)
// 		}
// 		ConnectionEvent::AddressChange(_) => {}
// 	}
// }
// }

///
/// TODO
///
// Copyright 2020 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
// pub mod codec;
// pub mod handler;
// pub use codec::{Codec, ProtocolName};

#[allow(deprecated)]
pub use codec::RequestResponseCodec;

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

/// The configuration for a `Behaviour` protocol.
#[derive(Debug, Clone)]
pub struct Config {
	request_timeout: Duration,
	connection_keep_alive: Duration,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			connection_keep_alive: Duration::from_secs(10),
			request_timeout: Duration::from_secs(10),
		}
	}
}

impl Config {
	/// Sets the keep-alive timeout of idle connections.
	pub fn set_connection_keep_alive(&mut self, v: Duration) -> &mut Self {
		self.connection_keep_alive = v;
		self
	}

	/// Sets the timeout for inbound and outbound requests.
	pub fn set_request_timeout(&mut self, v: Duration) -> &mut Self {
		self.request_timeout = v;
		self
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
	/// The protocol configuration.
	config: Config,
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
			config: Default::default(),
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
	type OutEvent = Event; // <<SpaceTimeCodec as Codec>::Request, <SpaceTimeCodec as Codec>::Response>;

	fn new_handler(&mut self) -> Self::ConnectionHandler {
		Handler::new(
			self.inbound_protocols.clone(),
			self.config.connection_keep_alive,
			self.config.request_timeout,
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
