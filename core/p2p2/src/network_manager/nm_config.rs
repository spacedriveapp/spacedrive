use std::collections::HashSet;

use sd_tunnel_utils::PeerId;

/// Stores configuration which is given to the [crate::NetworkManager] at startup so it can resume from it's previous state.
pub struct NetworkManagerConfig {
	/// known_peers contains a list of all the peers that were connected last time the application was running.
	/// These are used to know who to lookup when using the global discovery service.
	pub known_peers: HashSet<PeerId>,
	/// listen_port allows the user to specify which port to listen on for incoming connections.
	/// By default the network manager will listen on a random free port which changes every time the application is restarted.
	pub listen_port: Option<u16>,
	/// TODO
	pub spacetunnel_url: Option<String>,
}

impl Default for NetworkManagerConfig {
	fn default() -> Self {
		Self {
			known_peers: HashSet::new(),
			listen_port: None,
			spacetunnel_url: None,
		}
	}
}
