use std::time::Duration;

use sd_p2p::{Identity, NetworkManager, NetworkManagerEvent, P2PApplication, PeerId, PeerMetadata};
use tokio::sync::mpsc;

// APPLICATION_NAME is name of the application embedding 'sd-p2p'. This is used to identify the application during discovery.
const APPLICATION_NAME: &'static str = "spacedrive";

pub struct SpacedrivePeer {}

impl P2PApplication for SpacedrivePeer {
	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			name: "demo-computer".into(),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	fn can_peer_connection(&self, peer_id: PeerId) -> bool {
		println!("Allowing peer '{}' to connect?", peer_id);
		true /* If peer exists in a loaded Spacedrive library */
	}
}

#[tokio::main]
async fn main() {
	// Create a new Identity to represent the current node. In a normal application you would save the identity to a file and load it from there instead of generating a new one of every restart.
	let identity = Identity::new().unwrap();

	// Create a new NetworkManager which will handle all the P2P networking for the application.
	let (tx, mut rx) = mpsc::channel(100);
	let nm = NetworkManager::new(APPLICATION_NAME, SpacedrivePeer {}, identity, tx)
		.await
		.unwrap();
	println!(
		"Spacedrive P2P '{}' is listening on '{}'!",
		nm.peer_id(),
		nm.listen_addr().to_string()
	);

	loop {
		tokio::select! {
			// Handle all events coming from the NetworkManager.
			event = rx.recv() => {
				match event {
					Some(event) => match event {
						NetworkManagerEvent::PeerDiscovered { peer } => {
							println!("PeerDiscovered: {:?}", peer);
							if true /* Peer exists in loaded library */ {
								nm.connect(peer).await.unwrap();
							}
						}
						NetworkManagerEvent::ConnectionRequest { peer_id, resp } => {
							println!("ConnectionRequest from peer '{}'!", peer_id);
							let peer_is_known = true /* Peer exists in loaded library */;
							resp.send(peer_is_known).unwrap();
						}
						NetworkManagerEvent::ConnectionEstablished { peer } => {
							println!("ConnectionEstablished to peer '{}'!", peer.id);
						},
						NetworkManagerEvent::AcceptStream { peer, stream: (mut send, mut recv) } => {
							println!("AcceptStream from peer '{}'!", peer.id);

							tokio::spawn(async move {
								loop {
									// TODO: I would like the abstract the QUIC chunk reading if possible but we will see.
									match recv.read_chunk(64 * 1024, true).await {
										Ok(Some(data)) => {
											println!("received: {:?}", data);
											send.write(b"Pong").await.unwrap();
											break; // Close stream
										}
										Ok(None) => break, // Connection is closed
										Err(e) => {
											println!("Error: {:?}", e);
											break;
										}
									}
								}
							});
						},
						NetworkManagerEvent::ConnectionClosed { peer } => {
							println!("ConnectionClosed to peer '{}'!", peer.id);
						}
					},
					None => break,
				}
			}
			// Broadcast a ping to all connected peers every 2 seconds as an example.
			_ = tokio::time::sleep(Duration::from_secs(2)) => {
				println!("Connected: {:?} Discovered: {:?}", nm.connected_peers().await.into_iter().map(|x| x.0).collect::<Vec<_>>(), nm.discovered_peers().into_iter().map(|x| x.0).collect::<Vec<_>>());

				for (_, peer) in nm.connected_peers().await.iter() {
					let (mut tx, mut _rx) = peer.stream().await.unwrap();
					tx.write(b"Ping").await.unwrap();

					// TODO: rx is dropped here which will cause the stream to be closed before getting the 'Pong' response.
					// TODO: Hence this API is going to change in the near future.
				}
			}
		}
	}
}
