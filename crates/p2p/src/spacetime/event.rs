use serde::{Deserialize, Serialize};
use thiserror::Error;

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
	/// Establish the connection
	Establish,

	/// Send data on behalf of application
	Application(Vec<u8>),
}

// TODO: Use the following error types or remove them!

/// Possible failures occurring in the context of sending
/// an outbound request and receiving the response.
#[derive(Debug, Error)]
pub enum OutboundFailure {}

/// Possible failures occurring in the context of receiving an
/// inbound request and sending a response.
#[derive(Debug, Error)]
pub enum InboundFailure {}
