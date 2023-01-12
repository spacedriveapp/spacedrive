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

mod protocol;

// use crate::codec::Codec;
// use crate::{RequestId, EMPTY_QUEUE_SHRINK_THRESHOLD};

pub use protocol::{ProtocolSupport, RequestProtocol, ResponseProtocol};

use futures::{channel::oneshot, future::BoxFuture, prelude::*, stream::FuturesUnordered};
use instant::Instant;
use libp2p::{
	core::upgrade::{NegotiationError, UpgradeError},
	swarm::{
		handler::{
			ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr,
			DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound, KeepAlive,
			ListenUpgradeError,
		},
		SubstreamProtocol,
	},
};
use smallvec::SmallVec;
use std::{
	collections::VecDeque,
	fmt, io,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
	task::{Context, Poll},
	time::Duration,
};

use crate::spacetime::EMPTY_QUEUE_SHRINK_THRESHOLD;

use super::{Codec, RequestId, SpaceTimeCodec};

/// A connection handler for a request response [`Behaviour`](super::Behaviour) protocol.
pub struct Handler {
	/// The supported inbound protocols.
	inbound_protocols: SmallVec<[<SpaceTimeCodec as Codec>::Protocol; 2]>,
	/// The request/response message codec.
	codec: SpaceTimeCodec,
	/// The keep-alive timeout of idle connections. A connection is considered
	/// idle if there are no outbound substreams.
	keep_alive_timeout: Duration,
	/// The timeout for inbound and outbound substreams (i.e. request
	/// and response processing).
	substream_timeout: Duration,
	/// The current connection keep-alive.
	keep_alive: KeepAlive,
	/// A pending fatal error that results in the connection being closed.
	pending_error: Option<ConnectionHandlerUpgrErr<io::Error>>,
	/// Queue of events to emit in `poll()`.
	pending_events: VecDeque<TODOEvent>,
	/// Outbound upgrades waiting to be emitted as an `OutboundSubstreamRequest`.
	outbound: VecDeque<RequestProtocol>,
	/// Inbound upgrades waiting for the incoming request.
	inbound: FuturesUnordered<
		BoxFuture<
			'static,
			Result<
				(
					(RequestId, <SpaceTimeCodec as Codec>::Request),
					oneshot::Sender<<SpaceTimeCodec as Codec>::Response>,
				),
				oneshot::Canceled,
			>,
		>,
	>,
	inbound_request_id: Arc<AtomicU64>,
}

impl Handler {
	pub(super) fn new(
		inbound_protocols: SmallVec<[<SpaceTimeCodec as Codec>::Protocol; 2]>,
		codec: SpaceTimeCodec,
		keep_alive_timeout: Duration,
		substream_timeout: Duration,
		inbound_request_id: Arc<AtomicU64>,
	) -> Self {
		Self {
			inbound_protocols,
			codec,
			keep_alive: KeepAlive::Yes,
			keep_alive_timeout,
			substream_timeout,
			outbound: VecDeque::new(),
			inbound: FuturesUnordered::new(),
			pending_events: VecDeque::new(),
			pending_error: None,
			inbound_request_id,
		}
	}

	fn on_fully_negotiated_inbound(
		&mut self,
		FullyNegotiatedInbound {
			protocol: sent,
			info: request_id,
		}: FullyNegotiatedInbound<
			<Self as ConnectionHandler>::InboundProtocol,
			<Self as ConnectionHandler>::InboundOpenInfo,
		>,
	) {
		if sent {
			self.pending_events
				.push_back(TODOEvent::ResponseSent(request_id))
		} else {
			self.pending_events
				.push_back(TODOEvent::ResponseOmission(request_id))
		}
	}

