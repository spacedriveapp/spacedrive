use std::env;

use p2p::{Identity, NetworkManager, NetworkManagerConfig, P2PManager, PeerId, PeerMetadata};

pub struct SdP2PManager {
	// peer_name is the name of the current peer. In a normal application this would be a display name set by the end user.
	peer_name: String,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			name: self.peer_name.clone(),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	// fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}
}

#[tokio::main]
async fn main() {
	let identity = Identity::new().unwrap();
	let peer_id = PeerId::from_cert(&identity.clone().into_rustls().0);
	let nm = NetworkManager::new(
		identity,
		SdP2PManager {
			peer_name: format!(
				"{}-{}",
				peer_id
					.to_string()
					.chars()
					.into_iter()
					.take(5)
					.collect::<String>(),
				env::consts::OS
			),
		},
		NetworkManagerConfig {
			known_peers: Default::default(),
			listen_port: None,
		},
	)
	.await
	.unwrap();
	println!(
		"Peer '{}' listening on: {:?}",
		nm.peer_id(),
		nm.listen_addr()
	);

	tokio::time::sleep(std::time::Duration::from_secs(30)).await; // TODO: Remove
}
