use std::{collections::HashMap, sync::Arc};

use sd_p2p::{
	spacetunnel::{Identity, RemoteIdentity, Tunnel},
	DiscoveredPeer, PeerId,
};
use tokio::{io::AsyncReadExt, sync::RwLock};
use uuid::Uuid;

use crate::library::Library;

use super::{Header, P2PManager, PeerMetadata};

pub enum InstanceState {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

pub struct LibraryData {
	instances: HashMap<RemoteIdentity /* Identity public key */, InstanceState>,
}

pub struct NetworkedSyncManager {
	p2p: Arc<P2PManager>,
	libraries: RwLock<HashMap<Uuid /* Library ID */, LibraryData>>,
}

impl NetworkedSyncManager {
	pub fn new(p2p: Arc<P2PManager>) -> Arc<Self> {
		Arc::new(Self {
			p2p,
			libraries: Default::default(),
		})
	}

	pub async fn load_library(&self, library: &Library) {
		// TODO: Error handling
		let instances = library
			.db
			.instance()
			.find_many(vec![])
			.exec()
			.await
			.unwrap();

		let metadata_instances = instances
			.iter()
			.map(|i| {
				hex::encode(
					Identity::from_bytes(&i.identity)
						.unwrap()
						.to_remote_identity()
						.to_bytes(),
				)
			})
			.collect();

		let mut libraries = self.libraries.write().await;
		libraries.insert(
			library.id,
			LibraryData {
				instances: instances
					.into_iter()
					.map(|i| {
						(
							// TODO: Error handling
							// TODO: Linear issue about the `identity` column -> This will probs fail
							Identity::from_bytes(&i.identity)
								.unwrap()
								.to_remote_identity(),
							InstanceState::Unavailable,
						)
					})
					.collect(),
			},
		);

		self.p2p.update_metadata(metadata_instances).await;
	}

	// TODO: edit_library hook -> Send changes to all connected nodes!
	// TODO: delete_library hook -> Send delete to all connected nodes!

	pub async fn peer_discovered(&self, event: DiscoveredPeer<PeerMetadata>) {
		let pks = event
			.metadata
			.instances
			.iter()
			.filter_map(|pk| hex::decode(pk).ok())
			.filter_map(|pk| RemoteIdentity::from_bytes(&pk).ok())
			.collect::<Vec<_>>();

		for lib in self.libraries.write().await.values_mut() {
			if let Some((_pk, instance)) = lib
				.instances
				.iter_mut()
				.find(|(pk, _)| pks.iter().any(|pk2| *pk2 == **pk))
			{
				if !matches!(instance, InstanceState::Connected(_)) {
					let should_connection = matches!(instance, InstanceState::Unavailable);

					*instance = InstanceState::Discovered(event.peer_id.clone());

					if should_connection {
						event.dial().await;
					}
				}

				return; // PK can only exist once so we short circuit
			}
		}
	}

	pub async fn peer_expired(&self, id: PeerId) {
		for lib in self.libraries.write().await.values_mut() {
			for instance in lib.instances.values_mut() {
				if let InstanceState::Discovered(peer_id) = instance {
					if *peer_id == id {
						*instance = InstanceState::Unavailable;
					}
				}
			}
		}
	}

	pub async fn peer_connected(&self, peer_id: PeerId) {
		// TODO: This is a very suboptimal way of doing this cause it assumes a discovery message will always come before discover which is false.
		for lib in self.libraries.write().await.values_mut() {
			for instance in lib.instances.values_mut() {
				if let InstanceState::Discovered(id) = instance {
					if *id == peer_id {
						*instance = InstanceState::Connected(peer_id.clone());
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	pub async fn peer_disconnected(&self, peer_id: PeerId) {
		for lib in self.libraries.write().await.values_mut() {
			for instance in lib.instances.values_mut() {
				if let InstanceState::Connected(id) = instance {
					if *id == peer_id {
						*instance = InstanceState::Unavailable;
					}
				}
			}
		}
	}
}

#[derive(Debug)]
pub enum SyncMessage {
	NewOperations,
	OperationsRequest(u8),
	OperationsRequestResponse(u8),
}

impl SyncMessage {
	pub fn header(&self) -> u8 {
		match self {
			Self::NewOperations => b'N',
			Self::OperationsRequest(_) => b'R',
			Self::OperationsRequestResponse(_) => b'P',
		}
	}

	pub async fn from_tunnel(stream: &mut Tunnel) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			b'N' => Ok(Self::NewOperations),
			b'R' => Ok(Self::OperationsRequest(stream.read_u8().await?)),
			b'P' => Ok(Self::OperationsRequestResponse(stream.read_u8().await?)),
			header => Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!(
					"Invalid sync message header: {}",
					(header as char).to_string()
				),
			)),
		}
	}

	pub fn to_bytes(self, library_id: Uuid) -> Vec<u8> {
		// Header -> SyncMessage
		let mut bytes = Header::Sync(library_id).to_bytes();

		bytes.push(self.header());

		match self {
			Self::OperationsRequest(s) => bytes.push(s),
			Self::OperationsRequestResponse(s) => bytes.push(s),
			_ => {}
		}

		bytes
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_types() {
		// TODO: Finish this unit test

		{
			let original = SyncMessage::NewOperations;

			// let mut cursor = std::io::Cursor::new(original.to_bytes());
			// let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			// assert_eq!(original, result);
		}

		// let msg = SyncMessage::OperationsRequest(1);

		// let msg = SyncMessage::OperationsRequestResponse(2);
	}
}
