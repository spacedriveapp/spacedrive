use std::{env, time::Duration};

use p2p::{Identity, NetworkManager, NetworkManagerConfig, P2PManager, Peer, PeerId, PeerMetadata};
use quinn::{RecvStream, SendStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

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
	// peer_name is the name of the current peer. In a normal application this would be a display name set by the end user.
	peer_name: String,
	/// event_channel is used to send events to the application
	event_channel: UnboundedSender<P2PEvent>,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			name: self.peer_name.clone(),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: &PeerId) {
		self.event_channel
			.send(P2PEvent::PeerConnected(peer_id.clone()));
		nm.add_known_peer(peer_id.clone()); // Be careful doing this in a production application because it will just trust all clients
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

#[tokio::main]
async fn main() {
	let identity = Identity::new().unwrap();
	let peer_id = PeerId::from_cert(&identity.clone().into_rustls().0);
	let mut event_channel = unbounded_channel();
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
			event_channel: event_channel.0,
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

	loop {
		tokio::select! {
			event = event_channel.1.recv() => {
				if let Some(event) = event {
					println!("{:?}", event);

					match event {
						P2PEvent::PeerConnected(peer_id) => {
							nm.send_to(peer_id, b"Ping on Connection").await.unwrap();
						}
						_ => {}
					}
				}
			}
			_ = tokio::time::sleep(Duration::from_secs(5)) => {
				println!("");
				for (peer_id, peer) in nm.connected_peers().await {
					println!("Sending ping to '{:?}'", peer_id);
					let resp = nm.send_to(peer_id, b"Ping").await.unwrap();
					println!("Peer '{}' responded to ping with message '{:?}'", peer.id, resp.bytes);
				}
			}
		};
	}
}
