use serde::{Deserialize, Serialize};

/// TODO: Remove this and replace it with the type from Brendan's sync library.
#[derive(Debug, Serialize, Deserialize)]
pub enum PlaceholderSyncMessage {}

#[derive(Debug, Serialize, Deserialize)]
pub enum P2PRequest {
	Ping,
	SyncMessage(PlaceholderSyncMessage),
	GetFile { path: String }, // TODO: `path` should be converted to an ID in the final version to make it more secure.
}

#[derive(Debug, Serialize, Deserialize)]
pub enum P2PResponse {
	Pong,
	FileMetadata { path: String, size: u64 },
}