	fn on_dial_upgrade_error(
		&mut self,
		DialUpgradeError { info, error }: DialUpgradeError<
			<Self as ConnectionHandler>::OutboundOpenInfo,
			<Self as ConnectionHandler>::OutboundProtocol,
		>,
	) {
		match error {
			ConnectionHandlerUpgrErr::Timeout => {
				self.pending_events
					.push_back(TODOEvent::OutboundTimeout(info));
			}
			ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
				// The remote merely doesn't support the protocol(s) we requested.
				// This is no reason to close the connection, which may
				// successfully communicate with other protocols already.
				// An event is reported to permit user code to react to the fact that
				// the remote peer does not support the requested protocol(s).
				self.pending_events
					.push_back(TODOEvent::OutboundUnsupportedProtocols(info));
			}
			_ => {
				// Anything else is considered a fatal error or misbehaviour of
				// the remote peer and results in closing the connection.
				self.pending_error = Some(error);
			}
		}
	}
	fn on_listen_upgrade_error(
		&mut self,
		ListenUpgradeError { info, error }: ListenUpgradeError<
			<Self as ConnectionHandler>::InboundOpenInfo,
			<Self as ConnectionHandler>::InboundProtocol,
		>,
	) {
		match error {
			ConnectionHandlerUpgrErr::Timeout => self
				.pending_events
				.push_back(TODOEvent::InboundTimeout(info)),
			ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
				// The local peer merely doesn't support the protocol(s) requested.
				// This is no reason to close the connection, which may
				// successfully communicate with other protocols already.
				// An event is reported to permit user code to react to the fact that
				// the local peer does not support the requested protocol(s).
				self.pending_events
					.push_back(TODOEvent::InboundUnsupportedProtocols(info));
			}
			_ => {
				// Anything else is considered a fatal error or misbehaviour of
				// the remote peer and results in closing the connection.
				self.pending_error = Some(error);
			}
		}
	}
}

/// The events emitted by the [`Handler`].
pub enum TODOEvent {
	/// A request has been received.
	Request {
		request_id: RequestId,
		request: <SpaceTimeCodec as Codec>::Request,
		sender: oneshot::Sender<<SpaceTimeCodec as Codec>::Response>,
	},
	/// A response has been received.
	Response {
		request_id: RequestId,
		response: <SpaceTimeCodec as Codec>::Response,
	},
	/// A response to an inbound request has been sent.
	ResponseSent(RequestId),
	/// A response to an inbound request was omitted as a result
	/// of dropping the response `sender` of an inbound `Request`.
	ResponseOmission(RequestId),
	/// An outbound request timed out while sending the request
	/// or waiting for the response.
	OutboundTimeout(RequestId),
	/// An outbound request failed to negotiate a mutually supported protocol.
	OutboundUnsupportedProtocols(RequestId),
	/// An inbound request timed out while waiting for the request
	/// or sending the response.
	InboundTimeout(RequestId),
	/// An inbound request failed to negotiate a mutually supported protocol.
	InboundUnsupportedProtocols(RequestId),
}

impl fmt::Debug for TODOEvent {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			TODOEvent::Request {
				request_id,
				request: _,
				sender: _,
			} => f
				.debug_struct("Event::Request")
				.field("request_id", request_id)
				.finish(),
			TODOEvent::Response {
				request_id,
				response: _,
			} => f
				.debug_struct("Event::Response")
				.field("request_id", request_id)
				.finish(),
			TODOEvent::ResponseSent(request_id) => f
				.debug_tuple("Event::ResponseSent")
				.field(request_id)
				.finish(),
			TODOEvent::ResponseOmission(request_id) => f
				.debug_tuple("Event::ResponseOmission")
				.field(request_id)
				.finish(),
			TODOEvent::OutboundTimeout(request_id) => f
				.debug_tuple("Event::OutboundTimeout")
				.field(request_id)
				.finish(),
			TODOEvent::OutboundUnsupportedProtocols(request_id) => f
				.debug_tuple("Event::OutboundUnsupportedProtocols")
				.field(request_id)
				.finish(),
			TODOEvent::InboundTimeout(request_id) => f
				.debug_tuple("Event::InboundTimeout")
				.field(request_id)
				.finish(),
			TODOEvent::InboundUnsupportedProtocols(request_id) => f
				.debug_tuple("Event::InboundUnsupportedProtocols")
				.field(request_id)
				.finish(),
		}
	}
}

impl ConnectionHandler for Handler {
	type InEvent = RequestProtocol;
	type OutEvent = TODOEvent;
	type Error = ConnectionHandlerUpgrErr<io::Error>;
	type InboundProtocol = ResponseProtocol;
	type OutboundProtocol = RequestProtocol;
	type OutboundOpenInfo = RequestId;
	type InboundOpenInfo = RequestId;

	fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
		// A channel for notifying the handler when the inbound
		// upgrade received the request.
		let (rq_send, rq_recv) = oneshot::channel();

