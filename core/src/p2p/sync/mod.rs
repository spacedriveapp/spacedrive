use std::{collections::HashMap, sync::Arc};

use futures::future::join_all;
use sd_core_sync::{ingest, SyncManager};
use sd_p2p::{
	spacetunnel::{Identity, RemoteIdentity, Tunnel},
	DiscoveredPeer, PeerId,
};
use tokio::{io::AsyncWriteExt, sync::RwLock};
use uuid::Uuid;

use crate::library::Library;

use super::{Header, P2PManager, PeerMetadata};

mod proto;
pub use proto::*;

pub enum InstanceState {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

pub struct LibraryData {
	instances: HashMap<RemoteIdentity /* Identity public key */, InstanceState>,
}

pub struct NetworkedLibraryManager {
	p2p: Arc<P2PManager>,
	libraries: RwLock<HashMap<Uuid /* Library ID */, LibraryData>>,
}

impl NetworkedLibraryManager {
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

	// TODO: Error handling
	pub async fn alert_new_sync_events(&self, library_id: Uuid) {
		let libraries = self.libraries.read().await;

		join_all(
			libraries
				.get(&library_id)
				.unwrap()
				.instances
				.iter()
				.filter_map(|(_, i)| match i {
					InstanceState::Connected(peer_id) => Some(peer_id),
					_ => None,
				})
				.map(|peer_id| {
					let p2p = self.p2p.clone();
					async move {
						let mut stream =
							p2p.manager.stream(*peer_id).await.map_err(|_| ()).unwrap(); // TODO: handle providing incorrect peer id

						stream
							.write_all(&Header::Sync(library_id).to_bytes())
							.await
							.unwrap();

						let mut tunnel = Tunnel::initiator(stream).await.unwrap();

						tunnel
							.write_all(&SyncMessage::NewOperations.to_bytes())
							.await
							.unwrap();
					}
				}),
		)
		.await;
	}

	pub async fn emit_sync_ingest_alert(&self, mut tunnel: Tunnel, id: u8, sync: &SyncManager) {
		tunnel
			.write_all(&SyncMessage::OperationsRequest(id).to_bytes())
			.await
			.unwrap();
		tunnel.flush().await.unwrap();

		let msg = SyncMessage::from_stream(&mut tunnel).await.unwrap();

		match msg {
			SyncMessage::OperationsRequestResponse(id) => {
				sync.ingest
					.events
					.send(ingest::Event::Messages(id))
					.await
					.ok();
			}
			_ => todo!("unreachable but proper error handling"),
		};
	}
}
