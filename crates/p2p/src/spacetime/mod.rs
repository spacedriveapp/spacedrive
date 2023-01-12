//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application built on this library.

mod behaviour;
mod event;
pub(crate) mod handler;
mod protocol;

pub use behaviour::*;
pub use event::*;
pub use protocol::*;

/// A channel for sending a response to an inbound request.
///
/// See [`Behaviour::send_response`].
#[derive(Debug)]
pub struct ResponseChannel<TResponse> {
	sender: tokio::sync::oneshot::Sender<TResponse>,
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