		// A channel for notifying the inbound upgrade when the
		// response is sent.
		let (rs_send, rs_recv) = oneshot::channel();

		let request_id = RequestId(self.inbound_request_id.fetch_add(1, Ordering::Relaxed));

		// By keeping all I/O inside the `ResponseProtocol` and thus the
		// inbound substream upgrade via above channels, we ensure that it
		// is all subject to the configured timeout without extra bookkeeping
		// for inbound substreams as well as their timeouts and also make the
		// implementation of inbound and outbound upgrades symmetric in
		// this sense.
		let proto = ResponseProtocol {
			protocols: self.inbound_protocols.clone(),
			codec: self.codec.clone(),
			request_sender: rq_send,
			response_receiver: rs_recv,
			request_id,
		};

		// The handler waits for the request to come in. It then emits
		// `Event::Request` together with a
		// `ResponseChannel`.
		self.inbound
			.push(rq_recv.map_ok(move |rq| (rq, rs_send)).boxed());

		SubstreamProtocol::new(proto, request_id).with_timeout(self.substream_timeout)
	}

	fn on_behaviour_event(&mut self, request: Self::InEvent) {
		self.keep_alive = KeepAlive::Yes;
		self.outbound.push_back(request);
	}

	fn connection_keep_alive(&self) -> KeepAlive {
		self.keep_alive
	}

	fn poll(
		&mut self,
		cx: &mut Context<'_>,
	) -> Poll<ConnectionHandlerEvent<RequestProtocol, RequestId, Self::OutEvent, Self::Error>> {
		// Check for a pending (fatal) error.
		if let Some(err) = self.pending_error.take() {
			// The handler will not be polled again by the `Swarm`.
			return Poll::Ready(ConnectionHandlerEvent::Close(err));
		}

		// Drain pending events.
		if let Some(event) = self.pending_events.pop_front() {
			return Poll::Ready(ConnectionHandlerEvent::Custom(event));
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		// Check for inbound requests.
		while let Poll::Ready(Some(result)) = self.inbound.poll_next_unpin(cx) {
			match result {
				Ok(((id, rq), rs_sender)) => {
					// We received an inbound request.
					self.keep_alive = KeepAlive::Yes;
					return Poll::Ready(ConnectionHandlerEvent::Custom(TODOEvent::Request {
						request_id: id,
						request: rq,
						sender: rs_sender,
					}));
				}
				Err(oneshot::Canceled) => {
					// The inbound upgrade has errored or timed out reading
					// or waiting for the request. The handler is informed
					// via `inject_listen_upgrade_error`.
				}
			}
		}

		// Emit outbound requests.
		if let Some(request) = self.outbound.pop_front() {
			let info = request.request_id;
			return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
				protocol: SubstreamProtocol::new(request, info)
					.with_timeout(self.substream_timeout),
			});
		}

		debug_assert!(self.outbound.is_empty());

		if self.outbound.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.outbound.shrink_to_fit();
		}

		if self.inbound.is_empty() && self.keep_alive.is_yes() {
			// No new inbound or outbound requests. However, we may just have
			// started the latest inbound or outbound upgrade(s), so make sure
			// the keep-alive timeout is preceded by the substream timeout.
			let until = Instant::now() + self.substream_timeout + self.keep_alive_timeout;
			self.keep_alive = KeepAlive::Until(until);
		}

		Poll::Pending
	}

	fn on_connection_event(
		&mut self,
		event: ConnectionEvent<
			Self::InboundProtocol,
			Self::OutboundProtocol,
			Self::InboundOpenInfo,
			Self::OutboundOpenInfo,
		>,
	) {
		match event {
			ConnectionEvent::FullyNegotiatedInbound(fully_negotiated_inbound) => {
				self.on_fully_negotiated_inbound(fully_negotiated_inbound)
			}
			ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
				protocol: response,
				info: request_id,
			}) => {
				self.pending_events.push_back(TODOEvent::Response {
					request_id,
					response,
				});
			}
			ConnectionEvent::DialUpgradeError(dial_upgrade_error) => {
				self.on_dial_upgrade_error(dial_upgrade_error)
			}
			ConnectionEvent::ListenUpgradeError(listen_upgrade_error) => {
				self.on_listen_upgrade_error(listen_upgrade_error)
			}
			ConnectionEvent::AddressChange(_) => {}
		}
	}
}
