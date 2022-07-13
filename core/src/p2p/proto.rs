use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum P2PRequest {
	Ping,
	GetFile { path: String }, // TODO: `path`should be converted to an ID in the final version to make it more secure.
}

#[derive(Debug, Serialize, Deserialize)]
pub enum P2PResponse {
	Pong,
	FileMetadata { path: String, size: u64 },
}
