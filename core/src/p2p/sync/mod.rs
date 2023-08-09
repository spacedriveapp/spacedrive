use std::{collections::HashMap, sync::Arc};

use sd_p2p::{
	spacetunnel::{RemoteIdentity, Tunnel},
	DiscoveredPeer, PeerId,
};
use sync::GetOpsArgs;
use tokio::{io::AsyncWriteExt, sync::RwLock};
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
	library::{Libraries, Library, LibraryManagerEvent},
	sync,
};

use super::{Header, IdentityOrRemoteIdentity, P2PManager, PeerMetadata};

mod proto;
pub use proto::*;

#[derive(Debug, Clone, Copy)]
pub enum InstanceState {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

pub struct LibraryData {
	instances: HashMap<RemoteIdentity /* Identity public key */, InstanceState>,
}

pub struct NetworkedLibraries {
	p2p: Arc<P2PManager>,
	libraries: RwLock<HashMap<Uuid /* Library ID */, LibraryData>>,
}

impl NetworkedLibraries {
	pub fn new(p2p: Arc<P2PManager>, lm: &Libraries) -> Arc<Self> {
		let this = Arc::new(Self {
			p2p,
			libraries: Default::default(),
		});

		tokio::spawn({
			let this = this.clone();
			let rx = lm.rx.clone();

			async move {
				if let Err(err) = rx
					.subscribe(|msg| {
						let this = this.clone();
						async move {
							match msg {
								LibraryManagerEvent::Load(library) => {
									Self::load_library(&this, &library).await;
								}
								LibraryManagerEvent::Edit(library) => {
									Self::edit_library(&this, &library).await;
								}
								LibraryManagerEvent::InstancesModified(library) => {
									Self::load_library(&this, &library).await;
								}
								LibraryManagerEvent::Delete(library) => {
									Self::delete_library(&this, &library).await;
								}
							}
						}
					})
					.await
				{
					error!("Core may become unstable! NetworkedLibraryManager's library manager subscription aborted with error: {err:?}");
				}
			}
		});

		this
	}

	// TODO: Error handling
	async fn load_library(self: &Arc<Self>, library: &Library) {
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
				IdentityOrRemoteIdentity::from_bytes(&i.identity)
					.unwrap()
					.remote_identity()
			})
			.collect();

		let mut libraries = self.libraries.write().await;
		libraries.insert(
			library.id,
			LibraryData {
				instances: instances
					.into_iter()
					.filter_map(|i| {
						// TODO: Error handling
						match IdentityOrRemoteIdentity::from_bytes(&i.identity).unwrap() {
							IdentityOrRemoteIdentity::Identity(identity) => {
								Some((identity.to_remote_identity(), InstanceState::Unavailable))
							}
							// We don't own it so don't advertise it
							IdentityOrRemoteIdentity::RemoteIdentity(_) => None,
						}
					})
					.collect(),
			},
		);

