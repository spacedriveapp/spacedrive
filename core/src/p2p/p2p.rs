use std::{env, sync::Arc};

use futures::executor::block_on;
use p2p::{
	quinn::{RecvStream, SendStream},
	Identity, NetworkManager, NetworkManagerConfig, NetworkManagerError, P2PManager, Peer, PeerId,
	PeerMetadata,
};
use tokio::sync::mpsc::{self};

use crate::node::NodeConfigManager;

#[derive(Debug, Clone)]
pub enum P2PEvent {
	PeerDiscovered(PeerId),
	PeerExpired(PeerId),
	PeerConnected(PeerId),
	PeerDisconnected(PeerId),
}

// SdP2PManager is part of your application and allows you to hook into the behavior of the P2PManager.
#[derive(Clone)]
pub struct SdP2PManager {
	config: Arc<NodeConfigManager>,
	/// event_channel is used to send events back to the Spacedrive main event loop
	event_channel: mpsc::UnboundedSender<P2PEvent>,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			// TODO: `block_on` needs to be removed from here!
			name: block_on(self.config.get()).name.clone(),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: &PeerId) {
		self.event_channel
			.send(P2PEvent::PeerDiscovered(peer_id.clone()));
		// nm.add_known_peer(peer_id.clone()); // Be careful doing this in a production application because it will just trust all clients
	}

	fn peer_expired(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerExpired(peer_id));
	}

	fn peer_connected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerConnected(peer_id));
	}

	fn peer_disconnected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerDisconnected(peer_id));
	}

	fn accept_stream(&self, peer: &Peer<Self>, (mut tx, mut rx): (SendStream, RecvStream)) {
		let peer = peer.clone();
		tokio::spawn(async move {
			let msg = rx.read_chunk(1024, true).await.unwrap().unwrap();
			println!("Received '{:?}' from peer '{}'", msg.bytes, peer.id);
			tx.write(b"Pong").await.unwrap();
		});
	}
}

pub async fn init(
	config: Arc<NodeConfigManager>,
) -> Result<
	(
		Arc<NetworkManager<SdP2PManager>>,
		mpsc::UnboundedReceiver<P2PEvent>,
	),
	NetworkManagerError,
> {
	let identity = Identity::new().unwrap(); // TODO: Save and load from Spacedrive config
	let event_channel = mpsc::unbounded_channel();
	let nm = NetworkManager::new(
		identity,
		SdP2PManager {
			config,
			event_channel: event_channel.0,
		},
		NetworkManagerConfig {
			known_peers: Default::default(),
			listen_port: None,
		},
	)
	.await?;
	println!(
		"Peer '{}' listening on: {:?}",
		nm.peer_id(),
		nm.listen_addr()
	);

	Ok((nm, event_channel.1))
}
