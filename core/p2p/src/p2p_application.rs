use crate::{PeerId, PeerMetadata};

/// TODO: Docs
/// TODO: Maybe rename?
pub trait P2PApplication {
	fn get_metadata(&self) -> PeerMetadata;

	fn can_peer_connection(&self, _: PeerId) -> bool;
}
