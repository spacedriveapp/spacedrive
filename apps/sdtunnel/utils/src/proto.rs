use std::io::Read;

use serde::{Deserialize, Serialize};

use crate::PeerId;

/// MAX_MESSAGE_SIZE is the maximum size of a single message.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// MessageError is an error that occurs when a message is malformed.
/// NEVER REMOVE OR REORDER VARIANTS OF THIS ENUM OR YOU WILL BREAK STUFF DUE TO SUBOPTIMAL MSGPACK ENCODING.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageError {
	InvalidAuthErr,
	InvalidReqErr,
	InternalServerErr,
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
	QueryClientAnnouncement(PeerId),
	QueryClientAnnouncementResponse {
		peer_id: PeerId,
		addresses: Vec<String>,
	},
	Error(MessageError),
}

impl Message {
	/// encode will convert a message into it's binary form to be transmitted over the write.
	/// We are writing to Vec<u8> instead of directly to the network due to the mismatch of Write trait between `rmp_serde` and `quinn`. This is something that could be optimised in the future.
	pub fn encode(self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
		rmp_serde::encode::to_vec_named(&self)
	}

	/// read will read a message from it's binary form into it's Rust type.
	pub fn read<R: Read>(rd: &mut R) -> Result<Self, rmp_serde::decode::Error> {
		rmp_serde::decode::from_read(rd)
	}
}
