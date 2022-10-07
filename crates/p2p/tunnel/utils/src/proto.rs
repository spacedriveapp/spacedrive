use serde::{Deserialize, Serialize};

use crate::PeerId;

/// MessageError is an error that occurs when a message is malformed.
/// NEVER REMOVE OR REORDER VARIANTS OF THIS ENUM OR YOU WILL BREAK STUFF DUE TO SUBOPTIMAL MSGPACK ENCODING.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageError {
	InvalidAuthErr,
	InvalidReqErr,
	InternalServerErr,
}

/// ClientAnnouncementResponse is returned by the server when a client queries for an announcement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientAnnouncementResponse {
	pub peer_id: PeerId,
	pub addresses: Vec<String>,
}

/// Message is a single request that is sent between a client and the Spacetunnel server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
	// Announce your current device addresses
	ClientAnnouncement {
		peer_id: PeerId,
		addresses: Vec<String>,
	},
	ClientAnnouncementOk,
	// Query for an existing client announcement
	QueryClientAnnouncement(Vec<PeerId>),
	QueryClientAnnouncementResponse(Vec<ClientAnnouncementResponse>),
	Error(MessageError),
}
