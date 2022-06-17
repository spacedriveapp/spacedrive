use std::{collections::HashMap, sync::Arc};

use rustls::{Certificate, PrivateKey};
use tokio::sync::{mpsc, RwLock};

use crate::{NetworkManagerEvent, P2PApplication, Peer, PeerId};

// TODO: Can some of this be moved onto the NetworkManger itself????
pub struct NetworkManagerState {
	/// PeerId is the unique identifier of the current node.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current node.
	pub(crate) identity: (Certificate, PrivateKey),
	/// application_channel is the channel that the NetworkManager will send events to so the application embedded the networking layer can react.
	pub(crate) application_channel: mpsc::Sender<NetworkManagerEvent>,
	/// connected_peers is a map of all the peers that have an established connection with the current node.
	pub(crate) connected_peers: RwLock<HashMap<PeerId, Peer>>, // TODO: Move back to DashMap????
	// p2p_application is a trait implemented by the application embedded the network manager. This allows the application to take control of the actions of the network manager.
	pub(crate) p2p_application: Arc<dyn P2PApplication + Send + Sync>,
}
