use serde::{Deserialize, Serialize};

/// Action/Query envelopes for client-agnostic RPC
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
	Ping,
	Action { type_id: String, payload: Vec<u8> },
	Query { type_id: String, payload: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
	Pong,
	Ok(Vec<u8>),
	Error(String),
}

/// Well-known type identifiers for built-in actions/queries
pub mod type_ids {
    pub const FILE_COPY_INPUT: &str = "action:files.copy.input.v1";
    pub const INDEX_INPUT: &str = "action:indexing.input.v1";
    pub const LOCATION_RESCAN_ACTION: &str = "action:locations.rescan.v1";
    pub const LIST_LIBRARIES_QUERY: &str = "query:libraries.list.v1";
    pub const CORE_STATUS_QUERY: &str = "query:core.status.v1";
}


