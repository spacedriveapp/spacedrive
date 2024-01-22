#![allow(unused)] // TODO: Remove this

use crate::library::{Libraries, Library, LibraryManagerEvent};

use sd_p2p::Service;

use std::{
	collections::HashMap,
	fmt,
	sync::{Arc, PoisonError, RwLock},
};

use tokio::sync::mpsc;
use tracing::{error, warn};
use uuid::Uuid;

use super::{IdentityOrRemoteIdentity, LibraryMetadata, P2PManager};

pub struct LibraryServices {
	services: RwLock<HashMap<Uuid, Arc<Service<LibraryMetadata>>>>,
	register_service_tx: mpsc::Sender<Arc<Service<LibraryMetadata>>>,
}

impl fmt::Debug for LibraryServices {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("LibraryServices")
			.field(
				"services",
				&self
					.services
					.read()
					.unwrap_or_else(PoisonError::into_inner)
					.keys(),
			)
			.finish()
	}
}

impl LibraryServices {
	pub fn new(register_service_tx: mpsc::Sender<Arc<Service<LibraryMetadata>>>) -> Self {
		Self {
			services: Default::default(),
			register_service_tx,
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
						LibraryManagerEvent::InstancesModified(library)
						| LibraryManagerEvent::Load(library) => {
							manager
								.clone()
								.libraries
								.load_library(manager, &library)
								.await
						}
						LibraryManagerEvent::Edit(library) => {
							manager.libraries.edit_library(&library).await
						}
						LibraryManagerEvent::Delete(library) => {
							manager.libraries.delete_library(&library).await
						}
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `LibraryServices::start` manager aborted with error: {err:?}");
		}
	}

	pub fn get(&self, id: &Uuid) -> Option<Arc<Service<LibraryMetadata>>> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(id)
			.cloned()
	}

	pub fn libraries(&self) -> Vec<(Uuid, Arc<Service<LibraryMetadata>>)> {
		self.services
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(k, v)| (*k, v.clone()))
			.collect::<Vec<_>>()
	}

	pub(crate) async fn load_library(&self, manager: Arc<P2PManager>, library: &Library) {
		let identities = match library.db.instance().find_many(vec![]).exec().await {
			Ok(library) => library
				.into_iter()
				.filter_map(
					// TODO: Error handling
					|i| match IdentityOrRemoteIdentity::from_bytes(&i.identity) {
						Err(err) => {
							warn!("error parsing identity: {err:?}");
							None
						}
						Ok(IdentityOrRemoteIdentity::Identity(_)) => None,
						Ok(IdentityOrRemoteIdentity::RemoteIdentity(identity)) => Some(identity),
					},
				)
				.collect(),
			Err(err) => {
				warn!("error loading library '{}': {err:?}", library.id);
				return;
			}
		};

		let mut inserted = false;

		let service = {
			let mut service = self
				.services
				.write()
				.unwrap_or_else(PoisonError::into_inner);
			let service = service.entry(library.id).or_insert_with(|| {
				inserted = true;
				Arc::new(
					Service::new(library.id.to_string(), manager.manager.clone())
						.expect("error creating service with duplicate service name"),
				)
			});
			service.add_known(identities);
			service.clone()
		};

		if inserted {
			service.update(LibraryMetadata {});
			if self.register_service_tx.send(service).await.is_err() {
				warn!("error sending on 'register_service_tx'. This indicates a bug!");
			}
		}
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
