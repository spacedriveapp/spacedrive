//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application build on this library.

use futures::channel::oneshot;
use libp2p::{core::connection::ConnectionId, Multiaddr};
use std::{collections::HashSet, fmt};

mod behaviour;
mod event;
mod handler;
mod protocol;

pub use behaviour::*;
pub use event::*;
pub use handler::*;
pub use protocol::*;

/// A channel for sending a response to an inbound request.
///
/// See [`Behaviour::send_response`].
#[derive(Debug)]
pub struct ResponseChannel<TResponse> {
	sender: oneshot::Sender<TResponse>, // TODO: Move to tokio channel
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
