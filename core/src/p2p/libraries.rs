use std::{
	collections::HashMap,
	fmt,
	sync::{Arc, PoisonError, RwLock},
};

use sd_p2p::Service;
use tokio::sync::broadcast;
use tracing::error;
use uuid::Uuid;

use crate::library::{Libraries, Library, LibraryManagerEvent};

use super::{IdentityOrRemoteIdentity, P2PManager, PeerMetadata};

pub struct LibraryServices {
	services: RwLock<HashMap<Uuid, Arc<Service<PeerMetadata>>>>, // TODO: probs don't use `PeerMetadata` here
	tx: broadcast::Sender<()>,
}

impl fmt::Debug for LibraryServices {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LibraryServices")
			.field("services", &self.services.read().unwrap().keys())
			.finish()
	}
}

impl LibraryServices {
	pub fn new(tx: broadcast::Sender<()>) -> Self {
		Self {
			services: Default::default(),
			tx,
		}
	}

	pub(crate) async fn start(manager: Arc<P2PManager>, libraries: Arc<Libraries>) {
		if let Err(err) = libraries
			.rx
			.clone()
			.subscribe(|msg| {
				let manager = manager.clone();
				async move {
					match msg {
						LibraryManagerEvent::Load(library) => {
							manager.libraries.load_library(&library).await
						}
						LibraryManagerEvent::Edit(library) => {
							manager.libraries.edit_library(&library).await
						}
						LibraryManagerEvent::InstancesModified(library) => {
							manager.libraries.load_library(&library).await
						}
						LibraryManagerEvent::Delete(library) => {
							manager.libraries.delete_library(&library).await
						}
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `networked_libraries_v2` manager aborted with error: {err:?}");
		}
	}

	pub fn get(&self, id: &Uuid) -> Option<Arc<Service<PeerMetadata>>> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(id)
			.cloned()
	}

	pub fn libraries(&self) -> Vec<(Uuid, Arc<Service<PeerMetadata>>)> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(k, v)| (*k, v.clone()))
			.collect::<Vec<_>>()
	}

	pub(crate) async fn load_library(&self, library: &Library) {
		let identities = library
			.db
			.instance()
			.find_many(vec![])
			.exec()
			.await
			.unwrap()
			.into_iter()
			.filter_map(
				// TODO: Error handling
				|i| match IdentityOrRemoteIdentity::from_bytes(&i.identity).unwrap() {
					IdentityOrRemoteIdentity::Identity(_) => None,
					IdentityOrRemoteIdentity::RemoteIdentity(identity) => Some(identity),
				},
			)
			.collect();

		self.services
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.get_mut(&library.id)
			.unwrap()
			.add_known(identities);
	}

	pub(crate) async fn edit_library(&self, _library: &Library) {
		// TODO: Send changes to all connected nodes!
		// TODO: Update mdns
	}

	pub(crate) async fn delete_library(&self, library: &Library) {
		drop(
			self.services
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.remove(&library.id),
		);
	}
}
