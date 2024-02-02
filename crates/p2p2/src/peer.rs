use std::{collections::HashMap, convert::Infallible};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::UnicastStream;

/// The status of the communication with a peer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub enum PeerStatus {
	/// The peer is not available for communication.
	/// We could be offline or they could be offline, blocked by a firewall or any other reason.
	Unavailable,
	/// We have discovered a method to connect to the peer.
	/// You can call [Peer::connect] to establish a connection.
	Discovered,
	/// We have an active connection with the peer.
	/// You can call [Peer::disconnect] to disconnect.
	Connected,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Peer {
	status: PeerStatus,
	service: HashMap<String, String>,

	connector: Vec<()>,
	// TODO: How to choice which mechanism to use for the connection? Maybe have a channel that's fired.
	// metadata

	// TODO: Avoid this `pub peer_id: PeerId,`
}

impl Peer {
	pub fn new() -> Self {
		Self {
			status: PeerStatus::Unavailable,
			service: Default::default(),
			connector: Default::default(),
		}
	}

	pub fn status(&self) -> PeerStatus {
		self.status
	}

	pub fn set_state(&mut self, state: PeerStatus) {
		self.status = state;
	}

	pub fn service(&self) -> &HashMap<String, String> {
		&self.service
	}

	// TODO: Mutex instead here???
	pub fn service_mut(&mut self) -> &mut HashMap<String, String> {
		&mut self.service
	}

	pub fn is_connected(&self) -> bool {
		todo!();
	}

	pub async fn connect(&self) -> Result<UnicastStream, Infallible> {
		todo!();
	}

	pub async fn disconnect(&self) {
		todo!();
	}
}
