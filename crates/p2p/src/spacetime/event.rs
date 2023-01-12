use std::{fmt, sync::Arc};

use libp2p::{swarm::derive_prelude::ConnectionId, PeerId};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::warn;

use crate::{utils::AsyncFn2, ManagerEvent, ManagerRef, Metadata};

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

/// TODO
#[derive(Debug)]
pub struct SpaceTimeEvent {}

impl SpaceTimeEvent {}

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

/// The events emitted by the [`Handler`].
pub enum TODOEvent {
	/// A request has been received.
	Request {
		request_id: RequestId,
		request: SpaceTimeMessage,
		sender: oneshot::Sender<SpaceTimeMessage>,
	},
	/// A response has been received.
	Response {
		request_id: RequestId,
		response: SpaceTimeMessage,
	},
	/// A response to an inbound request has been sent.
	ResponseSent(RequestId),
	/// A response to an inbound request was omitted as a result
	/// of dropping the response `sender` of an inbound `Request`.
	ResponseOmission(RequestId),
	/// An outbound request timed out while sending the request
	/// or waiting for the response.
	OutboundTimeout(RequestId),
	/// An outbound request failed to negotiate a mutually supported `.
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

impl TODOEvent {
	pub fn handle(&self, peer_id: PeerId, connection: ConnectionId) {
		match self {
			Self::Response {
				request_id,
				response,
			} => {
				// 		let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				// 		debug_assert!(
				// 			removed,
				// 			"Expect request_id to be pending before receiving response.",
				// 		);

				// 		let message = Message::Response {
				// 			request_id,
				// 			response,
				// 		};
				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
				// 				peer,
				// 				message,
				// 			}));
			}
			Self::Request {
				request_id,
				request,
				sender,
			} => {
				// 		let channel = ResponseChannel { sender };
				// 		let message = Message::Request {
				// 			request_id,
				// 			request,
				// 			channel,
				// 		};
				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(Event::Message {
				// 				peer,
				// 				message,
				// 			}));

				// 		match self.get_connection_mut(&peer, connection) {
				// 			Some(connection) => {
				// 				let inserted = connection.pending_outbound_responses.insert(request_id);
				// 				debug_assert!(inserted, "Expect id of new request to be unknown.");
				// 			}
				// 			// Connection closed after `Event::Request` has been emitted.
				// 			None => {
				// 				self.pending_events
				// 					.push_back(NetworkBehaviourAction::GenerateEvent(
				// 						Event::InboundFailure {
				// 							peer,
				// 							request_id,
				// 							error: InboundFailure::ConnectionClosed,
				// 						},
				// 					));
				// 			}
				// 		}
			}
			Self::ResponseSent(request_id) => {
				// 		let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
				// 		debug_assert!(
				// 			removed,
				// 			"Expect request_id to be pending before response is sent."
				// 		);

				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(Event::ResponseSent {
				// 				peer,
				// 				request_id,
				// 			}));
			}
			Self::ResponseOmission(request_id) => {
				// 		let removed = self.remove_pending_outbound_response(&peer, connection, request_id);
				// 		debug_assert!(
				// 			removed,
				// 			"Expect request_id to be pending before response is omitted.",
				// 		);

				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(
				// 				Event::InboundFailure {
				// 					peer,
				// 					request_id,
				// 					error: InboundFailure::ResponseOmission,
				// 				},
				// 			));
			}
			Self::OutboundTimeout(request_id) => {
				// 		let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				// 		debug_assert!(
				// 			removed,
				// 			"Expect request_id to be pending before request times out."
				// 		);

				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(
				// 				Event::OutboundFailure {
				// 					peer,
				// 					request_id,
				// 					error: OutboundFailure::Timeout,
				// 				},
				// 			));
			}
			Self::InboundTimeout(request_id) => {
				// 		// Note: `Event::InboundTimeout` is emitted both for timing
				// 		// out to receive the request and for timing out sending the response. In the former
				// 		// case the request is never added to `pending_outbound_responses` and thus one can
				// 		// not assert the request_id to be present before removing it.
				// 		self.remove_pending_outbound_response(&peer, connection, request_id);

				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(
				// 				Event::InboundFailure {
				// 					peer,
				// 					request_id,
				// 					error: InboundFailure::Timeout,
				// 				},
				// 			));
			}
			Self::OutboundUnsupportedProtocols(request_id) => {
				// 		let removed = self.remove_pending_inbound_response(&peer, connection, &request_id);
				// 		debug_assert!(
				// 			removed,
				// 			"Expect request_id to be pending before failing to connect.",
				// 		);

				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(
				// 				Event::OutboundFailure {
				// 					peer,
				// 					request_id,
				// 					error: OutboundFailure::UnsupportedProtocols,
				// 				},
				// 			));
			}
			Self::InboundUnsupportedProtocols(request_id) => {
				// 		// Note: No need to call `self.remove_pending_outbound_response`,
				// 		// `Event::Request` was never emitted for this request and
				// 		// thus request was never added to `pending_outbound_responses`.
				// 		self.pending_events
				// 			.push_back(NetworkBehaviourAction::GenerateEvent(
				// 				Event::InboundFailure {
				// 					peer,
				// 					request_id,
				// 					error: InboundFailure::UnsupportedProtocols,
				// 				},
				// 			));
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
