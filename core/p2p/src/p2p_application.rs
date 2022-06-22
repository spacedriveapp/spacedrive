use sd_tunnel_utils::PeerId;

use crate::PeerMetadata;

/// P2PApplication is a trait implementation by the application embedding sd-p2p and it allows the application to communicate with the networking layer.
pub trait P2PApplication {
	/// is called to get the metadata of the application. This is so the application can modify it at will.
	fn get_metadata(&self) -> PeerMetadata;

	/// is called when a peer attempts to establish a connection with the server. This event allows the application to decide if the connection should be accepted or rejected.
	/// DO NOT assume the connection succeeded for this event because it may not.
	fn can_peer_connection(&self, _: PeerId) -> bool;
}
