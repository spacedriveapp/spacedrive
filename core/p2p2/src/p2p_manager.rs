use sd_tunnel_utils::PeerId;

use crate::{NetworkManager, PeerMetadata};

/// TODO
pub trait P2PManager: Send + Sync + Sized + 'static {
	const APPLICATION_NAME: &'static str;

	/// is called to get the metadata of the application. This metadata is sent as part of the discovery payload.
	fn get_metadata(&self) -> PeerMetadata;

	// /// TODO
	// fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	// /// TODO
	// fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	// /// TODO
	// fn peer_connected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	// /// TODO
	// fn peer_disconnected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}
}
