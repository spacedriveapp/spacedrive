use std::{fmt, sync::Arc};

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::warn;

use crate::{utils::AsyncFn2, ManagerRef, Metadata};

use super::{RequestId, ResponseChannel};

/// TODO
pub type SpaceTimeResponseChan = oneshot::Sender<Result<SpaceTimeMessage, OutboundFailure>>;

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
	/// Establish the connection
	Establish,

	/// Send data on behalf of application
	Application(Vec<u8>),
}

// TODO: Avoiding this impl because it provides no ability to do error handling
impl AsRef<[u8]> for SpaceTimeMessage {
	fn as_ref(&self) -> &[u8] {
		todo!()
	}
}

/// The events emitted by a request-response [`Behaviour`].
#[derive(Debug)]
pub enum Event {
	/// An incoming message (request or response).
	Message {
		/// The peer who sent the message.
		peer: PeerId,
		/// The incoming message.
		message: Message,
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
		// active_requests: &mut HashMap<
		// 	RequestId,
		// 	tokio::sync::oneshot::Sender<Result<SpaceTimeMessage, OutboundFailure>>,
		// >,
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
											// match state
											// 	.internal_tx
											// 	.send(ManagerEvent::SendResponse(
											// 		peer,
											// 		SpaceTimeMessage::Application(data),
											// 		channel,
											// 	))
											// 	.await
											// {
											// 	Ok(_) => {}
											// 	Err(_err) => todo!(),
											// }

											todo!();
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
					} => {
						todo!();
						// match active_requests.remove(&request_id) {
						// 	Some(resp) => resp.send(Ok(response)).unwrap(),
						// 	None => warn!(
						// 		"error unable to find destination for response id '{:?}'",
						// 		request_id
						// 	),
						// }
					}
				}
			}
			Self::OutboundFailure {
				peer,
				request_id,
				error,
			} => {
				todo!();
				// match active_requests.remove(&request_id) {
				// 	Some(resp) => resp.send(Err(error)).unwrap(),
				// 	None => warn!(
				// 		"error with onbound request '{:?}' to peer '{:?}': '{:?}'",
				// 		request_id, peer, error
				// 	),
				// }
			}
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

/// An inbound request or response.
#[derive(Debug)]
pub enum Message {
	/// A request message.
	Request {
		/// The ID of this request.
		request_id: RequestId,
		/// The request message.
		request: SpaceTimeMessage,
		/// The channel waiting for the response.
		///
		/// If this channel is dropped instead of being used to send a response
		/// via [`Behaviour::send_response`], a [`Event::InboundFailure`]
		/// with [`InboundFailure::ResponseOmission`] is emitted.
		channel: ResponseChannel<SpaceTimeMessage>,
	},
	/// A response message.
	Response {
		/// The ID of the request that produced this response.
		///
		/// See [`Behaviour::send_request`].
		request_id: RequestId,
		/// The response message.
		response: SpaceTimeMessage,
	},
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
