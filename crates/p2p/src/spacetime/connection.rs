use libp2p::swarm::{
	handler::{
		ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr,
		FullyNegotiatedInbound, KeepAlive,
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
use tracing::error;

use crate::{Manager, ManagerStreamAction2, Metadata, PeerId};

use super::{InboundProtocol, OutboundProtocol, OutboundRequest, EMPTY_QUEUE_SHRINK_THRESHOLD};

// TODO: Probs change this based on the ConnectionEstablishmentPayload
const SUBSTREAM_TIMEOUT: Duration = Duration::from_secs(10); // TODO: Tune value

#[allow(clippy::type_complexity)]
pub struct SpaceTimeConnection<TMetadata: Metadata> {
	peer_id: PeerId,
	manager: Arc<Manager<TMetadata>>,
	pending_events: VecDeque<
		ConnectionHandlerEvent<
			OutboundProtocol,
			<Self as ConnectionHandler>::OutboundOpenInfo,
			<Self as ConnectionHandler>::OutEvent,
			<Self as ConnectionHandler>::Error,
		>,
	>,
}

impl<TMetadata: Metadata> SpaceTimeConnection<TMetadata> {
	pub(super) fn new(peer_id: PeerId, manager: Arc<Manager<TMetadata>>) -> Self {
		Self {
			peer_id,
			manager,
			pending_events: VecDeque::new(),
		}
	}
}

// pub enum Connection

impl<TMetadata: Metadata> ConnectionHandler for SpaceTimeConnection<TMetadata> {
	type InEvent = OutboundRequest;
	type OutEvent = ManagerStreamAction2<TMetadata>;
	type Error = ConnectionHandlerUpgrErr<io::Error>;
	type InboundProtocol = InboundProtocol<TMetadata>;
	type OutboundProtocol = OutboundProtocol;
	type OutboundOpenInfo = ();
	type InboundOpenInfo = ();

	fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
		SubstreamProtocol::new(
			InboundProtocol {
				peer_id: self.peer_id,
				manager: self.manager.clone(),
			},
			(),
		)
		.with_timeout(SUBSTREAM_TIMEOUT)
	}

	fn on_behaviour_event(&mut self, req: Self::InEvent) {
		// TODO: Working keep alives
		// self.keep_alive = KeepAlive::Yes;
		// self.outbound.push_back(request);

		self.pending_events
			.push_back(ConnectionHandlerEvent::OutboundSubstreamRequest {
				protocol: SubstreamProtocol::new(
					OutboundProtocol(self.manager.application_name, req),
					(),
				) // TODO: Use `info` here maybe to pass into about the client. Idk?
				.with_timeout(SUBSTREAM_TIMEOUT),
			});
	}

	fn connection_keep_alive(&self) -> KeepAlive {
		KeepAlive::Yes // TODO: Make this work how the old one did with storing it on `self` and updating on events
	}

	fn poll(
		&mut self,
		_cx: &mut Context<'_>,
	) -> Poll<
		ConnectionHandlerEvent<
			Self::OutboundProtocol,
			Self::OutboundOpenInfo,
			Self::OutEvent,
			Self::Error,
		>,
	> {
		if let Some(event) = self.pending_events.pop_front() {
			return Poll::Ready(event);
		} else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
			self.pending_events.shrink_to_fit();
		}

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
			ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound {
				protocol, ..
			}) => {
				self.pending_events
					.push_back(ConnectionHandlerEvent::Custom(protocol));
			}
			ConnectionEvent::FullyNegotiatedOutbound(_) => {}
			ConnectionEvent::DialUpgradeError(event) => {
				error!("DialUpgradeError: {:#?}", event.error);
			}
			ConnectionEvent::ListenUpgradeError(event) => {
				error!("DialUpgradeError: {:#?}", event.error);

				// TODO: If `event.error` close connection cause we don't "speak the same language"!
			}
			ConnectionEvent::AddressChange(_) => {
				// TODO: Should we be telling `SpaceTime` to update it's info here or is it also getting this event?
			}
		}
	}
}