		self.p2p.update_metadata(metadata_instances).await;
	}

	async fn edit_library(&self, _library: &Library) {
		// TODO: Send changes to all connected nodes!

		// TODO: Update mdns
	}

	async fn delete_library(&self, library: &Library) {
		// TODO: Do proper library delete/unpair procedure.
		self.libraries.write().await.remove(&library.id);

		// TODO: Update mdns
	}

	// TODO: Replace all these follow events with a pub/sub system????

	pub async fn peer_discovered(&self, event: DiscoveredPeer<PeerMetadata>) {
		for lib in self.libraries.write().await.values_mut() {
			if let Some((_pk, instance)) = lib
				.instances
				.iter_mut()
				.find(|(pk, _)| event.metadata.instances.iter().any(|pk2| *pk2 == **pk))
			{
				if !matches!(instance, InstanceState::Connected(_)) {
					let should_connect = matches!(instance, InstanceState::Unavailable);

					*instance = InstanceState::Discovered(event.peer_id);

					if should_connect {
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
		// TODO: Hence part of the need for `Self::peer_connected2`
		for lib in self.libraries.write().await.values_mut() {
			for instance in lib.instances.values_mut() {
				if let InstanceState::Discovered(id) = instance {
					if *id == peer_id {
						*instance = InstanceState::Connected(peer_id);
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	// TODO: Remove need for this cause it's weird
	pub async fn peer_connected2(&self, instance_id: RemoteIdentity, peer_id: PeerId) {
		for lib in self.libraries.write().await.values_mut() {
			if let Some(instance) = lib.instances.get_mut(&instance_id) {
				*instance = InstanceState::Connected(peer_id);
				return; // Will only exist once so we short circuit
			}
		}
	}

	pub async fn peer_disconnected(&self, peer_id: PeerId) {
		for lib in self.libraries.write().await.values_mut() {
			for instance in lib.instances.values_mut() {
				if let InstanceState::Connected(id) = instance {
					if *id == peer_id {
						*instance = InstanceState::Unavailable;
						return; // Will only exist once so we short circuit
					}
				}
			}
		}
	}

	// TODO: Error handling
	pub async fn alert_new_ops(&self, library_id: Uuid, sync: &Arc<sync::Manager>) {
		debug!("NetworkedLibraryManager::alert_new_ops({library_id})");

		let libraries = self.libraries.read().await;
		let library = libraries.get(&library_id).unwrap();

		// libraries only connecting one-way atm
		dbg!(&library.instances);

		// TODO: Deduplicate any duplicate peer ids -> This is an edge case but still
		for (_, instance) in &library.instances {
			let InstanceState::Connected(peer_id) = *instance else {
				continue;
			};

			let sync = sync.clone();
			let p2p = self.p2p.clone();

			tokio::spawn(async move {
				debug!(
					"Alerting peer '{peer_id:?}' of new sync events for library '{library_id:?}'"
				);

				let mut stream = p2p
					.manager
					.stream(peer_id.clone())
					.await
					.map_err(|_| ())
					.unwrap(); // TODO: handle providing incorrect peer id

				stream
					.write_all(&Header::Sync(library_id).to_bytes())
					.await
					.unwrap();

				let mut tunnel = Tunnel::initiator(stream).await.unwrap();

				tunnel
					.write_all(&SyncMessage::NewOperations.to_bytes())
					.await
					.unwrap();
				tunnel.flush().await.unwrap();

				while let Ok(SyncMessage::OperationsRequest(args)) =
					SyncMessage::from_stream(&mut tunnel).await
				{
					// let args = match .unwrap() {
					// 	 => resp,
					// 	_ => todo!("unreachable but proper error handling"),
					// };

					let ops = sync.get_ops(args).await.unwrap();

					debug!(
						"Sending '{}' sync ops from peer '{peer_id:?}' for library '{library_id:?}'",
						ops.len()
					);

					tunnel
						.write_all(&SyncMessage::OperationsRequestResponse(ops).to_bytes())
						.await
						.unwrap();
					tunnel.flush().await.unwrap();
				}
			});
		}
	}

	// Ask the remote for operations and then ingest them
	pub async fn request_ops(
		&self,
		tunnel: &mut Tunnel,
		args: GetOpsArgs,
	) -> Vec<sd_sync::CRDTOperation> {
		tunnel
			.write_all(&SyncMessage::OperationsRequest(args).to_bytes())
			.await
			.unwrap();
		tunnel.flush().await.unwrap();

		let SyncMessage::OperationsRequestResponse(ops) = SyncMessage::from_stream(tunnel).await.unwrap() else {
			todo!("unreachable but proper error handling")
		};

		// debug!("Received sync events response w/ id '{id}' from peer '{peer_id:?}' for library '{library_id:?}'");

		ops
	}

	// TODO: Error handling
	pub async fn exchange_sync_ops(
		&self,
		mut tunnel: Tunnel,
		peer_id: &PeerId,
		library_id: Uuid,
		sync: &sync::Manager,
		args: GetOpsArgs,
	) {
		let ops = sync.get_ops(args).await.unwrap();

		debug!(
			"Sending '{}' sync ops from peer '{peer_id:?}' for library '{library_id:?}'",
			ops.len()
		);

		tunnel
			.write_all(&SyncMessage::OperationsRequestResponse(ops).to_bytes())
			.await
			.unwrap();
	}
}
