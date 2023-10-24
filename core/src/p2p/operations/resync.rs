//! TODO: I wanna remove this and replace it with a system in the P2P library itself!!!!

use std::sync::{Arc, PoisonError};

use sd_p2p::{spacetunnel::RemoteIdentity, PeerId, PeerStatus};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::{
	p2p::{Header, LibraryServices, P2PManager},
	Node,
};

// TODO: Break off `P2PManager`
impl P2PManager {
	pub async fn resync(
		libraries: &LibraryServices,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		peer_id: PeerId,
		instances: Vec<RemoteIdentity>,
	) {
		// TODO: Make this encrypted using node to node auth so it can't be messed with in transport

		stream
			.write_all(&Header::Connected(instances).to_bytes())
			.await
			.unwrap();

		let Header::Connected(identities) = Header::from_stream(stream).await.unwrap() else {
			panic!("unreachable but error handling")
		};

		for identity in identities {
			LibraryServices::peer_connected2(libraries, identity, peer_id);
		}
	}

	pub async fn resync_handler(
		libraries: &LibraryServices,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		peer_id: PeerId,
		local_identities: Vec<RemoteIdentity>,
		remote_identities: Vec<RemoteIdentity>,
	) {
		for identity in remote_identities {
			LibraryServices::peer_connected2(libraries, identity, peer_id);
		}

		stream
			.write_all(&Header::Connected(local_identities).to_bytes())
			.await
			.unwrap();
	}

	// TODO: Using tunnel for security - Right now all sync events here are unencrypted
	pub async fn resync_part2(
		libraries: &LibraryServices,
		node: Arc<Node>,
		connected_with_peer_id: &PeerId,
	) {
		let data = libraries.libraries();

		for (library_id, data) in data {
			let mut library = None;

			for data in data._get().values() {
				let PeerStatus::Connected(instance_peer_id) = data else {
					continue;
				};

				if *instance_peer_id != *connected_with_peer_id {
					continue;
				};

				let library = match library.clone() {
					Some(library) => library,
					None => match node.libraries.get_library(&library_id).await {
						Some(new_library) => {
							library = Some(new_library.clone());

							new_library
						}
						None => continue,
					},
				};

				// Remember, originator creates a new stream internally so the handler for this doesn't have to do anything.
				crate::p2p::sync::originator(library_id, &library.sync, &node.p2p).await;
			}
		}
	}
}
