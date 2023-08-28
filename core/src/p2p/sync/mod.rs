use std::{collections::HashMap, sync::Arc};

use sd_core_sync::ingest;
use sd_p2p::{
	proto::{decode, encode},
	spacetunnel::{RemoteIdentity, Tunnel},
	DiscoveredPeer, PeerId,
};
use sd_sync::CRDTOperation;
use serde::Serialize;
use specta::Type;
use sync::GetOpsArgs;

use tokio::{
	io::{AsyncRead, AsyncWriteExt},
	sync::RwLock,
};
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
	library::{Libraries, Library, LibraryManagerEvent},
	sync,
};

use super::{Header, IdentityOrRemoteIdentity, P2PManager, PeerMetadata};

mod proto;
pub use proto::*;

#[derive(Debug, Clone, Copy, Serialize, Type)]
pub enum InstanceState {
	Unavailable,
	Discovered(PeerId),
	Connected(PeerId),
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct LibraryData {
	instances: HashMap<RemoteIdentity /* Identity public key */, InstanceState>,
}

type LibrariesMap = HashMap<Uuid /* Library ID */, LibraryData>;

pub struct NetworkedLibraries {
	p2p: Arc<P2PManager>,
	pub(crate) libraries: RwLock<HashMap<Uuid /* Library ID */, LibraryData>>,
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
							// We don't own it so don't advertise it
							IdentityOrRemoteIdentity::Identity(_) => None,
							IdentityOrRemoteIdentity::RemoteIdentity(identity) => {
								Some((identity, InstanceState::Unavailable))
							}
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

	pub async fn state(&self) -> LibrariesMap {
		self.libraries.read().await.clone()
	}
}

// These functions could be moved to some separate protocol abstraction
// which would be pretty cool.
//
// TODO: Error handling

pub use originator::run as originator;
mod originator {
	use super::*;
	use responder::tx as rx;

	pub mod tx {
		use super::*;

		pub struct Operations(pub Vec<CRDTOperation>);

		impl Operations {
			// TODO: Per field errors for better error handling
			pub async fn from_stream(
				stream: &mut (impl AsyncRead + Unpin),
			) -> std::io::Result<Self> {
				Ok(Self(
					rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
				))
			}

			pub fn to_bytes(&self) -> Vec<u8> {
				let Self(args) = self;
				let mut buf = vec![];

				// TODO: Error handling
				encode::buf(&mut buf, &rmp_serde::to_vec_named(&args).unwrap());
				buf
			}
		}
	}

	pub async fn run(
		library_id: Uuid,
		sync: &Arc<sync::Manager>,
		nlm: &NetworkedLibraries,
		p2p: &Arc<super::P2PManager>,
	) {
		let libraries = nlm.libraries.read().await;
		let library = libraries.get(&library_id).unwrap();

		// libraries only connecting one-way atm
		dbg!(&library.instances);

		// TODO: Deduplicate any duplicate peer ids -> This is an edge case but still
		for instance in library.instances.values() {
			let InstanceState::Connected(peer_id) = *instance else {
				continue;
			};

			let sync = sync.clone();
			let p2p = p2p.clone();

			tokio::spawn(async move {
				debug!(
					"Alerting peer '{peer_id:?}' of new sync events for library '{library_id:?}'"
				);

				let mut stream = p2p.manager.stream(peer_id).await.map_err(|_| ()).unwrap(); // TODO: handle providing incorrect peer id

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

				while let Ok(rx::GetOperations::Operations(args)) =
					rx::GetOperations::from_stream(&mut tunnel).await
				{
					let ops = sync.get_ops(args).await.unwrap();

					debug!(
						"Sending '{}' sync ops from peer '{peer_id:?}' for library '{library_id:?}'",
						ops.len()
					);

					tunnel
						.write_all(&tx::Operations(ops).to_bytes())
						.await
						.unwrap();
					tunnel.flush().await.unwrap();
				}
			});
		}
	}
}

pub use responder::run as responder;
mod responder {
	use super::*;
	use originator::tx as rx;

	pub mod tx {
		use serde::{Deserialize, Serialize};

		use super::*;

		#[derive(Serialize, Deserialize)]
		pub enum GetOperations {
			Operations(GetOpsArgs),
			Done,
		}

		impl GetOperations {
			// TODO: Per field errors for better error handling
			pub async fn from_stream(
				stream: &mut (impl AsyncRead + Unpin),
			) -> std::io::Result<Self> {
				Ok(
					// TODO: Error handling
					rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
				)
			}

			pub fn to_bytes(&self) -> Vec<u8> {
				let mut buf = vec![];
				// TODO: Error handling
				encode::buf(&mut buf, &rmp_serde::to_vec_named(&self).unwrap());
				buf
			}
		}
	}

	pub async fn run(mut tunnel: Tunnel, library: Arc<Library>) {
		let ingest = &library.sync.ingest;

		let Ok(mut rx) = ingest.req_rx.try_lock() else {
			println!("Rejected sync due to libraries lock being held!");

			// TODO: Proper error returned to remote instead of this.
			// TODO: We can't just abort the connection when the remote is expecting data.
			tunnel
				.write_all(&tx::GetOperations::Done.to_bytes())
				.await
				.unwrap();
			tunnel.flush().await.unwrap();

			return;
		};

		ingest
			.event_tx
			.send(ingest::Event::Notification)
			.await
			.unwrap();

		while let Some(req) = rx.recv().await {
			use sync::ingest::*;

			const OPS_PER_REQUEST: u32 = 100;

			let Request::Messages { timestamps } = req else {
				continue;
			};

			tunnel
				.write_all(
					&tx::GetOperations::Operations(sync::GetOpsArgs {
						clocks: timestamps,
						count: OPS_PER_REQUEST,
					})
					.to_bytes(),
				)
				.await
				.unwrap();
			tunnel.flush().await.unwrap();

			let rx::Operations(ops) = rx::Operations::from_stream(&mut tunnel).await.unwrap();

			ingest
				.event_tx
				.send(Event::Messages(MessagesEvent {
					instance_id: library.sync.instance,
					has_more: ops.len() == OPS_PER_REQUEST as usize,
					messages: ops,
				}))
				.await
				.expect("TODO: Handle ingest channel closed, so we don't loose ops");
		}

		tunnel
			.write_all(&tx::GetOperations::Done.to_bytes())
			.await
			.unwrap();
		tunnel.flush().await.unwrap();
	}
}
