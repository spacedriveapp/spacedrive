use libp2p::swarm::{
	handler::{
		ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr,
		FullyNegotiatedOutbound, KeepAlive,
	},
	SubstreamProtocol,
};
use std::{
	collections::VecDeque,
	io,
	sync::Arc,
	task::{Context, Poll},
	time::Duration,
};

use crate::utils::AsyncFn2;

use super::{
	RequestProtocol, ResponseProtocol, SpaceTimeMessage, SpaceTimeState,
	EMPTY_QUEUE_SHRINK_THRESHOLD,
};

// TODO: Probs change this based on the connection type
const SUBSTREAM_TIMEOUT: Duration = Duration::from_secs(10); // TODO: Tune value
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(10); // TODO: Tune value

/// A connection handler for a request response [`Behaviour`](super::Behaviour) protocol.
pub struct Handler<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	state: Arc<SpaceTimeState<TMetadata, TConnFn>>,
	// request_id: Arc<AtomicU64>,
	// BREAK
	// /// The keep-alive timeout of idle connections. A connection is considered
	// /// idle if there are no outbound substreams.
	// keep_alive_timeout: Duration,
	// /// The timeout for inbound and outbound substreams (i.e. request
	// /// and response processing).
	// substream_timeout: Duration,
	// /// The current connection keep-alive.
	// keep_alive: KeepAlive,
	// /// A pending fatal error that results in the connection being closed.
	// pending_error: Option<ConnectionHandlerUpgrErr<io::Error>>,
	// /// Queue of events to emit in `poll()`.
	pending_events: VecDeque<
		ConnectionHandlerEvent<
			RequestProtocol,
			<Self as ConnectionHandler>::OutboundOpenInfo,
			<Self as ConnectionHandler>::OutEvent,
			<Self as ConnectionHandler>::Error,
		>,
	>,
	// /// Outbound upgrades waiting to be emitted as an `OutboundSubstreamRequest`.
	// outbound: VecDeque<RequestProtocol>,
	// /// Inbound upgrades waiting for the incoming request.
	// inbound: FuturesUnordered<
	// 	BoxFuture<
	// 		'static,
	// 		Result<
	// 			(
	// 				(RequestId, SpaceTimeMessage),
	// 				oneshot::Sender<SpaceTimeMessage>,
	// 			),
	// 			(), // oneshot::Canceled,
	// 		>,
	// 	>,
	// >,
	// inbound_request_id: Arc<AtomicU64>,
}

impl<TMetadata, TConnFn> Handler<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	pub(super) fn new(state: Arc<SpaceTimeState<TMetadata, TConnFn>>) -> Self {
		Self {
			state,
			pending_events: VecDeque::new(),
		}
	}
}

