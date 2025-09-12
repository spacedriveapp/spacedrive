use serde::{Deserialize, Serialize};

/// Action/Query envelopes for client-agnostic RPC
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
	Ping,
	Action { method: String, payload: Vec<u8> },
	Query { method: String, payload: Vec<u8> },
	Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
	Pong,
	Ok(Vec<u8>),
	Error(String),
}
