use std::collections::HashSet;

use sd_tunnel_utils::PeerId;

/// TODO
pub struct NetworkManagerConfig {
	/// known_peers contains a list of all the peers that were connected last time the application was running.
	/// These are used to know who to lookup using the global discovery service.
	pub known_peers: HashSet<PeerId>,
	/// TODO
	pub listen_port: Option<u16>,
}

impl Default for NetworkManagerConfig {
	fn default() -> Self {
		Self {
			known_peers: HashSet::new(),
			listen_port: None,
		}
	}
}