impl<TMetadata, TConnFn> ConnectionHandler for Handler<TMetadata, TConnFn>
where
	TMetadata: crate::Metadata,
	TConnFn: AsyncFn2<crate::Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
{
	type InEvent = SpaceTimeMessage;
	type OutEvent = SpaceTimeMessage;
	type Error = ConnectionHandlerUpgrErr<io::Error>;
	type InboundProtocol = ResponseProtocol<TMetadata, TConnFn>;
	type OutboundProtocol = RequestProtocol;
	type OutboundOpenInfo = ();
	type InboundOpenInfo = ();

	fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
		// A channel for notifying the handler when the inbound
		// upgrade received the request.
		// let (rq_send, rq_recv) = oneshot::channel();

		// A channel for notifying the inbound upgrade when the
		// response is sent.
		// let (rs_send, rs_recv) = oneshot::channel();

		// let request_id: RequestId = self.inbound_request_id.fetch_add(1, Ordering::Relaxed);

		// By keeping all I/O inside the `ResponseProtocol` and thus the
		// inbound substream upgrade via above channels, we ensure that it
		// is all subject to the configured timeout without extra bookkeeping
		// for inbound substreams as well as their timeouts and also make the
		// implementation of inbound and outbound upgrades symmetric in
		// this sense.
		// protocols: self.inbound_protocols.clone(),
		// request_sender: rq_send,
		// response_receiver: rs_recv,
		// request_id,

		// The handler waits for the request to come in. It then emits
		// `Event::Request` together with a
		// `ResponseChannel`.
		// self.inbound
		// 	.push(rq_recv.map_ok(move |rq| (rq, rs_send)).boxed());

		SubstreamProtocol::new(
			ResponseProtocol {
				state: self.state.clone(),
			},
			(), // TODO: Should I put some state here?
		)
		.with_timeout(SUBSTREAM_TIMEOUT)
	}

	fn on_behaviour_event(&mut self, request: Self::InEvent) {
		// self.keep_alive = KeepAlive::Yes;
		// self.outbound.push_back(request);

		self.pending_events
			.push_back(ConnectionHandlerEvent::OutboundSubstreamRequest {
				protocol: SubstreamProtocol::new(RequestProtocol { request: request }, ())
					.with_timeout(SUBSTREAM_TIMEOUT),
			});
	}

	fn connection_keep_alive(&self) -> KeepAlive {
		KeepAlive::Yes // TODO: Make this work how the old one did with storing it on `self` and updating on events
	}

	fn poll(
		&mut self,
		cx: &mut Context<'_>,
	) -> Poll<
		ConnectionHandlerEvent<
			RequestProtocol,
			Self::OutboundOpenInfo,
			Self::OutEvent,
			Self::Error,
		>,
	> {
		// // Check for a pending (fatal) error.
		// if let Some(err) = self.pending_error.take() {
		// 	// The handler will not be polled again by the `Swarm`.
		// 	return Poll::Ready(ConnectionHandlerEvent::Close(err));
		// }

		// Drain pending events.
		if let Some(event) = self.pending_events.pop_front() {
			return Poll::Ready(event);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

		// // Check for inbound requests.
		// while let Poll::Ready(Some(result)) = self.inbound.poll_next_unpin(cx) {
		// 	match result {
		// 		Ok(((id, rq), rs_sender)) => {
		// 			// We received an inbound request.
		// 			self.keep_alive = KeepAlive::Yes;
		// 			return Poll::Ready(ConnectionHandlerEvent::Custom(TODOEvent::Request {
		// 				request_id: id,
		// 				request: rq,
		// 				sender: rs_sender,
		// 			}));
		// 		}
		// 		Err(oneshot::Canceled) => {
		// 			// The inbound upgrade has errored or timed out reading
		// 			// or waiting for the request. The handler is informed
		// 			// via `inject_listen_upgrade_error`.
		// 		}
		// 	}
		// }

		// // Emit outbound requests.
		// if let Some(request) = self.outbound.pop_front() {
		// 	let info = request.request_id;
		// 	return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
		// 		protocol: SubstreamProtocol::new(request, info)
		// 			.with_timeout(self.substream_timeout),
		// 	});
		// }

		// debug_assert!(self.outbound.is_empty());

		// if self.outbound.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
		// 	self.outbound.shrink_to_fit();
		// }

		// if self.inbound.is_empty() && self.keep_alive.is_yes() {
		// 	// No new inbound or outbound requests. However, we may just have
		// 	// started the latest inbound or outbound upgrade(s), so make sure
		// 	// the keep-alive timeout is preceded by the substream timeout.
		// 	let until = Instant::now() + self.substream_timeout + self.keep_alive_timeout;
		// 	self.keep_alive = KeepAlive::Until(until);
		// }

		Poll::Pending
	}

	// TODO: Which level we doing error handler?. On swarm, on Behavior or here???
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
			ConnectionEvent::FullyNegotiatedInbound(event) => {
				// TODO
				// if sent {
				// 	self.pending_events
				// 		.push_back(TODOEvent::ResponseSent(event.request_id))
				// } else {
				// 	self.pending_events
				// 		.push_back(TODOEvent::ResponseOmission(event.request_id))
				// }
			}
			ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
				protocol: response,
				info: request_id,
			}) => {
				// TODO
				// 		self.pending_events.push_back(TODOEvent::Response {
				// 			request_id,
				// 			response,
				// 		});

				// 	// match error {
				// 	// 	ConnectionHandlerUpgrErr::Timeout => {
				// 	// 		self.pending_events
				// 	// 			.push_back(TODOEvent::OutboundTimeout(info));
				// 	// 	}
				// 	// 	ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
				// 	// 		// The remote merely doesn't support the protocol(s) we requested.
				// 	// 		// This is no reason to close the connection, which may
				// 	// 		// successfully communicate with other protocols already.
				// 	// 		// An event is reported to permit user code to react to the fact that
				// 	// 		// the remote peer does not support the requested protocol(s).
				// 	// 		self.pending_events
				// 	// 			.push_back(TODOEvent::OutboundUnsupportedProtocols(info));
				// 	// 	}
				// 	// 	_ => {
				// 	// 		// Anything else is considered a fatal error or misbehaviour of
				// 	// 		// the remote peer and results in closing the connection.
				// 	// 		self.pending_error = Some(error);
				// 	// 	}
				// 	// }
			}
			ConnectionEvent::DialUpgradeError(event) => {
				// error!("DialUpgradeError: {:#?}", event); // TODO: Better message

				// self.on_dial_upgrade_error(event) // TODO
			}
			ConnectionEvent::ListenUpgradeError(event) => {
				// error!("DialUpgradeError: {:#?}", event); // TODO: Better message

				// TODO
				// match error {
				// 	ConnectionHandlerUpgrErr::Timeout => self
				// 		.pending_events
				// 		.push_back(TODOEvent::InboundTimeout(info)),
				// 	ConnectionHandlerUpgrErr::Upgrade(UpgradeError::Select(NegotiationError::Failed)) => {
				// 		// The local peer merely doesn't support the protocol(s) requested.
				// 		// This is no reason to close the connection, which may
				// 		// successfully communicate with other protocols already.
				// 		// An event is reported to permit user code to react to the fact that
				// 		// the local peer does not support the requested protocol(s).
				// 		self.pending_events
				// 			.push_back(TODOEvent::InboundUnsupportedProtocols(info));
				// 	}
				// 	_ => {
				// 		// Anything else is considered a fatal error or misbehaviour of
				// 		// the remote peer and results in closing the connection.
				// 		self.pending_error = Some(error);
				// 	}
				// }
			}
			ConnectionEvent::AddressChange(_) => {}
		}
	}
}
